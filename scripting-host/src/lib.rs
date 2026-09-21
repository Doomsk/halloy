//! # scripting-host
//!
//! The interface layer binding Gouda Scripting ([`gouda`]) to the Halloy
//! client. See `.claude/plans/bubbly-humming-lantern.md`.
//!
//! Responsibilities (built out across phases):
//! - **Loader** — discover `*.gs` files under the jailed `gouda` directory.
//! - **Event bridge** — project `data::client::Event` into [`gouda::ScriptEvent`].
//! - **Scheduler** — run handlers on `corosensei` coroutines, fuel-metered and
//!   time-sliced so no handler starves the rest (Phase 2).
//! - **Effect executor** — validate [`gouda::ScriptEffect`]s (capabilities,
//!   rate limiting, path jailing) and apply them to the client.
//! - **Hot reload** — transactional swap of the handler tables (Phase 3).
//!
//! Phase 0 (current) establishes the crate and the outward-facing message
//! types. The engine loop and effect execution land in Phase 2.

use gouda::{ScriptEffect, ScriptEvent};

/// A message flowing from the client *into* the scripting engine.
#[derive(Debug, Clone)]
pub enum ToEngine {
    /// An event to dispatch to any registered handlers.
    Event(ScriptEvent),
    /// Request a reload of all scripts from disk (transactional).
    Reload,
    /// Shut the engine down.
    Shutdown,
}

/// A message flowing from the engine *out* to the client, to be turned into an
/// iced `Message` and applied on the UI thread.
#[derive(Debug, Clone)]
pub enum FromEngine {
    /// One or more validated effects to apply.
    Effects(Vec<ScriptEffect>),
    /// A reload finished; carries the number of scripts now active and any
    /// diagnostics to surface in the editor.
    Reloaded { active: usize },
    /// A non-fatal engine-level message worth logging/surfacing.
    Log(String),
}
