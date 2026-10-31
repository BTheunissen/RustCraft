---
name: rustcraft-rust
description: Write, review, and refactor RustCraft Rust code with a small, consistent set of style and validation rules. Use for Rust code, tests, Cargo configuration, or changes to the local build workflow.
---

# RustCraft Rust

## Scope and structure

- Keep one Cargo package with a thin executable and renderer-independent library logic.
- Add modules, crates, dependencies, and abstractions only when a concrete feature needs them.
- Use `wgpu` and `winit` when rendering work starts; do not introduce an engine framework.
- Target macOS first without unnecessarily tying world logic to the platform.

## Code style

- Use idiomatic Rust naming and the default `rustfmt` style. Do not hand-align code or add formatter overrides.
- Prefer straightforward ownership and borrowing; avoid clones introduced only to bypass an unexplored borrow error.
- Keep APIs small and types explicit at public boundaries. Prefer concrete types until an actual use case needs generic ones.
- Return `Result` for expected runtime failures. Avoid `unwrap`, `expect`, and panics in normal runtime paths; they are fine for test assertions and documented invariants.
- Document public items and non-obvious behavior, especially coordinate conventions. Comments should explain intent or constraints, not repeat the code.
- Use Rust language-server tools for navigation and diagnostics when available. Always verify with Cargo as well.

## Tests and verification

- Keep focused unit tests beside the behavior they exercise; add integration tests only when testing public interactions warrants them.
- Cover ordinary cases, boundaries, negative coordinates, and failure paths where applicable. Avoid tests that merely assert a constant or repeat an implementation.
- Use standard Clippy checks; do not enable blanket pedantic rules or suppress a warning without a concrete reason.
- Keep `Cargo.lock` committed. Use the pinned toolchain, and change dependencies or the toolchain deliberately.
- Before finishing a Rust change, run:

  ```sh
  cargo fmt --all -- --check
  cargo clippy --all-targets --locked -- -D warnings
  cargo test --locked
  ```

- Also run `cargo run --locked` when executable behavior changes and `cargo build --release --locked` when build configuration changes.
- Report failed checks and anything you could not verify. Do not add CI, hooks, task runners, or coverage services unless requested.
