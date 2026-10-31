# RustCraft

A hobby project to build a Minecraft-inspired voxel sandbox and its game engine in Rust. RustCraft is not affiliated with Mojang, Microsoft, or Airwallex.

The engine will use **`wgpu` + `winit`**, targeting **macOS first**. For now, this repository is a dependency-free foundation: a small executable, a renderer-independent library, and tests for block-to-chunk coordinates. It does not open a window or render a world yet.

## Setup

Install Rust through [rustup](https://rustup.rs/) if it is not already available:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal --default-toolchain none
```

Open a new terminal, or load Cargo's environment into the current shell:

```sh
. "$HOME/.cargo/env"
```

On macOS, Xcode or the Xcode Command Line Tools are required for linking. If neither is installed, run `xcode-select --install`.

Run commands from the repository root. `rust-toolchain.toml` pins **Rust 1.99.1** with `rustfmt`, Clippy, and `rust-analyzer`; rustup installs the pinned toolchain on first use if needed. The package uses **Rust edition 2024**.

## Build and run

```sh
cargo run --locked
cargo build --release --locked
```

The release executable is `target/release/rustcraft`. Debug builds are the default for local development; no custom build scripts or task runner are needed.

## Check a change

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

To apply formatting, run `cargo fmt --all` without `--check`. These are local checks, not CI.

Commit `Cargo.lock` and use `--locked` for routine builds and tests to prevent accidental lockfile changes. Update the lockfile deliberately when adding or upgrading a dependency. The toolchain pin and lockfile keep tool and dependency versions consistent; they do not guarantee byte-identical binaries across operating systems.

## Repository layout

```text
Cargo.toml                  Package metadata and dependencies
Cargo.lock                  Committed dependency lockfile
rust-toolchain.toml         Pinned Rust toolchain and components
src/
  main.rs                   Minimal executable entry point
  lib.rs                    Renderer-independent library entry point
  world.rs                  Block/chunk coordinate logic and unit tests
AGENTS.md                   Project instructions for coding agents
.opencode/skills/
  rustcraft-rust/SKILL.md    Concise Rust style and validation workflow
LICENSE                     MIT license
```

Keep one package until a concrete feature benefits from separate crates. Add rendering, assets, persistence, and other directories when they have real contents rather than scaffolding future subsystems now.

## Coordinate convention

Chunks span **16 blocks along each axis**. `world::split_block_coordinate` splits one integer world coordinate into a chunk coordinate and a local offset in `0..16`. Negative positions use floor division: world coordinate `-1` belongs to chunk `-1` at local offset `15`.

## Next milestone

Add a `winit` window and a minimal `wgpu` renderer, keeping world logic independent of the graphics layer.

## License

[MIT](LICENSE) © 2026 Ben Theunissen.
