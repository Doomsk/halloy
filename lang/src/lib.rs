//! # Gouda Scripting (GS)
//!
//! The host-agnostic core of Halloy's scripting language. See
//! `.claude/plans/bubbly-humming-lantern.md` for the full architecture.
//!
//! ## Scope of this crate
//!
//! This crate is a **pure evaluator**: it knows nothing about IRC, iced, or the
//! filesystem. It takes a [`ScriptEvent`] plus a compiled program and produces a
//! list of [`ScriptEffect`]s. All side effects (sending messages, file I/O,
//! opening windows) are the responsibility of the `scripting-host` crate, which
//! validates and applies the effects.
//!
//! ## Build-out status
//!
//! Phase 0 (current): the stable boundary types — [`Value`], [`ScriptEvent`],
//! [`ScriptEffect`], and error types — are in place. The lexer, parser, AST,
//! resolver, and the fuel-metered tree-walking interpreter land in Phase 1,
//! after the concrete grammar is authored (Phase 0.5).

pub mod ast;
pub mod effect;
pub mod error;
pub mod event;
pub mod value;

pub use effect::{
    ChannelName, InvalidPayload, ModeString, ScriptEffect, Target, TargetName, Text, TimerName,
};
pub use error::{Diagnostic, RuntimeError, Span};
pub use event::{EventKind, ScriptEvent};
pub use value::Value;
