# Gouda Scripting (GS) — AST & Evaluation Semantics

This is the **Phase 0.5** deliverable: the semantic model of the language,
independent of concrete syntax. You author the grammar against this; the parser
lowers source text into the AST in [`src/ast.rs`](src/ast.rs); the interpreter
(Phase 1) walks it with these semantics.

Anything the grammar decides (sigils, keywords, whitespace/line rules, operator
spellings, comment style) is **yours** and not fixed here. This document fixes
*what the nodes mean*, not how they look.

---

## 1. Program structure

A script file is a flat list of top-level **items** ([`Item`]):

- **Handler** — `event` + optional `guard` + `body`. Runs when an event of that
  kind fires *and* the guard (if present) evaluates truthy. The guard is the
  general form of mIRC's `on TEXT:matchtext:` filter.
- **Function** — `name`, `params`, `body`. Callable, returns a value.
- **Alias** — a user command `/name` invoked from the input box. Same shape as a
  function; registered in the alias table and dispatched through `on_input`.

There are no free-floating top-level statements: everything executes inside a
handler, function, or alias. (One-time setup goes in an `on_load` handler.)

### Event kinds (v1, locked)

`load`, `unload`, `connect`, `disconnect`, `message`, `private`, `notice`,
`action`, `join`, `part`, `quit`, `nick`, `timer`, `input`. See
[`EventKind`](src/event.rs). `on_kick/topic/mode/ctcp/raw` are the "if-cheap"
tier; highlight/file-transfer/url are v2.

---

## 2. The two namespaces (design keystone)

GS deliberately separates **variables** from **identifiers**, mirroring mIRC's
`%var` vs `$ident` split. Pick whatever sigils you like in the grammar, but the
distinction must survive into the AST:

| Concept | AST node | Notes |
|---|---|---|
| User variable (local/global) | [`Expr::Var`] / [`Place::Var`] | The **only** place variable-variables apply. |
| Function / built-in / super-global | [`Expr::Ident`] | Super-globals (`$nick`) are zero-arg identifiers. |

**Why it matters:** it removes the ambiguity that plagued dynamically-scoped
scripting — a bare name is never "maybe a variable, maybe a command". The reader
(and the resolver) always knows which table to consult.

---

## 3. Variables & scope

Two scopes ([`VarScope`]):

- **Local** — lives in the enclosing block/handler; gone when it exits. Block
  scoping is lexical (inner blocks see outer locals; shadowing allowed).
- **Global** — per **script** (not shared across scripts in v1), and
  **persisted** to `<script>.state.toml`. Survives reloads and restarts. The
  cross-script "super-global" bus is v2.

Declaration vs assignment:

- **Declare** ([`Stmt::Declare`]) introduces a name in a chosen scope.
- **Assign** ([`Stmt::Assign`]) writes to an existing place (variable or index),
  optionally compound (`+= -= *= /=`).

Reading an undefined variable is a runtime error (`UndefinedVariable`). (We can
soften this to "empty string" per mIRC tradition if you prefer — flagged as an
open question in §10.)

### Variable variables

The name in a declaration/read/assignment is a [`NameRef`]:

- `Static("count")` — a literal name.
- `Dynamic(expr)` — evaluate `expr`, coerce to string, and **that** is the
  variable name. This is `% $+ $nick` — e.g. building `score_alice`,
  `score_bob` from `"score_" + $nick`.

Dynamic names resolve only against the script's own variable table, never host
state — this is what makes the feature safe here where it was dangerous in mIRC.

---

## 4. Types & values

Dynamic typing. Values ([`Value`]): `null`, `bool`, `int` (i64), `float` (f64),
`string`, `list`, `map` (insertion-ordered). No GC (reference-counted).

**Truthiness:** `null`, `false`, `0`, `0.0`, `""`, empty list, empty map are
falsy; all else truthy.

**Coercion (explicit, at defined points — never silent at arbitrary operators):**

- To **string**: via `Display` (§ used by `Interp`, `echo`, dynamic names). Ints
  and floats render naturally; `null` → `""`; collections render structurally.
- To **number**: string→number only through explicit built-ins (`int()`,
  `float()`), or in arithmetic where a string is numeric — TBD in §10.
- **Equality** (`==`): same-type value equality; `int`/`float` compare
  numerically across the two; different unrelated types are unequal (no
  throwing).

---

## 5. Expressions

Literals: `Null`, `Bool`, `Int`, `Float`, `Str`.

- **Interp** — the lowered form of string interpolation / `$+` concat: parts
  evaluated left→right, each coerced to string, joined. Your grammar decides the
  surface (`"hi $nick"`, or `"hi" $+ $nick`, …); both lower to `Interp`.
- **List** / **Map** literals.
- **Var(NameRef)** — read a user variable (dynamic name capable).
- **Ident { name, args }** — call a function/built-in/super-global.
- **Index { base, index }** — `list[i]` / `map[key]`.
- **Unary** — `Neg`, `Not`.
- **Binary** — `Add Sub Mul Div Mod Concat Eq Ne Lt Le Gt Ge`.
- **Logical** — `And`, `Or`, **short-circuiting** (rhs not evaluated if lhs
  settles the result).
- **Ternary** ([`Expr::Ternary`]) — the general form of mIRC's `$iif(c, t, f)`.
  **Lazy**: evaluate `cond`, then *only* the taken branch. This is an AST node
  (not a built-in) precisely so the untaken branch has no effect and cannot
  error — a plain `iif(...)` built-in would eval all three args eagerly.

Operator **precedence/associativity is the grammar's job**; the AST is already
fully parenthesised by nesting, so define precedence however reads best.

### Pipe operator (grammar sugar — no AST node)

Two things go by "pipe"; GS supports both, and neither needs an AST node:

- **Command chaining** (mIRC's `|`) — several statements on one line. This is
  just a statement separator; it lowers to a sequence of `Stmt`s in a `Block`.
  Choose the spelling in the grammar (`|`, `;`, newline…).
- **Data pipe** (bash/perl/awk `|`, spelled `|>` in Elixir/F#) — feed a value
  into the next call as its **first argument**. The parser desugars it:

  ```
  lhs |> name(a, b)   ==>   Ident { name, args: [lhs, a, b] }
  lhs |> name         ==>   Ident { name, args: [lhs] }
  ```

  So `$text |> lower() |> trim()` is just nested identifier calls. Because it's
  pure desugaring, pipelines get the same fuel metering and error handling as
  ordinary calls, for free.

---

## 6. Statements & control flow

`Declare`, `Assign`, `Expr` (effect-only), `If` (with ordered `elif` arms and
optional `else`), `While`, `For` (in over list elements / map keys), `Break`,
`Continue`, `Return`. **No `goto`.**

`Break`/`Continue` are only valid inside a loop; `Return` only inside a
function/alias/handler body (a handler `Return` just stops that handler).

---

## 7. Event dispatch & effects

When an event fires, the host builds a [`ScriptEvent`] (`kind` + a `bindings`
map holding the super-globals). The engine runs every registered handler for
that kind whose guard passes, in registration order.

Handlers do not mutate the client. They emit [`ScriptEffect`]s
([`src/effect.rs`](src/effect.rs)) — `Send`, `Notice`, `Action`, `Join`, `Part`,
`Mode`, `Echo`, `Notify`, `SetTimer`, `CancelTimer` — which the host validates
(capabilities, `anti_flood` rate limiting, path jailing) and applies. In the
language these appear as built-in identifiers (e.g. `send(...)`), so producing an
effect is just an `Expr::Ident` call evaluated as a statement.

**The AST is never turned back into a string.** Source is parsed to the AST
once, at load. At runtime the interpreter *evaluates* argument expressions to
[`Value`]s, and the effect-producing built-in constructs the effect from those
runtime values. Those values flow into **validated newtypes** — [`Text`],
[`TargetName`], [`ChannelName`], [`ModeString`], [`TimerName`] — whose
constructors are the only way to build them. This makes malformed effects
unrepresentable:

- CR/LF/NUL are rejected in every field → no **protocol injection** (a script
  can't smuggle `\r\n RAW` inside a message/nick/mode).
- Target names reject spaces and `,` → no **target smuggling**.
- `join`/`part` require a channel prefix (`# & + !`) → can't be aimed at a nick.
- Every field is length-capped.
- Formatting control bytes (colour/bold/italic) are **allowed** in `Text` — only
  the protocol-breaking bytes are forbidden.

A construction failure becomes a `RuntimeError::InvalidEffect`, aborting just
that handler. This is a second, type-enforced guardrail *behind* the host's
capability/rate-limit checks.

Super-globals available in v1: `$me $server $chan $nick $text $target $address
$time $version`. These are identifiers reading from the event `bindings`.

---

## 8. Built-in identifiers (v1)

Pure (in `gouda`): string ops, math, `rand`, list/map ops, time, `int()`/
`float()`/type checks, bounded `regex`. Effect-producing (host): the effects in
§7, plus `read_toml`/`write_toml` (+ `readini`/`writeini` sugar), `notify`.

The exact built-in list is finalised in Phase 1/2; the grammar only needs to
know that identifier-calls exist with `name(args...)` shape.

---

## 9. Execution limits (how the grammar is unaffected but the runtime is safe)

The interpreter is **fuel-metered**: it decrements a counter at every statement
and every expression node, and when fuel runs out it yields to the scheduler
(cooperative time-slicing via `corosensei`) or, past the total budget, aborts
the handler with `BudgetExceeded`. Recursion is depth-capped (`RecursionLimit`).
None of this affects syntax — it's purely a property of the walk — but it's why
an infinite `while true` loop in a script can never freeze the client.

---

## 10. Open questions for you (before/with grammar authoring)

1. **Undefined variable read** → runtime error (current design) *or* empty
   string (mIRC tradition)? Affects how forgiving scripts feel.
2. **String→number in arithmetic**: auto-coerce numeric strings, or require
   `int()`/`float()`? (mIRC auto-coerces.)
3. **Do you want an explicit `Concat` operator** in addition to interpolation,
   or should all string-building go through `Interp`? (AST supports both.)
4. **Alias arguments**: positional `params` (as modelled) vs mIRC's `$1 $2 $*`
   token style? Either lowers fine; pick the ergonomics you want.
5. **Handler guard**: a general boolean expression (current), and/or a
   dedicated match-pattern sugar over `$text`?
6. **Pipe spellings**: pick the surface for the two pipes — command separator
   (`|` / `;` / newline) and data pipe (`|>` / `|` / other). Both are grammar
   choices; the AST is unaffected (§5).

Answer these as you write the grammar; none require AST changes except possibly
adding a super-global for `$1..$N` alias tokens (trivial).
