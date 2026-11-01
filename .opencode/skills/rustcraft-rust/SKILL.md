---
name: rustcraft-rust
description: Write, review, and refactor RustCraft Rust code with a small, consistent set of style and validation rules. Use for Rust code, tests, Cargo configuration, or changes to the local build workflow.
---

# RustCraft Rust

## Scope and structure

- Keep one Cargo package with a thin executable and renderer-independent library logic.
- Use `docs/architecture.md` for ownership and dependency boundaries. Keep module internals private, split cohesive responsibilities before 650 lines, and preserve the architecture tests rather than adding exemptions.
- Add modules, crates, dependencies, and abstractions only when a concrete feature needs them.
- Use `wgpu` and `winit` when rendering work starts; do not introduce an engine framework.
- Target macOS first without unnecessarily tying world logic to the platform.
- For parallel subagent work, agree on shared coordinate/API contracts first and assign non-overlapping file ownership. The integrating agent owns Cargo, module registration, and the final whole-project checks.

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
- Follow the pinned toolchain's idioms: Rust 1.99 Clippy prefers `is_multiple_of` and `as_chunks::<N>()` for fixed-size groups. Floating-point matrix/collision tests need tolerances justified by world scale, not exact equality for accumulated arithmetic.
- Keep `Cargo.lock` committed. Use the pinned toolchain, and change dependencies or the toolchain deliberately.
- Before finishing a Rust change, run:

  ```sh
  cargo fmt --all -- --check
  cargo clippy --all-targets --locked -- -D warnings
  cargo test --locked
  ```

- Also run `cargo run --locked -- --smoke-test` when executable/graphics behavior changes and `cargo build --release --locked` when build configuration changes. Example tests need an explicit `--example` target; run the biome probe tests when its diagnostic logic changes.
- Report failed checks and anything you could not verify. Do not add CI, hooks, task runners, or coverage services unless requested.
- Graphics changes need an actual GPU/window smoke run in addition to compilation. Prefer bounded auto-exit smoke modes and frame readback so validation does not rely on a permanently running interactive window.
- When whole-crate APIs are being integrated, the already-installed Naga library can validate WGSL independently without a new project dependency. This supplements, never replaces, native GPU validation.
- When extending an existing coordinate/material contract, update storage indexing, neighbor sampling, camera bounds, shader layouts, diagnostics, and fall recovery together. Keep a regression check for the previous playable milestone.
