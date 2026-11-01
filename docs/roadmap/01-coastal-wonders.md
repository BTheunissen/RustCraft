# Milestone 1: a coast worth finding

**Outcome:** a fresh-world player can discover a walkable coastal region inspired by Victoria's Twelve Apostles: layered limestone cliffs, distinct offshore sea stacks, an accessible cove, and a coastal arch. The player can explore, return through the landscape, and resume the world later.

Depends on the inspected explorer/survival foundation. Task state and fresh evidence are recorded below. Follow the [roadmap](README.md) and [agent workflow](agent-workflow.md).

## Experience and scope

The landmark is a region, not a pile of decorative pillars. Design the relationship between mainland cliffs, headlands, cove, seabed, open water, and detached stacks. Use the seed to vary orientation, proportions, spacing, and surface detail while preserving recognizable silhouettes and a viable approach.

- Use ordinary editable voxels and existing limestone/sediment materials first.
- Include one coastal arch with a real opening and solid supports, visible from accessible ground. An arch is not merely a painted texture or a heightfield saddle.
- Provide a natural walking approach to a land viewpoint and a reversible walking route to a cove/beach.
- Let skyline changes, headlands, shoreline shape, and changing rock exposure guide exploration. No new navigation UI or special route-marker items.
- Keep survival and creative rules unchanged. Ordinary walking/jumping must suffice for the primary visit; flight is optional exploration, not an acceptance shortcut.
- Suggest weathering through generated shape and layering, not simulated erosion. No extra sea-cave network, tides, boats, wildlife, or collapse mechanics.

### Reference, not replica

[Parks Victoria: Fly over the Twelve Apostles](https://www.parks.vic.gov.au/get-into-nature/conservation-and-science/our-amazing-diversity/virtual-exploration/fly-over-the-twelve-apostles) describes limestone cliffs and detached stacks, coastal caves/arches, and horizontal sediment layers. [Parks Victoria: Twelve Apostles](https://www.parks.vic.gov.au/places-to-see/parks/port-campbell-national-park/things-to-do/twelve-apostles) provides further coastal context. Reviewed during planning on 31 October 2026.

Adapt the visual vocabulary to RustCraft's block scale. Do not reproduce exact geography, require exactly twelve stacks, import protected assets, or present the simplified generator as a geological simulation. Create original terrain and textures.

## Technical starting point

Inspect these before editing:

- `src/world/generation/hydrology.rs` and `hydrology/{catchment,features,sampling}.rs`: seeded, bounded catchments already couple drainage, beds, water, and an island.
- `src/world/generation/{terrain,chunks,surface}.rs`: direct, full-column, batched, and dense generation paths must agree.
- `src/world/generation/underground/column.rs`: bedrock, cave carving, and hydrology sealing have explicit precedence.
- `src/world/chunk.rs`: resident water samples must describe actual water voxels.
- `src/game/environment.rs`: its current below-column-cap heuristic must be reconsidered for arch overhangs; a roof's top height alone must not turn an open daylight coast into a dark cave.
- `src/mesh/`, `src/streaming/`, `src/playtest/`: geometry, edit-aware streaming, and bounded graphics verification.

Proposed ownership: a private `src/world/generation/coastal.rs` / `coastal/` subsystem, integrated with the existing hydrology and generation facades. These paths are proposals, not existing files. Do not introduce a generic wonder registry or replace the generator.

## Shared contracts to freeze in M1-01

1. **Coordinates:** +Y up; 16-block X/Z chunks; world Y is `-64..80` with exclusive top. Feet and eye positions remain distinct. Promote feature/halo coordinate arithmetic before overflow-prone products.
2. **Deterministic roots:** choose bounded coastal feature roots from seed and marine catchment coordinates, independent of generation order. Avoid river mouths, freshwater basins, and the dry spawn. Include full feature/variation bounds in query halos.
3. **Sampling:** one feature definition and precedence contract serves direct voxels, projected columns, batches, dense chunks, procedural fallback, and mesh halos. Do not add a chunk-only decoration that direct collision/daylight sampling cannot see.
4. **Hydrology:** stacks rise from a supported seabed. Solid rock, actual wet occupancy, mean waterline, current, and marine metadata agree. Visual waves remain separate from occupancy; mining still does not cause water spreading.
5. **Arch occupancy:** explicitly represent rock above an opening. Do not use the crown height as the seabed or invent water inside solid supports. Keep the first arch crown/opening above the mean marine surface so no multiple/disconnected water-span model is required. Ocean below an overhang retains marine metadata rather than falling back to a freshwater cave-pool classification.
6. **Generation precedence:** preserve bedrock and basin seals; prevent unrelated cave carving or trees from erasing the designed stack/arch silhouette or route. This is generation precedence, not protection against player mining. Edits apply after generation as before.
7. **Environment/light:** under-arch daylight follows actual occlusion/exposure and resident edits, not a simplistic tallest-column classification. Preserve real cave darkness, headlamp behavior, underwater transitions, and cutaway behavior.
8. **Bounded work:** project nearby features once per region/column rather than rebuilding them for every voxel. No terrain generation or unbounded landmark search on the render thread. CPU/GPU residency remains bounded.

### Initial tuning defaults — verify, then adjust with evidence

- A compact landmark segment, initially about **120–220 blocks along the coast**, so nearby views fit the existing streaming/render-distance model.
- Roughly **3–7 main stacks**, with irregular positions and distinct profiles; do not force all stacks into every camera view.
- Main stack heights initially **16–36 blocks above mean sea level**, with grounded bases and enough margin below the world ceiling.
- At least three visibly separate main stack bodies above the waterline and at least two noticeably different heights/profiles in the accepted early region.
- An arch opening initially at least **4 blocks wide and 3 blocks high**, with two grounded supports and a connected crown. It must read as an opening from the shore; its top need not be a traversable bridge.
- A standing-player route with full body clearance to the viewpoint and cove, using the existing controller's walking/jumping abilities. No required swimming, mining, placement, flight, or return-to-spawn teleport.

These numbers are design starting points, not measurements or universal seed guarantees. Test and inspect them at default render distance, then check graceful behavior at supported distances 2 and 12. Do not promise a distant silhouette beyond loaded/fogged terrain.

## Task queue

### M1-00 — validate the baseline and close out existing saves

- [x] Complete
- **Dependencies:** none. **Scope:** existing save/session tests, save UI/documentation, and baseline evidence.
- Read the current working tree, run the normal local checks, and record pre-existing failures rather than assuming the foundation is verified.
- Validate existing snapshot round trips, edited chunk reload, inventory/pickup preservation, Continue, autosave, save-on-leave/exit, and explicit replacement confirmation. Reuse existing tests; add only missing high-value cases.
- Confirm interrupted/failed writes preserve committed data; corrupt/incompatible slots stay occupied and are never silently reset. Ensure an incompatible development world produces an actionable explanation to the player.
- Use isolated test slots and preserve the smoke mode's disabled persistence. Never exercise replacement against the owner's real save without explicit consent.
- Reconcile the session-only/no-save descriptions in `README.md` and `docs/architecture.md` with verified behavior. Label older world/water documents' scope statements as historical where needed; do not imply their old test counts validate this baseline.
- **Gate:** persistence readiness is demonstrated or blockers are explicit. Do not rebuild saves, add multiple slots, or repair unrelated work under this task.

### M1-01 — freeze the coastal integration contract

- [x] Complete
- **Dependencies:** M1-00. **Scope:** this brief plus a short technical contract if needed; no broad implementation yet.
- Record exact feature bounds/halos, seed inputs, catchment placement, material precedence, arch occupancy, water metadata, and environment classification approach.
- Resolve how the current column/water representation will accommodate a roof over open air without divergent direct/dense sampling or phantom water. Prefer a bounded coastal occupancy layer over a global fluid-storage redesign.
- Define early-site selection without recursive calls between spawn, coast selection, and decorated terrain sampling. Define a bounded route survey and stable diagnostic site selection.
- Record the generation-version change and which golden fingerprints will intentionally change. No backward generator or save migration is required.
- **Gate:** the contracts above have one coherent implementation plan with testable invariants. If the chosen representation needs a world-wide rewrite, reduce its complexity or discuss the scope change first.

### M1-02 — generate the cliff-and-stack region

- [x] Complete
- **Dependencies:** M1-01. **Scope:** coastal generation, hydrology integration, generation facades, targeted CPU tests/diagnostics, generation version.
- Add the coherent cliff, cove, seabed, and sea-stack foundation before adding the arch. Preserve estuary connectivity and freshwater systems.
- Generate several rooted silhouettes rather than uniform cylinders; use limestone exposure/layering without new material IDs unless existing materials demonstrably cannot achieve the design.
- Exercise all sampling paths and cross-chunk feature halos, including negative coordinates and different chunk orders. Use deterministic seed variation rather than a fixed scene near world origin.
- Measure actual separated stack geometry, foundations, wet/dry neighbors, finite water metadata, and unaffected control locations outside feature bounds. Enum labels do not establish these properties.
- Bump the generation version for the intentional terrain change; explain intentional golden updates.
- **Gate:** a CPU-valid, GPU-visible cliffs/stacks slice exists. Inspect a land view early before investing in the arch; do not claim the full milestone yet.

### M1-03 — add the coastal arch and integrate overhang behavior

- [x] Complete
- **Dependencies:** M1-02. **Scope:** coastal occupancy/carving, affected water/environment/daylight paths, mesh correctness, targeted tests.
- Add a supported rock crown with a real through-opening and clear shore-side view, varying its shape within the frozen bounds.
- Keep the opening above the mean waterline and retain correct marine water below it. Verify no water samples in rock supports or detached unsupported wet cells.
- Check direct, full-column, batched, and resident voxels/fluids agree at supports, opening, crown, and positive/negative seams. Do not simply raise a column height and fill below it.
- Test actual sky occlusion under/around the arch; preserve outdoor atmosphere where appropriate, under-roof shading, existing dark caves, and headlamp behavior. Check player-made roof/edit invalidation as a regression.
- Verify ordinary meshing exposes the opening and underside with correct winding and no water/rock overlap. Mining/placing at the site still uses normal survival rules.
- **Gate:** the arch is structurally and visually present, with coherent collision, water, and lighting—not just a successful shader submission.

### M1-04 — make the first coast reachable and readable

- [x] Complete
- **Dependencies:** M1-03. **Scope:** bounded early-site/approach tuning, spawn safeguards, diagnostic route checks, traversal tests.
- Ensure an early region can be reached from a normal fresh spawn and approached through land cues. Do not move the player to a diagnostic camera or add an in-game coast teleport.
- Require a reversible route to a viewpoint and cove. Straight-line distance, flat height samples, or a path through a too-narrow gap are insufficient evidence.
- Survey actual support, full body width/height, headroom, legal rises/jumps, and cliffs/gaps. Replay critical approach/shore sections using the real player controller.
- The initial acceptance corpus is **default seed, 42, 0, 1, 7, and 2026**. Show a valid early route for each; use additional sampled seeds to look for placement/path failures. This corpus is not proof for every possible seed.
- Add a bounded CPU diagnostic, proposed `examples/coast_probe.rs`, reporting sites, bounds, geometry/fluids, route validity/length, and generation/meshing cost. Diagnostic routes/coordinates are not navigation UI.
- Keep the 5–10-minute discovery target a human tuning goal. Existing coasts are relatively near spawn; do not stretch them thousands of empty blocks solely to meet a stopwatch target.
- **Gate:** accepted seeds have genuinely traversable approaches and a coast that offers enough visible cues to try without a HUD destination.

### M1-05 — prove the coastal expedition and close the milestone

- [ ] Complete
- **State:** coastal automated/native-verified; **awaiting owner expedition playtest**. Late concurrent renderer changes require separate combined-tree revalidation. Full evidence and the manual gate are in [`coastal-evidence.md`](coastal-evidence.md).
- **Dependencies:** M1-04. **Scope:** bounded coastal playtest scenarios, regression evidence, current docs, owner playtest.
- Add bounded GPU-readback scenes for the land vista, accessible cove, stack/seabed connection, arch aperture/underside, and underwater marine view. Proposed implementation location: private `src/playtest/coastal.rs` / `coastal/`.
- Extend native/offscreen smoke checks without using diagnostic placement as proof of walkability. Check the real edited site's remeshing/reload and save round trip through isolated storage.
- Regress ordinary surface biomes, caves/entrances, headlamp on/off, phasing/restoration, freshwater and marine water, swimming/shore exits, mining, inventory, placement, and menu/capture transitions.
- Run locked fmt, Clippy, tests, affected example tests (including the new coast probe), existing biome/cave/water diagnostics as relevant, and a native smoke screenshot pass. Inspect GPU frames; report offscreen-only results separately.
- Compare generation/meshing and frame-cost samples before/after using consistent seeds, coordinates, build mode, and hardware. Record output geometry/work changes; do not quote older research benchmarks as current results.
- **Owner playtest:** start a fresh world intentionally; reach a rewarding coastal view by walking, explore the cove/arch, return without flight or R, save/quit, and Continue. Assess silhouettes, route readability, repetitive shapes, shoreline frustration, and whether the trip was memorable. Preserve the player's explicit consent for replacing any occupied save.
- **Gate:** automated and visual evidence passes, physical input limitations are stated, and the owner accepts the expedition before milestone two begins.

## Completion evidence

Record exact commands, seeds, diagnostic observations, screenshot/report paths, save version changes, and the owner's playtest outcome using the [handoff template](agent-workflow.md#completion-record). Mark any untested physical journey as **awaiting owner playtest**, not completed gameplay acceptance.

### M1-00 — completed, 31 October 2026

- **Outcome:** existing saves retained; incompatible format/generation failures now have a distinct in-game notice and actionable terminal explanation. Occupied slots still require explicit replacement, with Cancel selected. No progress format/version change in this task.
- **Changes:** `src/save/format.rs`, `src/app/{persistence,session}.rs`, `src/hud/{interface,menus}.rs`, focused version/session/menu tests, private persistence GPU scene, and current/historical documentation. Source inspection confirmed README/architecture already described local saves; older research scope statements are now labeled historical.
- **Baseline:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`: 573 passed, 0 failed, one ignored timing-only diagnostic. No pre-existing failures.
- **After closeout:** the same commands pass: 574 passed, 0 failed, one ignored timing diagnostic. Tests cover snapshot/slot round trips, edited first-stream reload, inventory/cursor/pickup preservation, autosave during pause/inventory, leave/native exit, interrupted writes, corrupt/incompatible occupation, and explicit replacement/cancel.
- **Native command:** `cargo run --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m1-00.png`. Apple M3 Max / Metal, optimized dev, default seed; passed with 365 presented frames, 289 chunks, 149.8 sampled FPS, changed-size presentation. Inspected `roadmap-m1-00-save-incompatible.png`, baseline restored-pickup and replacement-confirmation frames. Private-slot same-pose GPU pixels passed, as did existing cave/headlamp/cutaway/water/swimming/survival checks. The real local save was not read or modified.
- **Evidence:** approved temporary directory above, `roadmap-baseline-{tests,clippy,native}.log`, `roadmap-m1-00-{tests,clippy,native}.log`, and captured PNGs. Current release water-probe baseline: default / 42 each 55 chunks, generation 1.223 / 1.222 ms per chunk, meshing 1.148 / 1.138 ms per chunk; simultaneous process samples, not isolated timing guarantees. Full geometry totals are in `roadmap-baseline-water-{default,42}.log`.
- **Limitations:** MCP Rust LSP returned broken-pipe errors, then was removed from the available tool catalog. Cargo is the fresh compiler evidence. Physical keyboard/mouse acceptance remains unverified, not a persistence-readiness blocker.
- **Next eligible task:** M1-01.

### M1-01 — completed, 31 October 2026

- **Outcome:** the bounded coastal contract is frozen in [`coastal-contract.md`](coastal-contract.md). No generic landmark framework, new material IDs, or global fluid rewrite.
- **Validation:** reviewed the direct/full-column/batch/dense/mesh/water/edited-environment call paths and existing overflow, bed seal, tree halo, save safety, and roof-invalidation tests. M1-00 checks above remain current; this checkpoint changes documentation only.
- **Generation change:** version 1 → 2 will land with M1-02. Unmodified coordinates retain their golden behavior; changed coastal fingerprints require recorded intentional updates and independent geometry/seam tests.
- **Owner gate:** this is a technical contract, not expedition acceptance. The M1-05 owner walking/playtest gate remains required before M2.
- **Next eligible task:** M1-02.

### M1-02 — completed, 31 October 2026

- **Outcome:** private bounded coast projection supplies original layered limestone cliffs/headlands, a beach/cove ramp, supported seabed, and five/six independently varied stacks per marine catchment. All generation paths share its column projection. Freshwater footprints/aprons are excluded with a smooth blend. No new materials, UI destinations, or changed survival rules.
- **Changes:** `src/world/coastal.rs`, `src/world/generation/coastal.rs` and its sampling/sites/tests, hydrology roots/profiles, direct/batch/dense/material precedence, tree exclusion, `examples/coast_probe.rs`, and private `src/playtest/coastal.rs`. Generation **1 → 2**, file format still 1. Seed-42 golden `0x063bd5af55be6470 → 0xd8c40b8f927f7819`: only fixture chunk `(2,-18)` has coastal columns (256); default's six fixture chunks have zero coastal columns and its golden is unchanged. Independent stack/water/path-equivalence/control tests remain.
- **Commands:** `cargo fmt --all -- --check`; `cargo clippy --all-targets --locked -- -D warnings`; `cargo test --locked` (582 passed, 0 failed, one ignored timing-only diagnostic); `cargo test --locked --example coast_probe` (2 passed); `cargo test --locked --example water_probe` (6 passed); `cargo test --locked intentional_golden_changes -- --nocapture` reports the intersecting columns. Logs use `roadmap-m1-02-*` in the approved temporary directory recorded above.
- **Geometry:** six acceptance seeds have five/six distinct above-water rock components, at least three differing heights, connected solid foundations, finite marine water, sealed wet neighbors, and clear dry view/cove feet. Direct/full/batch/dense sampling and fluid metadata agree across positive/negative seams and reverse chunk order; negative catchments and extreme controls pass. Early diagnostic selection now checks actual line of sight to at least three stacks after inspecting a seed-42 ledge-obstructed frame.
- **GPU:** initial `cargo run --locked -- --smoke-test --screenshot …/roadmap-m1-02.png` presented 456 frames, including land/cove captures, on Apple M3 Max/Metal. Inspected final `roadmap-m1-02-final-coast-land.png`, `roadmap-m1-02-coast-cove.png`, `roadmap-m1-02-view-42-coast-land.png`, and seed-42 cove. Final default/42 offscreen runs passed with 365/333 GPU frames. Later native runs timed out waiting for window/GPU presentation (no false native pass); final viewpoint tuning is offscreen-only. The dark below-horizon coastal ring is recorded for the next occlusion/overhang integration task, not ignored as a visual success.
- **Probe:** `cargo run --release --locked --example coast_probe [-- --seed 42]`: default 48 chunks, 5,831 supported marine columns, 1.286 ms generation / 1.053 ms meshing per chunk, 431,072 opaque / 23,324 water vertices; 42: 50 chunks, 6,568 columns, 1.151 / 0.988 ms, 445,424 / 26,324 vertices. Sampled concurrent processes, not performance guarantees. Existing default water-probe geometry was unchanged (416,140/41,128 vertices); fresh generation/meshing 1.184/1.149 ms per chunk versus baseline 1.223/1.148.
- **Next eligible task:** M1-03. Route/arch/full expedition and owner acceptance are not claimed yet.

### M1-03 — completed, 31 October 2026

- **Outcome:** explicit roof spans create a real limestone through-arch above marine water; two piers and crown are six-neighbor connected and grounded. All six acceptance seeds have at least 7×15 sampled through-air columns above the sea, with a minimum three-block opening height. Seabed caps remain distinct from the roof; no water in supports/crown and no freshwater fallback under the roof. Stack roots were moved farther offshore to prevent arch/stack merging; the dry shoreline ribbon reaches a supported arch view.
- **Changes:** private coastal arch projection/readouts/tests; resident roof metadata; direct/dense/batch/halo precedence; bounded edit-aware camera/mesh sky spill; actual-cavity lower sky fallback; exposed-overhang opaque fog payload; original mesh layout/voxel IDs unchanged. Generation remains 2/file format 1. The generation-2 goldens still pass after these refinements.
- **Checks:** `cargo fmt --all -- --check`; `cargo clippy --all-targets --locked -- -D warnings`; `cargo test --locked` (597 passed, 0 failed; one timing diagnostic ignored); `cargo test --locked --example coast_probe` (2 passed). Fresh logs `roadmap-m1-03-complete-{tests,clippy}.log` and focused `roadmap-m1-03-interactions4.log` in the approved temporary directory. Added tests exercise aperture/support/crown occupancy, full columns/batches/resident fluids and seams, real sky occlusion, enclosed/added-roof edits and invalidation, surfacing, ordinary mining/placement/drop/reload, mesh winding, underside spill, exact reload vertices, and seam fields. Existing cave/headlamp/cutaway/save/water regression suites pass.
- **GPU:** default and 42 `cargo run --locked -- --offscreen-smoke-test [--seed 42] --screenshot …/roadmap-m1-03-{lit,42}.png` pass. Inspected arch shore view, rounded underside, and submerged marine bed; open arch remains outdoor with lamp off. The coastal black horizon ring is gone in the inspected land frame. Final default readback: land/cove/arch/underside/marine contrast 0.144/0.106/0.154/0.135/0.010, 343 GPU frames; no claim of native presentation on this locked/unavailable desktop. Earlier M1-02 native evidence is separate. Physical input and expedition acceptance remain pending.
- **Probe:** default release `cargo run --release --locked --example coast_probe`: actual aperture `[280.5,21,-23.5]`, roof `[27,38]`, grounded piers `[275,20,-30]` / `[285,20,-18]`; 50 chunks, 8,177 supported marine columns, 1.270 ms generation / 1.485 ms meshing per chunk, opaque/water vertices 431,900/32,708. Fresh scene work differs from the earlier stack-only probe; these are bounded samples, not identical-work performance claims.
- **Next eligible task:** M1-04. Diagnostic shore poses do not certify spawn-to-coast walking.

### M1-04 — completed, 31 October 2026

- **Outcome:** normal fresh spawns have reversible walking/jumping routes to the clear land vista, cove, and arch shore view. No spawn change, new navigation UI, in-game teleport, trail markers, or route excavation. Broad cove ramp and the natural beach ribbon guide the approach. Deterministic diagnostic routes stay outside interactive gameplay.
- **Changes:** `src/game/coastal_route.rs` with private bounded survey/A*/controller replay modules/tests; public diagnostic `survey_coastal_routes`; `examples/coast_probe.rs` prints actual route lengths, rises, chunks/nodes/update counts and costs. Real dense voxels determine support/headroom/body clearance. All one-block edges are checked reversibly, including jump headroom; every leg is replayed forward and reverse through `Player::update_with_water` with full-body, no-water/no-flight assertions.
- **Commands:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` (602 passed, 0 failed, one timing diagnostic ignored); `cargo test --locked coastal_route -- --nocapture` (5 passed); `cargo test --locked --example coast_probe` (2 passed); `cargo run --release --locked --example coast_probe`. Logs `roadmap-m1-04-{complete-tests,sampled-clippy,sampled-routes,complete-probe-tests,probe-default}.log` in the approved temp directory.
- **Accepted corpus (approach / cove descent / beach route length in blocks):** default 304.3 / 58.5 / 64.0; 42 382.6 / 70.6 / 63.0; 0 273.7 / 62.9 / 52.0; 1 221.6 / 64.4 / 67.0; 7 348.3 / 84.3 / 58.0; 2026 241.7 / 38.8 / 58.0. Surveys use 75–185 chunks and 11,826–38,717 expanded nodes; full forward/reverse controller verification uses 17,992–27,625 updates per seed. Additional sampled seeds 2,3,8,17,99,12345,`u64::MAX` also pass. This is not a universal all-seed guarantee or a human navigation/readability result.
- **Probe performance:** default release diagnostic route survey/replay 358.704 ms, 131 transient CPU chunks, 24,810 nodes; work runs only in the explicit probe/test, not a frame/audio callback. Geometry timings are separate and include sampled workload/hardware caveats. The 5–10-minute discovery target remains a human tuning target; no empty distance was added.
- **Unverified:** physical input, route readability/fun, visual grace at radii 2/12, and full edited/save capstone remain for M1-05. Read-only console metadata shows a logged-in on-console session, not proof that native Metal presentation is currently available; earlier native timeouts are retained as failures.
- **Next eligible task:** M1-05, then stop for owner expedition acceptance before M2.

### M1-05 — automated/native-verified; awaiting owner playtest

- **Outcome:** default/42 bounded native GPU scenes verify land/cove, distances 2/12, actual arch/aperture/underside, stack–seabed connection, marine view, ordinary coastal mining/remesh, private-slot inventory/cursor/pickup/pose round trip, exact resumed pixels, and edited eviction/reload. Existing cave/headlamp/phasing/freshwater/marine/swimming/survival/menu/capture/save-safety checks remain intact.
- **Changes:** private coastal GPU and persistence scenes plus focused failure-path tests; resident-only revision-aware camera exposure hardening; current README/architecture/roadmap descriptions and concise validated skill guidance. No Cargo/dependency changes, commits, real-slot reads/writes, or additional gameplay systems.
- **Evidence:** [`coastal-evidence.md`](coastal-evidence.md) records exact commands, seed corpus, build/hardware, native versus offscreen results, scene inspections, current performance/work samples, and earlier failed/repaired checks. Approved temporary log/PNG paths are explicit.
- **Owner gate:** physical mouse/keyboard, landscape-led navigation, the roughly 30-minute expedition, return without flight/R, save/quit/Continue, and subjective experience remain unverified by the owner. M1-05 stays unchecked. The later “Looks great, keep going” authorizes preparing M2-01 while retaining this unverified physical gate; it does not approve an exact dependency or certify the expedition.
- **Late shared-tree change:** new HDR/shadow/probe lighting/rendering modules and shader/pipeline edits appeared from concurrent work during closeout. The earlier 606-test/Clippy/native evidence remains valid for the coastal state then checked, not the changing combined renderer. Reviewed tests/Clippy failed on a new unresolved probe type and unused new lighting code; preserved without removal or unrelated repair. Latest separate revalidation is recorded in the evidence note.
- **M1 closeout combined-tree result (historical):** Clippy passes; tests fail in new `lighting::hdr::tests::exposure_adapts_smoothly_but_cannot_make_unlit_caves_bright` at `src/lighting/hdr.rs:87` (exact `0.96000004` vs `0.96`), with 540 other library tests passing. Combined formatting fails in concurrent transport/rendering files. Fresh M2-01 revalidation is recorded in the evidence note; the replaced GPU pipelines still need separate verification rather than relying on old captures.
- **M2 closeout update:** combined checks now pass (**702 ordinary + 34 example tests**, fmt/Clippy/release) with fresh default/42 native coastal/save/rendering regressions. See [`audio-evidence.md`](audio-evidence.md), which separately records audio hardware versus human listening. Neither physical M1 nor listening M2 acceptance has been invented.
