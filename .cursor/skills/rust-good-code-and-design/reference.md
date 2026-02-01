# Rust Good Code and Design — Reference

Extended checklists and design-pattern details. Use when the agent needs deeper guidance.

## Golden Rule (Pragmatic Rust)

Understand why each rule exists and what it protects; follow the spirit, not the letter. Deviate only with good reason. Goals: safety, performance, readability, maintainability.

---

## Documentation Checklist

| Item | Practice |
|------|----------|
| Crate-level | `//!` at top of lib.rs/main.rs: purpose, example, main types |
| Public items | `///` with one-line summary + errors/panics/safety if applicable |
| Examples | Runnables in rustdoc; use `?` in examples |
| Links | Link to related types/methods in prose |
| Cargo.toml | description, license, repository, documentation |

---

## Design Patterns — Detail

### Builder

- Use when: many fields, optional config, or construction has side effects.
- Provide `Type::builder() -> TypeBuilder` or `TypeBuilder::default()`.
- Chain: setters return `self` or `&mut self`; final `build() -> T` or `build() -> Result<T, E>`; validate in `build()`.
- Adding fields: add optional/default on builder only; keep callers compatible.
- Std example: `std::process::Command`. Optional: `derive_builder` for boilerplate.

### Newtype

- Use for: distinct semantics (UserId vs OrderId), hiding representation (future proofing).
- `pub struct NewType(pub(crate) Inner)` or private fields; `From`/`TryFrom` as needed; `Deref` only for smart-pointer semantics.
- Expose only required constructors and methods.

### RAII

- Use for: locks, file handles, connections (acquire–release pairs).
- Guard type: acquire in constructor, release in `Drop`; expose via `Deref` (e.g. `MutexGuard`).
- `Drop` must not fail or block; provide explicit `close()` for fallible/blocking cleanup and document.

### Strategy / Swappable behavior

- Idiomatic: trait or closure. One-off: `impl Fn(...) -> ...` or `fn` pointer. Reusable/testable: trait (e.g. `Formatter`), hold `impl Formatter` or `dyn Formatter`.
- YAGNI: implement one path first; introduce trait when a second implementation appears.

### State / State machine

- Simple: enum for states; methods or match for behavior; transitions = new enum value.
- Advanced: one type per state implementing a common trait; `Box<dyn State>` or type-state to constrain operations by current state.
- Avoid: raw `bool`, `u8`, or string for state; use enum or types.

### Visitor

- Use for: traversing heterogeneous structures (e.g. AST) without changing node types.
- Define `Visitor` trait; each node type has `accept(&self, v: &mut dyn Visitor)` calling `v.visit_xxx(self)`. Alternatively: unify with enum and match (often simpler). Prefer Visitor only when node types are many and fixed.

---

## Anti-Patterns

| Anti-pattern | Problem | Prefer |
|--------------|---------|--------|
| `unwrap()`/`expect()` for normal errors | Unrecoverable crash | `Result` + `?`; avoid panic in libraries |
| `clone()` to fix borrows | Cost and ownership confusion | Restructure or lifetimes; prefer borrowing |
| `Deref` for “inheritance” | Implicit, surprising | Use only for smart pointers; otherwise methods or composition |
| Early/over abstraction | Hard to read and change | YAGNI; concrete first, abstract on repetition |
| Failure or blocking in `Drop` | Unobservable, hard to test | `Drop` best-effort only; explicit `close()` for fallible/blocking |

---

## Type and Error Design

- **Invariants**: Encode in types (e.g. validated input as newtype + constructor).
- **Options**: Prefer enum over magic values or bare `bool`.
- **Errors**: Dedicated type per domain; implement `Error`, `Display`, `source()`; consider `thiserror`. Libraries: `Result` for recoverable errors; document when panic is used.
- **Ownership**: Prefer `&T`/`&str`; use `impl AsRef`/`Borrow` in APIs; return `impl Iterator` where possible to avoid extra `collect()`.

---

## External References

- [Rust API Guidelines Checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust Style Guide](https://doc.rust-lang.org/style-guide/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [Rust 编码规范（中文）](https://rust-coding-guidelines.github.io/rust-coding-guidelines-zh/)
- [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/)
- The Rust Programming Language (the book)
- Project: CLAUDE.md
