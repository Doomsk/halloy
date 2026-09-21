//! The Abstract Syntax Tree for Gouda Scripting.
//!
//! This is the **semantic** shape of a program, independent of concrete syntax.
//! The grammar (authored separately) parses source text into these nodes; the
//! interpreter (Phase 1) walks them. Spans are omitted here for readability and
//! will be attached by the parser in Phase 1 (each node wrapped so the editor
//! can anchor diagnostics) — the node *shapes* below are the stable contract.
//!
//! ## Two namespaces (the mIRC `%var` vs `$ident` duality)
//!
//! GS keeps user *variables* and *identifiers* in separate namespaces, exactly
//! as classic mIRC scripting did, but leaves the sigils to the grammar:
//!
//! - [`Expr::Var`] — a **user variable** (local or global). This is the only
//!   place "variable variables" apply: the *name* itself may be computed at
//!   runtime (see [`NameRef::Dynamic`]).
//! - [`Expr::Ident`] — an **identifier**: a user-defined function, a built-in,
//!   or an event super-global (`$nick`, `$chan`, …, modelled as a zero-argument
//!   identifier resolved from the event bindings). Resolution order is
//!   user-function → host identifier/built-in.

use crate::event::EventKind;

/// A whole compiled script file: a flat list of top-level declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// An event handler, e.g. run this block on every channel message.
    /// `guard`, if present, must evaluate truthy for the handler to run
    /// (the general form of mIRC's `on TEXT:matchtext:...`).
    Handler {
        event: EventKind,
        guard: Option<Expr>,
        body: Block,
    },
    /// A named, callable function returning a value.
    Function {
        name: String,
        params: Vec<String>,
        body: Block,
    },
    /// A user command (`/name ...`) invoked from the input box. Semantically a
    /// function, but registered in the alias table and dispatched via
    /// `on_input`. Args are the whitespace-split command arguments.
    Alias {
        name: String,
        params: Vec<String>,
        body: Block,
    },
}

/// A sequence of statements sharing a lexical scope.
pub type Block = Vec<Stmt>;

/// Variable scope for a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarScope {
    /// Lexically scoped to the enclosing block/handler; discarded when it ends.
    Local,
    /// Per-script, persisted to `<script>.state.toml` across reloads/restarts.
    Global,
}

/// A statement. Control flow is structured only — there is no `goto`.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Declare/initialise a variable in the given scope.
    Declare {
        scope: VarScope,
        name: NameRef,
        value: Expr,
    },
    /// Assign to an existing place (variable or index), optionally compound.
    Assign {
        target: Place,
        op: AssignOp,
        value: Expr,
    },
    /// An expression evaluated for its effect (e.g. a built-in call like
    /// `send(...)`), result discarded.
    Expr(Expr),
    If {
        cond: Expr,
        then_block: Block,
        /// Zero or more `elif` arms, in order.
        elifs: Vec<(Expr, Block)>,
        else_block: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    /// Iterate over a list (element by element) or a map (key by key).
    For {
        var: String,
        iter: Expr,
        body: Block,
    },
    Break,
    Continue,
    Return(Option<Expr>),
}

/// An assignable location.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// A variable, possibly with a dynamically computed name.
    Var(NameRef),
    /// An element of a list/map: `base[index]`.
    Index { base: Box<Place>, index: Box<Expr> },
}

/// How an assignment combines with the current value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    /// `=`
    Set,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
}

/// A variable name, static or computed — the heart of "variable variables".
#[derive(Debug, Clone, PartialEq)]
pub enum NameRef {
    /// A literal identifier known at parse time.
    Static(String),
    /// A name computed at runtime: the inner expression is evaluated and
    /// coerced to a string, and *that* string is the variable name. This is
    /// mIRC's `% [ $+ [ $nick ] ]` idiom. It only ever touches the script's own
    /// variable table, never host state, so it is safe by construction.
    Dynamic(Box<Expr>),
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),

    /// String building / concatenation lowered from interpolation or an
    /// explicit concat operator: the parts are evaluated left-to-right, each
    /// coerced to a string, and joined. (mIRC's `$+`.)
    Interp(Vec<Expr>),

    /// A list literal.
    List(Vec<Expr>),
    /// A map literal (insertion-ordered).
    Map(Vec<(Expr, Expr)>),

    /// Read a **user variable** (local/global). Supports dynamic names.
    Var(NameRef),

    /// Call an **identifier**: user function, built-in, or event super-global
    /// (the latter as a zero-arg identifier). Resolution: user fn → host.
    Ident { name: String, args: Vec<Expr> },

    /// Index into a list/map: `base[index]`.
    Index { base: Box<Expr>, index: Box<Expr> },

    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// Short-circuiting `and`/`or`.
    Logical {
        op: LogicalOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// Conditional expression (the general form of mIRC's `$iif(c, t, f)`).
    /// **Lazy**: `cond` is evaluated, then *only* the taken branch. Modelling
    /// this as an AST node rather than a built-in is what guarantees the
    /// untaken branch has no effect and cannot error.
    Ternary {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation `-x`.
    Neg,
    /// Logical negation `!x` / `not x`.
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    /// Explicit string concatenation, if the grammar exposes one.
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}
