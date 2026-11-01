# M1 coastal expedition evidence

**State:** coastal implementation and automated/native verification delivered; **awaiting owner expedition playtest**. M1-05 remains unchecked. The owner authorized M2 and then M3 progression; audio/base implementation and CPU/GPU evidence are delivered, with experience acceptance still unreported. Fresh combined-renderer regressions are in [audio evidence](audio-evidence.md) and [base evidence](expedition-base-evidence.md), not inferred from these earlier captures. No automated result certifies physical input, landscape-led navigation, fun, or the owner's acceptance.

## Delivered behavior and safety

- Seeded catchment coast, broad layered mainland cliffs, cove/ramp/beach, five or six distinct grounded editable limestone stacks, and an actual connected supported arch.
- Ground/seabed caps, extra roof spans, marine wet occupancy/current, and visual waves are separate. Direct/full-column/batch/dense/resident/halo samples agree, including negative seams/catchments and extreme controls.
- Real under-arch sky exposure and edit-aware enclosure distinguish a short outdoor roof from caves. Mesh sky spill is bounded/worker-projected; camera reads use resident columns and invalidate on camera cell, edit revision, and 3×3 residency revisions. Original cave darkness/headlamp/underwater/cutaway behavior remains.
- Actual supported full-width/full-height walking/jumping routes from normal spawn, through the viewpoint/cove/beach, replayed entirely in both directions with the real controller. No flight, swimming, mining, placement, R, map/compass/markers, or diagnostic teleport is used to establish reachability.
- Generation version **2**, save format still **1**, append-only material IDs unchanged. Incompatible slots remain occupied and explain backup/confirmed fresh-world replacement. Smoke and unit tests use disabled or exclusively owned temporary storage; the player's slot was not read or changed.
- Existing uncommitted work was preserved. Cargo/toolchain/dependencies were not changed during M1 (M2 separately adds owner-approved CPAL); no commits/remotes/pushes or corporate tooling were added. MCP Rust LSP returned broken-pipe errors, then disappeared from the tool catalog; Cargo is the fresh compilation/diagnostic evidence.

## Reproduction commands

From `/Users/bentheunissen/Dev/RustCraft`:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --locked --example biome_probe
cargo test --locked --example cave_probe
cargo test --locked --example water_probe
cargo test --locked --example coast_probe
cargo test --locked generation_profile -- --ignored --nocapture
cargo test --locked coastal_route -- --nocapture
cargo build --release --locked
cargo run --release --locked --example coast_probe
cargo run --release --locked --example coast_probe -- --seed 42
cargo run --release --locked --example biome_probe
cargo run --release --locked --example cave_probe
cargo run --release --locked --example water_probe
cargo run --release --locked --example water_probe -- --seed 42
```

All capture/log files below are in this existing approved output directory:

```text
/private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/
```

Exact final native release commands (the script-free output directory above is not a new save location):

```sh
cargo run --release --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m1-05-final-default.png
cargo run --release --locked -- --smoke-test --seed 42 --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m1-05-final-42.png
cargo run --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m1-05-dev-comparison.png
cargo run --release --locked -- --offscreen-smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m1-05-offscreen.png
```

Commands were logged with `> <approved-output-directory>/<name>.log 2>&1`; ordinary interactive saves are not involved.

## Automated checks

- Baseline: **573** unit/integration/doc tests passed; no pre-existing failure, one intentionally ignored timing diagnostic. Save closeout: 574 passed.
- Coastal closeout **before concurrent renderer expansion**: **606** ordinary unit/integration/doc tests passed, no failure; the timing-only ignored diagnostic was also explicitly run and passed. Explicit examples: biome **7**, cave **19**, water **6**, coast **2** (34 total). Formatting, strict all-target locked Clippy, architecture boundaries/650-line ceiling, and locked release build passed at that state.
- Passing whole-project logs: `roadmap-m1-05-final-{tests,clippy}.log`; release `roadmap-m1-05-release-build.log`; example `roadmap-m1-05-{biome,cave,water,coast-example}-tests.log`; explicit timing `roadmap-m1-05-generation-timing.log`. Late `roadmap-m1-05-reviewed-{tests,clippy}.log` **failed** after concurrent new lighting work appeared: unresolved `super::super::IrradianceProbe` in `src/lighting/transport/tests.rs:121,123`, plus unused/dead new HDR/shadow/volume code. These are not coastal pass logs. The reference was subsequently changed by the concurrent work; separate latest results use `roadmap-m1-05-concurrent-{tests,clippy}.log` and are not assumed to pass.
- **M1 closeout combined-tree checks (historical):** strict Clippy passes in `roadmap-m1-05-concurrent-clippy.log`; `cargo test --locked` fails in the concurrently added `lighting::hdr::tests::exposure_adapts_smoothly_but_cannot_make_unlit_caves_bright` (`src/lighting/hdr.rs:87`, exact float assertion: `0.96000004` versus `0.96`; 540 library tests passed, 1 failed, 1 timing diagnostic ignored; integration suites did not run after that failure). Exact focused rerun: `cargo test --locked lighting::hdr::tests::exposure_adapts_smoothly_but_cannot_make_unlit_caves_bright -- --nocapture`. Combined formatting fails on new transport/renderer-lighting/postprocess/resources/shadow/frame files (`roadmap-m1-05-concurrent-fmt.log`); no broad formatter or fix was applied over concurrently changing user work. The replaced GPU renderer is **not** certified by earlier screenshots. These failures are retained, not counted as coastal completion.
- **M2-01 fresh revalidation:** concurrent work fixed the HDR assertion. Initial strict Clippy passes (`roadmap-m2-01-baseline-clippy.log`). The first full run passes library tests, then the architecture scanner mistakes external `glam::camera` for RustCraft's outward `camera` layer. A narrow scanner correction and regression tests pass the focused six-test suite (`roadmap-m2-01-architecture-fix-tests.log`); **625 whole-project tests pass**, one timing diagnostic ignored (`roadmap-m2-01-rechecked-tests.log`). That rerun contains new unfinished-renderer dead-code warnings, so the earlier Clippy pass is not a final result. Formatting still fails in concurrent files (`roadmap-m2-01-baseline-fmt.log`); new native evidence is still required.
- **M2 closeout revalidation:** fresh formatting/strict Clippy/release build and **702 tests + 34 example tests** pass. Final default native **332 presented frames** and seed-42 **324** pass coastal/save/ordinary water/cave/survival plus newly added shadow/probe/HDR/volumetric checks, with silent fake audio. Exact commands/logs and inspected new frames are in [audio evidence](audio-evidence.md). This is fresh combined-tree evidence, not a retroactive claim for earlier captures; physical expedition acceptance remains pending.
- Negative/failed cases include unknown/corrupt/incompatible saves, interrupted partial/failed writes, occupied-slot cancellation, cursor restoration, rejected edits/placements, missing/unloaded interaction, roof/enclosure edits, sky-spill seam/reload consistency, fake scenic/flat GPU readbacks, expired diagnostic deadlines, unsupported route endpoints, oversized bounds, illegal rises/drops, forced controller-replay obstruction, and absence of source residency.
- Generation-2 golden update is intentional and bounded: seed 42 fixture chunk `(2,-18)` contains 256 coastal columns; its golden changed `0x063bd5af55be6470 → 0xd8c40b8f927f7819`. Default/control fixtures are unchanged, and independent geometry/fluid/seam/order/control tests pass. Log: `roadmap-m1-02-golden-coverage.log`.

## Walking acceptance corpus (controlled simulation)

`roadmap-m1-04-sampled-routes.log` records the complete forward/reverse replay. Lengths are actual 3D step sums, not straight-line distance; height changes require ordinary jumping and reverse clearance.

| Seed | Approach / cove / beach lengths (blocks) | Height changes | Survey chunks / nodes | Controller updates |
| --- | --- | --- | --- | --- |
| default (`5932739705502318918`) | 304.3 / 58.5 / 64.0 | 37 / 23 / 0 | 131 / 24,810 | 23,428 |
| 42 | 382.6 / 70.6 / 63.0 | 28 / 28 / 0 | 162 / 32,449 | 27,625 |
| 0 | 273.7 / 62.9 / 52.0 | 33 / 24 / 0 | 149 / 26,579 | 21,421 |
| 1 | 221.6 / 64.4 / 67.0 | 40 / 25 / 0 | 105 / 18,854 | 19,887 |
| 7 | 348.3 / 84.3 / 58.0 | 32 / 32 / 0 | 185 / 38,717 | 26,711 |
| 2026 | 241.7 / 38.8 / 58.0 | 21 / 14 / 0 | 75 / 11,826 | 17,992 |

Additional seeds **2,3,8,17,99,12345,`u64::MAX`** pass the same route/controller rules. This sampled corpus is not a guarantee for every seed. The probe reports limits and fails instead of clearing terrain or teleporting a stuck player. The 5–10-minute discovery target remains a human exploration/observation tuning target; no empty travel padding was added.

## Native presentation and inspected GPU output

Hardware: **Apple M3 Max, Metal**, pinned Rust 1.99.1, macOS, display-reported **165 Hz** with VSync. Native smoke uses a bounded no-mouse-capture window and requires a completion frame at an actually changed size.

| Run | Result | Evidence |
| --- | --- | --- |
| Final default, release | **326 presented frames**, 289 streamed chunks, 150.0 sampled FPS; resize/completion passed | `roadmap-m1-05-final-native-default.log` |
| Final 42, release | **325 presented frames**, 289 chunks, 150.3 sampled FPS; resize/completion passed | `roadmap-m1-05-final-native-42.log` |
| Same-mode default comparison, optimized dev | **345 presented frames**, 289 chunks, 141.2 sampled FPS; resize/completion passed | `roadmap-m1-05-dev-comparison.log` |
| Explicit default offscreen, release | **330 GPU frames**, resize passed; **not native presentation** | `roadmap-m1-05-offscreen.log` |

Inspected real frame readbacks (not generated images):

- `<stem>-coast-land.png` and `-coast-cove.png`: mainland/cove, distinct separated stack silhouettes and grounded water contacts, no black marine horizon ring.
- `-coast-distance-2.png` / `-12.png`: bounded fog/load coverage; radius 2 intentionally loses distant detail, not a promise to see every stack. Radius 12 shows the longer coast.
- `-coast-arch.png` / `-coast-arch-underside.png`: real aperture/connected crown/piers, textured shaded underside, outdoor sky with headlamp off. The arch shore view is supported and included in controller replay; underside camera placement is visual evidence only.
- `-coast-stack-foundation.png`: visible solid stack descending into an actual supported marine bed. `-coast-arch-marine.png`: retained ocean optics and bed beneath the roof.
- `-coast-edited.png` / `-coast-resumed.png`: mining really changes rebuilt pixels; isolated save/resume preserves exact same-pose pixels, edited voxel, inventory/cursor reservation, one dropped stack, pose and settings. Eviction/reload keeps the edit. Existing Continue/replacement/failure menu scenes also pass.
- Existing cave rooms/headlamp-on/off, entrance, pools/lava, morphology, phasing/restoration, all surface water families, submerged palettes/reverse draw pass/surfacing, current/swimming, survival cracks/pickups/held blocks/inventory/placement/menu/resize scenes pass as regressions. Inspected seed-42 entrance, dark rocky cave, ocean, coastal views and resumed output.

Default / 42 scene contrast: land **0.144 / 0.141**, cove **0.106 / 0.108**, arch **0.154 / 0.162**, underside **0.135 / 0.110**, stack foundation **0.054 / 0.046**. Marine underside is low-contrast by underwater attenuation (0.010), supplemented by direct support/water tests and explicit foundation frame inspection. HUD is absent from coastal acceptance statistics.

### Failures retained, not counted as passes

- During final closeout, new `src/lighting/{hdr,materials,shadows,transport,volume}.rs`, `src/renderer/{lighting,postprocess,shadows}.rs`, renderer resources/passes and shaders appeared and continued changing outside the coastal task (observed modification times around 22:44–22:49). They were preserved, not reverted, disabled, repaired, or treated as roadmap authorization for a broad lighting rewrite. Earlier native screenshots/metrics describe the verified coastal renderer, not this newly changing renderer. Its final combined-tree Cargo/GPU acceptance must be independently re-established; see the late-check logs above.

- Initial new tests exposed a freshwater-bank intrusion; complete freshwater-apron exclusion fixed it, retaining strict ordinary-bank checks outside intentional coast/lips. New steep cliffs/stacks are intentionally not advertised as gradual swim exits everywhere; actual cove routes and ordinary shores remain tested.
- Initial seed-42 diagnostic view was blocked by a ledge; actual line-of-sight candidate validation fixed it. A clear feet cell alone was insufficient.
- Arch placement initially touched a stack and had a dry/filled opening; offshore stack spacing and explicit capped aperture bed fixed the real geometry. Connected-component and full sampling/fluid tests remain independent of labels.
- Several intermediate native dev launches timed out waiting for window/GPU presentation. Their logs remain failures (`roadmap-m1-02-native-42.log`, `roadmap-m1-02-view.log`); explicit offscreen evidence was used separately, and final native release/dev runs above now pass. Read-only session metadata alone was never counted as presentation proof.
- A coastal interaction fixture initially placed the camera inside rock (zero-normal target), and a fixture initially excluded its own lower support from a connectivity search; full-body/exterior-target validation and correct survey bounds fixed the fixtures. The final mining→collection→placement test conserves the block; no failed assertion was turned into acceptance.

## Current workload/performance samples

Same seeds/hardware/locked **release** water-probe fixture: 55 chunks each, identical geometry before/after. Baseline and final samples were not isolated from all other CPU/GPU work, so these are observed bounded costs, not definitive regressions or throughput guarantees.

| Seed / 55-chunk water workload | Baseline generation / mesh ms per chunk | M1 closeout generation / mesh | Opaque / water vertices (unchanged) |
| --- | --- | --- | --- |
| default | 1.223 / 1.148 | 1.232 / 1.432 | 416,140 / 41,128 |
| 42 | 1.222 / 1.138 | 1.205 / 1.340 | 450,232 / 36,420 |

Logs: `roadmap-baseline-water-{default,42}.log`, `roadmap-m1-05-water-{default,42}.log`. New bounded roof-halo setup adds CPU mesh work; residency/queues remain bounded. The old terrain workload is unchanged, not silently replaced by smaller meshes to improve a benchmark.

Coast probe is new work, not comparable to the old river/ocean fixture. Default: 50 geometry chunks, 8,177 supported marine columns, generation/mesh **1.161 / 1.312 ms per chunk**, 431,900 opaque / 32,708 water vertices; full route work **250.305 ms**. Seed 42: 46 chunks, 6,917 marine columns, generation/mesh **1.210 / 1.263 ms per chunk**, 414,768 / 27,720 vertices; full route work **337.135 ms**. Exact final default metrics are recorded in `roadmap-m1-05-probe-default.log` (the CPU probe ran during concurrent renderer changes and emitted unused new-lighting warnings; it still validates CPU geometry/routes, not the changing renderer). Diagnostic search/replay time is separate from generation/meshing and never an interactive frame callback.

Optimized-dev default, same ordinary ocean GPU pose: baseline render+completion median/p95 **1.00 / 1.89 ms**; closeout **0.99 / 1.98 ms**, both 32 samples. Native smoke baseline **331 presented frames / 145.3 sampled FPS**; closeout **345 / 141.2** (289 chunks, same motion). Extra scenes and workload/context prohibit a precise interactive FPS claim.

New coastal fixed land pose, release, 32 render+completion samples: default median/p95 **1.14 / 2.33 ms**, 42 **1.12 / 1.81 ms**. These include frame completion, not physical input latency or an unlimited-world benchmark. No older research result is used as current evidence.

## Owner expedition gate — needed to continue

1. Back up any occupied local slot while the game is closed. Generation 2 intentionally requires a fresh development world for older versions; do **not** replace the only slot unless you choose to.
2. Run `cargo run --release --locked`, intentionally start/confirm a new Survival world at a chosen acceptance seed, and spend roughly 30 minutes exploring from the normal spawn.
3. Reach the coastal vista using walking/jumping and landscape cues; descend to the cove, view/explore the arch, and return through the landscape without flight or R. Ordinary hand mining/building is still available but not needed for reachability.
4. Save/quit and Continue; confirm position/edits/finds resume. Exercise physical mouse capture/look, keyboard/jumping, inventory/pause/focus transitions.
5. Report whether silhouettes, navigation, return route, shoreline feel, and the destination are memorable enough to accept M1; note repetitive shapes, frustrating steps, or a too-obscure approach.

**Owner physical playtest decision: not provided.** Later progression/explicit CPAL approval and “Sounds good, you can proceed” authorized M2/M3 implementation, now delivered. They do not certify a physical expedition/listening/base-building report or authorize automatic real-save replacement. Remaining experience/native acceptance is documented in the current evidence notes.
