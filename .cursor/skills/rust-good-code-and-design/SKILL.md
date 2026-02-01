---
name: rust-good-code-and-design
description: Guides Rust code and API design following Rust API Guidelines, style guide, and design patterns (Builder, Newtype, RAII, Strategy, State). Use when writing or reviewing Rust code, designing types or APIs, adding modules, or when the user asks for Rust best practices, naming, design patterns, or code review.
---

# Rust Good Code and Design

Apply these checklists when writing or reviewing Rust code. Follow the spirit of each rule; deviate only with good reason (Pragmatic Rust golden rule).

## Project Conventions (CLAUDE.md)

- One type per file; tests in a separate directory.
- Every type and method has detailed **English** comments, including **which types/methods they interact with**.
- Prefer simplicity; split tasks, document, mark done.

---

## Before Commit

| Action | Command |
|--------|---------|
| Format | `cargo fmt` |
| Lint | `cargo clippy` (zero warnings preferred; document any `#[allow(...)]`) |
| Build | `cargo build` / `cargo build --release` |
| Test | `cargo test` |

**Docs**: Public items use `///` with purpose, params, return, errors/panics/safety. Examples use `?`, not `unwrap()`. Comments in English; state interacting types/methods.

---

## Naming

| Area | Rule |
|------|------|
| Casing | `PascalCase` types/traits, `snake_case` functions/vars/modules, `SCREAMING_SNAKE_CASE` constants |
| Conversions | `as_*` (no ownership), `to_*` (copy), `into_*` (consume) |
| Getters | No `get_` prefix (e.g. `fn len()`) |
| Iterators | `iter`, `iter_mut`, `into_iter`; iterator type name matches method |
| Word order | Keep consistent across the crate |

---

## API and Type Design (Review Checklist)

**Interoperability**: Implement common traits where appropriate (`Copy`, `Clone`, `Eq`, `Debug`, `Display`, `Default`, etc.). Use `From`/`TryFrom`, `AsRef`/`AsMut`. Custom errors implement `std::error::Error` (e.g. with `thiserror`). Prefer `Send`+`Sync` when safe.

**Predictability**: Use methods (receiver) instead of free functions. No out-parameters; return values or `(T, U)`. Constructors: `Type::new()` / `Type::default()`. Use `Deref`/`DerefMut` only for smart pointers. Operator overloads match `std::ops` semantics.

**Flexibility**: Prefer generics (`R: Read`, `W: Write`). Expose intermediate results when callers might reuse them. If a trait may be used as `dyn Trait`, keep it object-safe (no `Self` return, no generic methods).

**Type safety**: Use newtypes for distinct semantics (e.g. `UserId` vs `OrderId`). Use enums instead of `bool`/magic values for options. Use `bitflags` for flag sets. Use Builder for complex or optional construction.

**Dependability**: Validate at API boundaries; return `Err` or document panic. `Drop` must not fail or block; provide explicit `close()` for fallible/blocking cleanup.

**Future proofing**: Structs have private fields. Use newtypes to hide internals. Use sealed traits to restrict implementors. Do not duplicate derived bounds on structs.

---

## Design Patterns (When to Use)

| Scenario | Prefer | Pattern |
|----------|--------|---------|
| Many fields / optional config | Struct literal or `Type::new()` | **Builder** if complex |
| Same shape, different meaning | **Newtype** (wrap + private) | — |
| Resource (lock, handle, connection) | **RAII** (Guard + `Drop`) | — |
| Swappable algorithm | **trait or closure** | Strategy |
| State machine | **enum** or type-state | State |
| Heterogeneous traversal (e.g. AST) | enum + match or **Visitor** | Visitor |

**YAGNI**: Prefer one simple implementation first; abstract to trait/strategy when a second variant appears.

**Builder**: `Type::builder()` → chain setters returning `self`/`&mut self` → `build()`. Validate in `build()`. Add new optional fields on builder for compatibility.

**Newtype**: `pub struct Wrapper(Inner)` or private fields. Expose only needed constructors/methods. Use `Deref` only for smart-pointer semantics.

**RAII**: Acquire in guard constructor, release in `Drop`. Guard exposes access via `Deref`. No failure or blocking in `Drop`; use explicit `close()` if needed.

**Anti-patterns to avoid**: Using `unwrap()`/`expect()` for normal error handling; unnecessary `clone()` to satisfy borrows; using `Deref` for “inheritance”; over-abstracting with generics; failing or blocking in `Drop`.

---

## Code Review Quick Checklist

- [ ] `cargo fmt`, `cargo clippy`, `cargo test` pass
- [ ] New types/files: English comments and “interacting types/methods” noted
- [ ] Public API: naming (above), interoperability, predictability, type safety, dependability, future proofing
- [ ] Errors: dedicated type, `Error` impl, documented conditions
- [ ] Public items: rustdoc and runnable example using `?`
- [ ] Design: Builder/Newtype/RAII/trait/closure/enum where appropriate; no listed anti-patterns

---

## Module and Layout

- One type per file (e.g. `user_id.rs`, `order.rs`).
- Minimal `pub`; use `pub(crate)` or private for internals.
- Group by domain/layer (e.g. `domain/`, `api/`, `infra/`) to keep files manageable.

---

## Testing (This Project)

- Tests in a separate directory or test module.
- BDD style; each test has an English comment describing scenario and what is being tested.
- Cover happy path, boundaries, and error paths; add integration tests if needed.

---

## Additional Resources

- Full checklists and design-pattern details: [reference.md](reference.md)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust Style Guide](https://doc.rust-lang.org/style-guide/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- Project: CLAUDE.md for repo-specific rules
