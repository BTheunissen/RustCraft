# RustCraft

RustCraft is a personal hobby project: a Minecraft-inspired voxel sandbox and game engine in Rust. It is not an Airwallex project. Do not add corporate tooling, CI, or external integrations unless requested.

## Development

- Before changing Rust code, tests, Cargo configuration, or the build workflow, load the `rustcraft-rust` skill at `.opencode/skills/rustcraft-rust/SKILL.md`.
- Keep one Cargo package, a thin `src/main.rs`, and testable, renderer-independent logic in `src/lib.rs` and its modules.
- Follow `docs/architecture.md`; split by responsibility before 650 lines and keep CPU layers independent of native/GPU adapters. `cargo test` enforces these boundaries.
- Rendering uses `wgpu` + `winit`; keep GPU resources out of terrain, meshing, and player simulation.
- Use the toolchain pinned in `rust-toolchain.toml` and keep `Cargo.lock` tracked.
- Add focused tests for new behavior and use Rust language-server tools when available.
- Only create commits, add remotes, or push when requested.
- For terrain/meshing, use `rustcraft-voxel-world`; for movement/collision, use `rustcraft-player-controls`; for native rendering and end-to-end checks, use `rustcraft-native-playtest`.
- For negative-Y cave generation, geology, and underground lighting, also use `rustcraft-underground`. Chunk Y reads use world coordinates in `MIN_WORLD_Y..WORLD_HEIGHT`; do not confuse the upper coordinate with storage depth.
- For aquatic generation, currents/swimming, or water rendering, also read `.opencode/skills/rustcraft-water/SKILL.md`. Mean water levels, wet occupancy, resident metadata, and visual waves are separate contracts.
- When delegation is requested, assign distinct files and agree on coordinate/material/API contracts before parallel edits. Keep skill improvements short and grounded in bugs or validated workflows.

## Local checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Also run `cargo run --locked -- --smoke-test` for executable/graphics changes and `cargo build --release --locked` for build-configuration changes. Use `--screenshot <path>.png` with smoke mode to inspect actual GPU frame readback. Report anything that could not be verified, especially physical input interactions.
