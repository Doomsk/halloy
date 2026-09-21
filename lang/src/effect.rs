//! Script effects — the *only* way a script influences the outside world.
//!
//! Handlers never touch Halloy state directly. They emit `ScriptEffect`s, which
//! the host validates further (capability checks, rate limiting, path jailing)
//! before applying. This indirection is the security firewall and keeps the
//! language core pure and testable.
//!
//! ## Typed, self-validating payloads
//!
//! Effect fields are **not** raw `String`s. Each carries a validated newtype
//! ([`Text`], [`TargetName`], [`ChannelName`], [`ModeString`], [`TimerName`])
//! whose constructor is the *only* way to build it. This makes an entire class
//! of attacks unrepresentable:
//!
//! - **Protocol injection** — CR/LF/NUL are rejected everywhere, so a script
//!   can never smuggle `\r\n RAW COMMAND` inside a message, nick, or mode.
//! - **Target smuggling** — target names reject spaces and `,` (the IRC target
//!   separator), so one "message" can't fan out to unintended targets.
//! - **Oversized payloads** — every field is length-capped.
//!
//! Importantly, formatting control bytes used for colours/bold/italic (e.g.
//! `0x02`, `0x03`, `0x0F`) are **allowed** in [`Text`] — only the
//! protocol-breaking bytes CR (`\r`), LF (`\n`) and NUL (`\0`) are rejected.
//!
//! The interpreter's effect-producing built-ins construct these types from
//! runtime [`crate::Value`]s; a validation failure surfaces to the script as an
//! ordinary [`crate::RuntimeError`], aborting that one handler, never the client.

use thiserror::Error;

/// Maximum bytes for a single text payload. IRC lines are ~512 bytes; the host
/// splits long messages, but we still cap to keep a runaway script bounded.
const MAX_TEXT_LEN: usize = 8192;
/// Maximum bytes for a name (nick/channel/timer) or mode string.
const MAX_NAME_LEN: usize = 256;

/// Why a payload was rejected. Never leaks host state; purely about the value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidPayload {
    #[error("value is empty")]
    Empty,
    #[error("value exceeds the maximum length of {max} bytes")]
    TooLong { max: usize },
    #[error("value contains a forbidden control character (CR, LF, or NUL)")]
    ProtocolChar,
    #[error("value contains a forbidden character `{0}`")]
    ForbiddenChar(char),
}

/// Reject the three bytes that would break the IRC line protocol. Deliberately
/// narrow: formatting control codes are fine, these are not.
fn reject_protocol_chars(s: &str) -> Result<(), InvalidPayload> {
    if s.bytes().any(|b| b == b'\r' || b == b'\n' || b == b'\0') {
        return Err(InvalidPayload::ProtocolChar);
    }
    Ok(())
}

/// A single-line text payload (message/notice/action/echo/notify body).
///
/// Guarantees: no CR/LF/NUL, at most [`MAX_TEXT_LEN`] bytes. May be empty (an
/// empty NOTICE is legal) and may contain formatting control codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPayload> {
        let s = s.into();
        if s.len() > MAX_TEXT_LEN {
            return Err(InvalidPayload::TooLong { max: MAX_TEXT_LEN });
        }
        reject_protocol_chars(&s)?;
        Ok(Text(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A message target name (a nick or channel used as a destination).
///
/// Guarantees: non-empty, ≤ [`MAX_NAME_LEN`] bytes, no whitespace, no CR/LF/NUL,
/// and no `,` (the IRC target separator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetName(String);

impl TargetName {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPayload> {
        let s = s.into();
        validate_name(&s)?;
        if let Some(c) = s.chars().find(|c| c.is_whitespace() || *c == ',') {
            return Err(InvalidPayload::ForbiddenChar(c));
        }
        Ok(TargetName(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A channel name. Same guarantees as [`TargetName`], plus it must begin with a
/// channel prefix (`#`, `&`, `+`, `!`) so a script's `join`/`part` cannot be
/// aimed at a bare nick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelName(String);

impl ChannelName {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPayload> {
        let s = s.into();
        validate_name(&s)?;
        match s.chars().next() {
            Some('#' | '&' | '+' | '!') => {}
            Some(c) => return Err(InvalidPayload::ForbiddenChar(c)),
            None => return Err(InvalidPayload::Empty),
        }
        if let Some(c) = s.chars().find(|c| c.is_whitespace() || *c == ',') {
            return Err(InvalidPayload::ForbiddenChar(c));
        }
        Ok(ChannelName(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A mode change string (e.g. `+o`, `-b`, `+mnt`). Guarantees: non-empty,
/// bounded, no CR/LF/NUL. Parameters are separated by single spaces only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeString(String);

impl ModeString {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPayload> {
        let s = s.into();
        if s.is_empty() {
            return Err(InvalidPayload::Empty);
        }
        if s.len() > MAX_NAME_LEN {
            return Err(InvalidPayload::TooLong { max: MAX_NAME_LEN });
        }
        reject_protocol_chars(&s)?;
        // No tabs/newlines-as-whitespace; a plain space is allowed between the
        // mode flags and their parameters, but nothing exotic.
        if let Some(c) = s.chars().find(|c| c.is_whitespace() && *c != ' ') {
            return Err(InvalidPayload::ForbiddenChar(c));
        }
        Ok(ModeString(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A timer identifier. Guarantees: non-empty, bounded, and restricted to
/// `[A-Za-z0-9_-]` so it can be used as a stable key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerName(String);

impl TimerName {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPayload> {
        let s = s.into();
        if s.is_empty() {
            return Err(InvalidPayload::Empty);
        }
        if s.len() > MAX_NAME_LEN {
            return Err(InvalidPayload::TooLong { max: MAX_NAME_LEN });
        }
        if let Some(c) = s
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
        {
            return Err(InvalidPayload::ForbiddenChar(c));
        }
        Ok(TimerName(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared validation for name-like fields: non-empty, bounded, no CR/LF/NUL.
fn validate_name(s: &str) -> Result<(), InvalidPayload> {
    if s.is_empty() {
        return Err(InvalidPayload::Empty);
    }
    if s.len() > MAX_NAME_LEN {
        return Err(InvalidPayload::TooLong { max: MAX_NAME_LEN });
    }
    reject_protocol_chars(s)
}

/// A destination for outgoing text/commands, resolved by the host against the
/// active server/buffer context of the triggering event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The channel or query the current event belongs to.
    Current,
    /// A validated named channel or nick.
    Named(TargetName),
}

/// An action requested by a script, to be validated and applied by the host.
/// Every field is a validated newtype, so an ill-formed effect cannot exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptEffect {
    /// Send a PRIVMSG to a target.
    Send { target: Target, text: Text },
    /// Send a NOTICE to a target.
    Notice { target: Target, text: Text },
    /// Send a CTCP ACTION (`/me`) to a target.
    Action { target: Target, text: Text },
    /// Join a channel.
    Join { channel: ChannelName },
    /// Part a channel, optionally with a reason.
    Part {
        channel: ChannelName,
        reason: Option<Text>,
    },
    /// Set a channel/user mode string.
    Mode { target: Target, modes: ModeString },
    /// Print a line to a buffer in the client (script-local output).
    Echo { target: Target, text: Text },
    /// Raise a desktop notification.
    Notify { title: Text, body: Text },
    /// Register (or replace) a script timer that will re-enter the engine with
    /// an `on_timer` event after `delay_ms`, repeating `repeat` more times
    /// (`None` = run forever until cancelled).
    SetTimer {
        name: TimerName,
        delay_ms: u64,
        repeat: Option<u64>,
    },
    /// Cancel a previously registered timer by name.
    CancelTimer { name: TimerName },
}
