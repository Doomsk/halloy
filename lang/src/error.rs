//! Error and diagnostic types for Gouda Scripting.
//!
//! Parse/resolve errors carry a source span so the editor can highlight the
//! offending range (Phase 3). Runtime errors abort a single handler without
//! taking down the engine.

use std::ops::Range;

use thiserror::Error;

/// A byte range in the source, used to anchor editor diagnostics.
pub type Span = Range<usize>;

/// A compile-time diagnostic (lex/parse/resolve). Phase 1 fills these in via
/// `chumsky`'s recoverable errors; Phase 0 just defines the shape.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            message: message.into(),
            span,
        }
    }
}

/// A runtime error raised while a handler executes. Aborting is per-handler:
/// the scheduler logs it and moves on, never crashing the client.
#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("type error: expected {expected}, found {found}")]
    Type {
        expected: &'static str,
        found: &'static str,
    },
    #[error("undefined variable `{0}`")]
    UndefinedVariable(String),
    #[error("undefined function `{0}`")]
    UndefinedFunction(String),
    #[error("wrong number of arguments to `{name}`: expected {expected}, got {got}")]
    Arity {
        name: String,
        expected: usize,
        got: usize,
    },
    #[error("division by zero")]
    DivisionByZero,
    /// The handler exhausted its fuel budget (see the scheduler). This is how a
    /// runaway or intentionally-infinite handler is stopped.
    #[error("execution budget exceeded")]
    BudgetExceeded,
    /// Recursion went past the configured depth cap.
    #[error("maximum recursion depth exceeded")]
    RecursionLimit,
    /// An effect-producing built-in was given a value that failed the effect
    /// boundary's validation (e.g. a message containing CR/LF, a target with a
    /// comma). Aborts the handler, never the client.
    #[error("invalid effect argument: {0}")]
    InvalidEffect(#[from] crate::effect::InvalidPayload),
    #[error("{0}")]
    Custom(String),
}
