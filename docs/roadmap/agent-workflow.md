# Agent implementation workflow

Use this with the [roadmap](README.md) and the selected milestone brief. It describes future implementation work; the planning exercise itself changes documentation only.

## Start one bounded task

1. Read repository `AGENTS.md`, `docs/architecture.md`, the roadmap, and the selected task. Reconcile inspected source with stale descriptions rather than assuming documentation is current.
2. Check `git status --short` and inspect relevant existing changes. Preserve unrelated/user work; never reset, revert, delete, or rewrite it merely to get a clean baseline. No commits, remotes, or pushes unless requested.
3. Verify dependency gates and the milestone's owner playtest status. An unchecked prerequisite or an unresolved contract decision is a reason to stop, not permission to guess broadly.
4. Load `rustcraft-rust` before Rust/tests/Cargo/build changes and applicable subsystem skills from `.opencode/skills/`. Look for relevant additional instructions in `.claude`/`.codex` if present.
5. Use Rust language-server navigation/diagnostics when available, then verify with Cargo. If the server is unavailable or stale, state that; do not invent a clean diagnostic result.
6. Identify the task's file/API ownership and smallest testable change. Proposed paths in the briefs are suggestions; use the surrounding module conventions and keep responsibilities coherent.

Default to sequential implementation. Parallel agent edits require an explicit delegation request, agreed APIs/coordinates, non-overlapping ownership, and an integrating owner for module registration, Cargo, and whole-project validation. This document is not an instruction to launch agents automatically.

## Preserve project contracts

- One Cargo package; thin executable; testable CPU logic separated from `winit`/`wgpu` and native adapters.
- Follow the 650-line Rust file guardrail, including tests/examples. Do not disable architecture checks to accommodate a large patch.
- Seed/coordinate determinism, append-only material IDs, explicit unknown-residency behavior, actual water occupancy versus visual waves, and bounded residency/queues remain meaningful contracts.
- No incidental crates/frameworks, corporate tooling, CI, hooks, coverage services, task runner, multiplayer, or migrations. A feature-specific dependency requires a deliberate decision at its documented gate.
- Generation/format incompatibility may require a fresh development world. That does not authorize touching the owner's real save or bypassing replacement confirmation.
- Do not refresh golden hashes, weaken assertions, or discard tests solely because the new implementation fails them. Explain intentional changes and retain independent behavior/seam/failure tests.
- Keep early feature slices usable. Do not expose placeholder actions that cannot complete or make all prerequisites land before anything can be played.

## Implementation-ready handoff

Before a task's code work begins, resolve these from its brief/contract checkpoint:

- The player-visible outcome and explicit non-goals.
- The owning modules, API/data contracts, and any intentional save/generation version change.
- Deterministic or bounded work/memory limits where relevant.
- High-value ordinary, boundary, negative-coordinate, and failure-path tests.
- Visual/audio/manual checks that cannot be established by CPU assertions.

If a critical decision is missing, ask a narrow question or finish the specified contract task. Do not use a roadmap heading as authority for an open-ended rewrite.

## Validation

For every Rust implementation task:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Add focused behavior tests rather than tests that merely repeat constants. Reuse existing suites and add integration tests where public subsystem interactions warrant them. This hobby project does not need new corporate coverage infrastructure.

For executable/graphics changes, run the bounded real native check and inspect GPU readback:

```sh
cargo run --locked -- --smoke-test --screenshot <existing-approved-output-directory>/frame.png
```

The output directory must already exist. Prefer the session's approved temporary directory for local captures. Do not leave an unattended mouse-capturing game running or overwrite a user's screenshot without consent.

If native presentation is unavailable on a locked/asleep desktop:

```sh
cargo run --locked -- --offscreen-smoke-test --screenshot <existing-approved-output-directory>/frame.png
```

Report this as **offscreen rendering**, not native presentation or physical input acceptance. Native screenshots do not prove a walking route or audible playback. Diagnostic camera placement remains private verification behavior, not an in-game shortcut.

Also run:

- `cargo build --release --locked` for build-configuration changes; a locked release build and consistent build mode for graphics/performance closeout.
- Explicit `cargo test --locked --example <name>` for changed example diagnostics; ordinary `cargo test` does not run their example test harnesses.
- Relevant existing biome/cave/water probes and the proposed coast probe when generated-world behavior changes. Do not call a planned command verified before its example exists.
- Audio fake-output tests plus separately identified hardware/listening checks for milestone two.
- Isolated save/preferences paths for persistence tests; ordinary smoke must remain separate from the real progress slot.

Record failed and pre-existing checks accurately with focused failure context. Do not claim new coverage, FPS, latency, save safety, audio output, or physical input results without evidence.

## Completion record

Append a short record to the relevant milestone document, or link a focused evidence note if it becomes large:

```text
Task: Mx-yy
State: planned | implementing | automated-verified/awaiting-playtest | completed | blocked
Outcome:
Changed files / API or format changes:
Tests and exact commands:
Seeds / build mode / hardware (where relevant):
Evidence paths and observed results:
Unverified behavior / blockers:
Owner playtest decision (for milestone gates):
Next eligible task:
```

Mark the task checkbox only when its gate is satisfied. Automated completion may be recorded separately while manual acceptance is pending. Historical test/benchmark counts in other documents do not substitute for current evidence.

At each milestone gate, stop for the owner to assess the playable outcome. Adapt tuning or the next contract from findings before adding breadth; the roadmap is an ordered hypothesis about a good game, not a feature quota.

## Suggested task prompt

```text
Implement only TASK_ID from docs/roadmap/MILESTONE_FILE.md.
Read docs/roadmap/README.md, docs/roadmap/agent-workflow.md,
AGENTS.md, docs/architecture.md, and applicable skills first.
Verify prerequisite gates and the current working-tree baseline.
Preserve existing user work. Stay within this task's scope and non-goals.
Add focused tests, run the required local checks, and record evidence
in the milestone document. Report manual verification still needed.
Do not implement follow-up tasks, launch subagents, commit, or push
unless explicitly requested.
```

First task: **M1-00** in `docs/roadmap/01-coastal-wonders.md`.
