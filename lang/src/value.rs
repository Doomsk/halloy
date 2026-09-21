//! The dynamic value type for Gouda Scripting.
//!
//! Values use `Rc` for cheap sharing and interior mutability for the
//! collection types. There is deliberately **no garbage collector**: memory is
//! reclaimed by reference counting via Rust's ownership. Scripts cannot build
//! self-referential cycles today (no closures capturing their own binding in
//! v1), so plain `Rc` is sufficient; if cycles become reachable later this is
//! the single place that would need a cycle-collector or weak links.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;

/// A runtime value. Dynamically typed; coercions are explicit (see the `as_*`
/// and `to_display_string` helpers) rather than implicit at the operator level.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<IndexMap<String, Value>>>),
}

impl Value {
    /// Construct a string value from anything string-like.
    pub fn str(s: impl AsRef<str>) -> Self {
        Value::Str(Rc::from(s.as_ref()))
    }

    /// Construct a list value from an iterator of values.
    pub fn list(items: impl IntoIterator<Item = Value>) -> Self {
        Value::List(Rc::new(RefCell::new(items.into_iter().collect())))
    }

    /// Construct an (ordered) map value from key/value pairs.
    pub fn map(entries: impl IntoIterator<Item = (String, Value)>) -> Self {
        Value::Map(Rc::new(RefCell::new(entries.into_iter().collect())))
    }

    /// The name of this value's type, for diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Map(_) => "map",
        }
    }

    /// Truthiness rules. `null`, `false`, `0`, `0.0`, empty string, and empty
    /// collections are falsy; everything else is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(items) => !items.borrow().is_empty(),
            Value::Map(entries) => !entries.borrow().is_empty(),
        }
    }
}

impl fmt::Display for Value {
    /// User-facing rendering, used by `echo`, string interpolation, and
    /// coercion to string. Kept deliberately simple and stable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str(""),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => f.write_str(s),
            Value::List(items) => {
                f.write_str("[")?;
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Map(entries) => {
                f.write_str("{")?;
                for (i, (k, v)) in entries.borrow().iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                f.write_str("}")
            }
        }
    }
}
