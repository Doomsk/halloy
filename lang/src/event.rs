//! Script events — the host-agnostic projection of things that happen in the
//! client (an IRC event, a timer firing, a script being loaded).
//!
//! The `scripting-host` crate is responsible for translating Halloy's
//! `data::client::Event` into these. Keeping the type here (with no Halloy
//! dependency) is what lets the language core be tested with synthetic events
//! and keeps the event/effect boundary serializable, so the engine can later
//! move into a sidecar process without reshaping the language.

use crate::value::Value;

/// The kind of event, used to look up which handlers should run.
///
/// This is the v1 surface locked in the architecture plan. `on_input` drives
/// user-defined aliases (`/command`); the raw/gated kinds are opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    // Script lifecycle
    Load,
    Unload,
    // Connection lifecycle
    Connect,
    Disconnect,
    // Messages
    Message,
    Private,
    Notice,
    Action,
    // Channel
    Join,
    Part,
    Quit,
    Nick,
    // Client
    Timer,
    Input,
}

impl EventKind {
    /// Stable lowercase identifier, used when binding handlers and in tests.
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Load => "load",
            EventKind::Unload => "unload",
            EventKind::Connect => "connect",
            EventKind::Disconnect => "disconnect",
            EventKind::Message => "message",
            EventKind::Private => "private",
            EventKind::Notice => "notice",
            EventKind::Action => "action",
            EventKind::Join => "join",
            EventKind::Part => "part",
            EventKind::Quit => "quit",
            EventKind::Nick => "nick",
            EventKind::Timer => "timer",
            EventKind::Input => "input",
        }
    }
}

/// A concrete event handed to the interpreter. `bindings` carries the
/// per-event super-globals (`$nick`, `$chan`, `$text`, …) as a value map so the
/// language core never needs to know their Halloy origin.
#[derive(Debug, Clone)]
pub struct ScriptEvent {
    pub kind: EventKind,
    pub bindings: Value,
}

impl ScriptEvent {
    pub fn new(kind: EventKind, bindings: Value) -> Self {
        ScriptEvent { kind, bindings }
    }
}
