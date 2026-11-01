# M3 expedition base evidence

**State: 1 November 2026.** Selected feature implementation and CPU/offscreen GPU
capstone delivered. M3-01–07 complete; **M3-08 awaiting owner playtest** and
current native presentation. The owner authorized progression (“Sounds good,
you can proceed”) without a reported M1 physical expedition or M2 listening pass;
those experience gates remain unverified. No additional scope has been added.

## Delivered loop

- Block/tool items with independent stable tags/IDs and per-item caps (blocks
  64, permanent tools one); selected hotbar equips, never places/consumes tools.
  Original HUD and held/drop silhouettes distinguish pickaxe from axe.
- Left whole move/merge/swap, right half-up/single placement, reserved-source
  conservation. Source slots and held quantities are excluded from automatic
  insertion/crafting. Close/focus/save restores safely; full/incompatible actions
  do not drop or delete overflow. CPU totals survive 8,000 player and 10,000
  cross-inventory mixed operations, including tools and concurrent insertion.
- Planks/Dressed Limestone are editable full cubes with original atlas/icon/
  held/drop/phase mappings, unchanged terrain generation/goldens/legacy art.
- Six live transactional recipes: Wood→4 Planks; Limestone→Dressed Limestone;
  4 Planks+3 Stone→Pickaxe; 4 Planks+2 Stone→Axe; 8 Planks→Crate;
  2 Planks+Crystal→Lantern. Capacity is planned after costs; invalid/full/missing
  crafts leave exact inventory state unchanged. Paused screen/button/C/keyboard
  navigation shares physical-pixel hit layout. No placeholders/grid/unlocks.
- Pickaxe is 3× faster on rock/ores/crystal/dressed stone; axe on wood/planks/
  crate; unsuitable material keeps hand speed. No durability/access tiers.
  Tool switches (including away/back between frames), targets, release/pause
  reset damage; f64 accumulated work avoids completion rounding stalls. One
  normal block drop/success, no extra tool consumption or duplicate audio.
- 18-slot solid Storage Crate with explicit right-click open vs Shift-placement,
  reach/resident/target validation, one reserved player/crate cursor, at most
  4,096 sparse records. Nonempty removal shows an error; empty removal drops one
  crate and removes metadata. Contents and held quantities survive eviction,
  saved negative seams, snapshots/restart, full transfers, and failed saves.
- Permanent warm Lantern with at most eight voxel-step attenuation/solid
  occlusion/water weakening. CPU worker discovers sources only in a fixed 3×3
  sparse edit halo, generates at most 32×32×144 occupancy, processes eight bounded
  integer levels, and uploads separate local RGB. No extra field for source-free
  chunks, global per-frame edit scan, unbounded emitter/queue, or long-range probe
  emission. Existing revision snapshots reject stale remeshes; negative chunk
  seams, wall edits, source removal, and eviction/reload remain deterministic.

Peaceful scope is preserved: headlamp/hand mining/return-to-spawn retained,
Creative remains exploration-only; no health/combat/hunger/oxygen penalty,
durability/fuel/furnace, navigation systems, wildlife/weather, extra packages,
frameworks, CI/integrations, or commits/pushes.

## Files and compatibility

`src/game/{item,inventory,crafting,interaction,pickup,storage,snapshot}.rs` and
cohesive submodules own CPU transactions/state. `src/save/format.rs` validates
explicit tags and bounded records; app screens/input/persistence manage pauses
and modifier intent. `src/hud/{inventory,crafting,storage,icons}.rs` paints borrowed
readouts; renderer survival owns bounded presentation. `src/world/{material,edits}.rs`,
`src/atlas/building.rs`, `src/mesh/{materials,lanterns}.rs`,
`src/lighting/lantern.rs`, shader/vertex layouts, and explicit probes evolved
together. `tests/{processed_materials,expedition_base}.rs` exercise public contracts.

**Save format 3, generation 2.** Format 2 deliberately introduced tagged items;
format 3 adds crate records. No migrations/reinterpretation; incompatible/corrupt
slots remain occupied, backup + intentional confirmed replacement required.
Voxel IDs append only: Planks 23, Dressed Limestone 24, Crate 25, Lantern 26;
atlas tiles 24/25/26/27 are separate. Existing default/42 generator fingerprints
and all 24 prior atlas textures remain unchanged. No dependency after the
owner-approved M2 CPAL addition. Real save/preferences were never read/modified;
tests and smoke use disabled/exclusively owned temporary storage.

## Commands and final CPU checks

Evidence lives in the existing approved directory:

```text
/private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/
```

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --locked --example biome_probe --example cave_probe --example water_probe --example coast_probe
cargo build --release --locked
cargo test --locked --test expedition_base
cargo test --locked game::storage
cargo test --locked lantern
```

- **755 ordinary tests pass:** 673 library, 81 integration, one doc;
  zero failures, one timing diagnostic ignored. `roadmap-final-tests.log`.
- **34 explicit example tests pass:** biome 7, cave 19, coast 2, water 6.
  `roadmap-final-examples.log`. Updated diagnostic vertex fixtures carry
  separate zero local-light payload; previous terrain statistics not rewritten.
- Formatting, strict all-target locked Clippy, locked release build pass:
  `roadmap-final-{fmt,clippy,release}.log`.
- `roadmap-m3-08-capstone-tests.log`: public gather/drop→craft→equip/use→crate
  transfer→lantern placement→travel eviction→private save/Continue test passes,
  identical rebuilt light vertices; failed craft leaves snapshot unchanged.
- Unit failures cover malformed item tags/IDs/caps, truncated/corrupt/future
  files, missing/orphan/duplicate/oversized crates, invalid edits/pose, full
  destinations, cursor swaps/splits/reservations, unmineable/nonempty targets,
  out-of-reach/stale/evicted containers, invalid field bounds, solid walls,
  removal/revisions/stale worker responses, and separate preference/world saves.
- Rust LSP is unavailable in current tool catalog; no clean LSP result invented.

## GPU and native evidence

```sh
cargo run --release --locked -- --offscreen-smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-final-default.png
cargo run --release --locked -- --offscreen-smoke-test --seed 42 --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m3-08-final-42.png
cargo run --release --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m3-08-final-native.png
```

Apple M3 Max/Metal, pinned Rust 1.99.1, locked release, macOS. Latest offscreen
default **315 GPU frames**, seed 42 **315**, 289 chunks, resize verified.
`roadmap-final-default.log` / `roadmap-m3-08-final-42.log`. Final default rechecks
the last press-edge modifier, hovered-recipe activation, and cached audio-fade
hardening. The 42 capture predates only those final CPU/input changes; its full
generation/base/rendering scene remains covered by the whole-suite regressions.
All smoke/fake audio stays silent and touches no player files.

Final separately selected `cargo run --release --locked -- --audio-check` passes
**1,501 callbacks**, MacBook Pro Speakers, 48 kHz stereo; native stream stopped/
dropped (`roadmap-final-audio.log`). No microphone/window/preferences/save, and
no human listening approval inferred from delivery. M2's original bank measurements
and provenance remain in [audio evidence](audio-evidence.md).

Inspected actual readbacks (not image generation):

- M3-02 pickaxe/axe held silhouettes and half-stack inventory; M3-03 processed
  material cubes/held art; M3-04 live available/post-craft status.
- `roadmap-m3-08-crafting-all-delivered.png` and seed-42 equivalent: six
  functioning exact-cost recipes with enabled/disabled state, readable shared layout.
- `roadmap-m3-06-{crate-transfer,crate-resumed}.png`: held split returns to source
  in save, persistent contents, original tool icons. Latest capstone rechecks it.
- `roadmap-m3-08-base-crafted-lantern.png`: ordinary crafted crate/light placement.
- `roadmap-final-default-lantern-lit-receivers.png`: with headlamp off and
  **source out of view/HUD absent**, textured warm receiving wall; sealed far side
  remains dark. Latest default/42 regression rechecks those same-pose conditions.
- `roadmap-final-default-crate-resumed.png`: final restored crate/lantern/tool
  UI quantities and source-normalized contents inspected after the last checks.
- Earlier coast/arch/cove/edits/private-save exact pixels, fresh water/caves/
  headlamp/cutaway, survival/pause/audio panel, and concurrent sunlight/probe/HDR/
  volumetric scenes pass in the same offscreen run.

Lantern receiver scene mean luminance **0.0040 → 0.0417**; receiver RGB
`[0.05413, 0.04021, 0.02024]`. A sealed full-height plane and a distant room
outside reach produce identical source-present/absent RGB (tolerance 0.00001).
Removing the source restores baseline darkness. This measures receiver surfaces,
not the glowing cube or headlamp. Layout is **84-byte Vertex**, local RGB offset
72/location 7; unchanged sky/water contracts have separate fields.

**Latest native presentation is not verified.** Final native launch and recent
M3-05/06 attempts time out waiting for window/GPU presentation, report focus false
and current display 120 Hz, then terminate. `roadmap-m3-08-final-native.log` and
`roadmap-m3-05-native*.log`, `roadmap-m3-06-native.log` are retained failures, not
counted as offscreen passes. Earlier M3-02/03/04 native launches passed 313/314/328
presented frames (at 165 Hz); they do not certify the newer crate/light pipeline.
An unlocked/awake owner desktop must rerun native smoke and physical interactions.

## Intermediate failures retained

- Item refactor exposed leftover block-only fixture accesses, private visibility,
  and inactive zero hand-instance assertions; adapted actual emitted instance
  counts, retaining winding/layout checks. No tests removed.
- Processed fixture kept mining after first success; it now stops at the actual
  changed voxel and still requires exactly one break/drop. Legacy COUNT guard
  intentionally extended with a strict unchanged-prefix/ID test, not a golden reset.
- f32 accumulated hand/tool mining fell just below nominal completion; f64 work
  accumulation fixes it while suitable/unsuitable duration/progress tests remain.
- Crate Shift-placement fixture attempted to place inside the player's body;
  moved the private test listener back to permit legitimate placement. Collision
  rejection still holds and no gameplay clearance hack was added.
- Lantern test initially duplicated an edit coordinate; replaced the fixture
  override rather than weakening duplicate rejection. Old 72-byte tests/explicit
  example initializers were updated alongside the actual 84-byte GPU layout.
- Removing the private light-plane originally erased room boundary roof/floor;
  restored original boundary materials. Baseline-darkness/occlusion assertions
  remain unchanged. The first screenshot contained the emitter; final acceptance
  turns away from it so only receiving surfaces can pass the check.

## Owner acceptance still required

Back up any occupied world while closed; format 3 intentionally requires a fresh
development world for older saves, never automatic replacement. On an awake
desktop run native smoke, then choose/confirm a Survival session and:

1. Gather Wood/Limestone/Stone/Crystal; craft materials and both tools, comparing
   hand/suitable/unsuitable gathering. Exercise whole/half/single transfers/full failure.
2. Build a modest home near the coastal journey; place/open a crate, store a find,
   use Shift-placement, reject nonempty mining, close/focus/pause safely.
3. Place a lantern near a cave/approach with headlamp off; check useful surfaces,
   wall blocking, removal, and darkness beyond reach.
4. Visit the coast/caves, return, save/quit and Continue; confirm items/base/crate/
   pose/light state, comfortable sound, input/capture, and stopped audio on exit.
5. Report whether conveniences improved the trip without compulsory preparation,
   including UI/balance/repetition/navigation frustrations.

**Owner result: not reported.** M1 physical expedition, M2 listening, and M3-08
experience/native/physical interaction cannot be marked accepted by these tests.
All selected implementation is delivered; wait for tuning/acceptance, not another
wonder family, recipe tier, combat/upkeep, or weather/wildlife expansion.
