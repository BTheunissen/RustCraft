# RustCraft

RustCraft is a personal hobby project: a Minecraft-inspired voxel sandbox and game engine in Rust. It is not an Airwallex project. Do not add corporate tooling, CI, or external integrations unless requested.

## Development

- Before changing Rust code, tests, Cargo configuration, or the build workflow, load the `rustcraft-rust` skill at `.opencode/skills/rustcraft-rust/SKILL.md`.
- Keep one Cargo package, a thin `src/main.rs`, and testable, renderer-independent logic in `src/lib.rs` and its modules.
- The planned rendering stack is `wgpu` + `winit`. Add it when a rendering feature needs it, not preemptively.
- Use the toolchain pinned in `rust-toolchain.toml` and keep `Cargo.lock` tracked.
- Add focused tests for new behavior and use Rust language-server tools when available.
- Only create commits, add remotes, or push when requested.

## Local checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Also run `cargo run --locked` for executable changes and `cargo build --release --locked` for build-configuration changes. Report anything that could not be verified.
