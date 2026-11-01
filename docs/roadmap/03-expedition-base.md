# Milestone 3: a home that supports expeditions

**Outcome:** gathered resources can become a few practical improvements: faster permanent tools, useful building blocks, saved home storage, and lights that actually illuminate nearby terrain. Returning home becomes useful without restricting exploration.

The original sequence begins after M2-05 listening acceptance. The owner's “Sounds good, you can proceed” now authorizes M3 progression while physical/listening acceptance remains explicitly unverified. M3-01 is frozen in [the expedition-base contract](expedition-base-contract.md); implementation follows sequentially. Follow the [roadmap](README.md) and [agent workflow](agent-workflow.md).

## Gameplay boundaries

- Apply crafting/base interactions to survival building; keep Creative Explorer's existing exploration-only rules unless the owner explicitly requests otherwise.
- Keep hand mining possible. Tools improve suitable gathering speed; they do not unlock regions, ores, or mandatory routes.
- Keep the headlamp, safe return-to-spawn, and current non-damaging world. No hunger, durability, combat, death penalties, equipment access gates, furnace, or metal-processing tree.
- Use a small recipe list in the inventory, not a spatial crafting grid, workbench hierarchy, or discovery/unlock system.
- Placeable lights need an effect on nearby world surfaces. A glowing cube texture alone does not satisfy the feature.
- Each checkpoint exposes only functioning materials/recipes/actions; do not show broken placeholder recipes while their feature is unfinished.

## Initial recipe/behavior proposal

These are intentionally small tuning defaults to freeze or adjust in M3-01, not additional user-approved balance decisions.

| Inputs | Output | Purpose |
| --- | --- | --- |
| 1 Wood | 4 Planks | A distinct, useful home-building material |
| 1 Limestone | 1 Dressed Limestone | A processed stone material for homes and ordinary block-built trails |
| 4 Planks + 3 Stone | 1 Stone Pickaxe | Faster rock/ore gathering, no access gates |
| 4 Planks + 2 Stone | 1 Stone Axe | Faster wood gathering |
| 8 Planks | 1 Storage Crate | Keep finds at home |
| 2 Planks + 1 Crystal | 1 Lantern | Permanent local light, no fuel meter |

Initially propose **3× suitable-material mining speed** for the pickaxe/axe, no benefit for inappropriate materials, **18 crate slots**, and a lantern reach around **8 blocks** with occlusion and attenuation. Keep tools non-stackable and ordinary blocks bounded to the existing stack size. No crafting dependency should require a tool that can only be obtained using itself.

## Architectural starting point

- `src/game/inventory.rs`: `ItemStack` currently contains a `Block`; only collectible solid blocks are valid. Equipment needs a deliberately small item representation, not just a new recipe button.
- `src/game/{pickup,pickup_physics,interaction,snapshot}.rs`: placement, drops, mining duration, inventory, and snapshots assume blocks. Update their contracts together.
- `src/save/format.rs`: explicit versioned encoding and payload limits must evolve with item/container state. No migration is required; no silent reset is allowed.
- `src/world/material.rs`, `src/atlas/`, `src/hud/`, `src/renderer/survival/`: voxel IDs, atlas IDs, icons, and held-item geometry are different responsibilities.
- `src/lighting.rs`, `src/mesh/`, `src/streaming/`, `src/shaders/`: current daylight/headlamp/emissive presentation is not already a general placeable-light propagation system.

Keep domain item/recipe/container logic in `game/`, voxel physical/emission properties in `world/`, CPU light math in `lighting/` or the appropriate existing CPU layer, GPU work in `renderer/`, and screen transitions in `app/`. Do not introduce a global entity framework, plugin registry, or multi-crate workspace.

## Task queue

### M3-01 — freeze item, recipe, storage, and light contracts

- [x] Complete
- **Dependencies:** M2-05 accepted. **Scope:** a concise contract/design update before code expansion.
- Define the minimal distinction between block items and tool items, stable item encoding, stack limits, selected/equipped behavior, pickup representation, and held-item presentation.
- Freeze the small recipe set, material groups/tool speed, crate slot/count limits, nonempty-crate removal rule, and lighting radius/occlusion/invalidation approach.
- Define lossless crafting and transfer transactions, including capacity after consuming ingredients, cursor-held items, failure paths, and close/pause/save behavior.
- Define bounded sparse container ownership keyed by world coordinate, validation against placed crate blocks, and snapshot/format size limits. Containers must survive chunk eviction without persisting generated chunks.
- Record incompatible file-format/generation changes, if any, under the fast-iteration policy. Do not add migration machinery or reinterpret old block IDs as tools.
- **Gate:** changes across game/save/UI/renderer/lighting have coherent contracts. If the light design needs an engine rewrite, discuss a smaller bounded approach rather than dropping real illumination.

### M3-02 — evolve blocks-only inventory into a small item model

- [x] Complete
- **Dependencies:** M3-01. **Scope:** item/inventory domain, pickups, snapshots/format, item-aware HUD/held presentation and tests as needed to preserve existing play.
- Represent placeable block items and the selected small tool set distinctly. Non-block tools must not pass block-placement validation.
- Preserve existing stack insertion, whole-stack cursor reservations, close restoration, pickups, selection, mining block drops, and item conservation. All existing block items remain usable.
- Add minimal right-click splitting/single-item transfers for practical inventory/storage use, with a safe close path; avoid a broad shortcut overhaul.
- Update snapshot validation and explicit serialization together, with appropriate item tags, quantities, selected slot, bounds, and version. Invalid/unknown IDs, oversized counts, corrupt files, and wrong-mode settings remain rejected safely.
- Adapt icon/held readouts without turning item IDs into voxel or atlas IDs. Keep native/GPU resources out of CPU logic.
- **Tests:** boundary quantities, tools with count greater than one rejected, full inventories, split/merge/swap/close conservation, block-only placement, pickup leftovers, and isolated save round trips/failures.
- **Gate:** all existing mining/building and saved progress work under the new model; there is no publicly exposed half-working crafting system yet.

### M3-03 — add two useful building materials

- [x] Complete
- **Dependencies:** M3-02. **Scope:** planks/dressed-limestone material properties, original textures/icons, mappings, rendering, tests.
- Add the two simple full-cube materials with append-only voxel IDs and deliberate atlas mappings. Do not add partial blocks, doors, stairs, rotations, or a construction-tool framework.
- Verify mining, placement, collisions, item names/icons, held blocks, pickups, and saves. Keep terrain generation unchanged; these are crafted materials, not newly painted biomes.
- Check ordinary and phasing/cutaway texture mappings, face UVs, seams, and edited chunk reload.
- **Gate:** both materials render and behave correctly and are ready to become the first craftable outputs.

### M3-04 — implement a small, transactional crafting interface

- [x] Complete
- **Dependencies:** M3-03. **Scope:** declarative recipe logic, inventory craft panel, app/input transitions, tests.
- Start with the two implemented material recipes; enable the remaining agreed recipes as their features land in M3-05–07.
- Show inputs, output, and whether one craft can succeed. Craft from available inventory slots, excluding cursor-held items; keep the existing paused inventory semantics.
- Validate and commit consumption/output as one transaction. Missing inputs or insufficient output capacity must leave all quantities unchanged. Account for slots freed by consumed ingredients; never silently drop overflow.
- Prevent UI clicks from also mining/placing. Preserve cursor safety, focus loss, Escape, and saving while the inventory is open.
- **Tests:** exact costs, quantities, output stacking, capacity after input removal, full inventory rejection, held cursor, invalid recipe, repeated clicks, overflow bounds, and UI hit-test consistency.
- **Gate:** gathering Wood/Limestone and crafting usable building blocks is playable and lossless; no crafting grid or recipe unlock ladder has appeared.

### M3-05 — make permanent tools a convenience upgrade

- [x] Complete
- **Dependencies:** M3-04. **Scope:** pickaxe/axe recipes, mining rules/progress, held-item feedback, audio mappings, tests.
- Selecting a tool in the hotbar makes it the active mining tool; it is not placeable or consumed by use. Bare-hand mining remains valid.
- Apply the frozen speed benefit to suitable materials only, retaining bedrock/water/lava restrictions and one normal drop per mined block.
- Switching tools/targets, releasing mining, pausing, or opening inventory must reset/reconcile damage consistently; normalized crack progress must use the active mining duration.
- Provide distinct original icons/held silhouettes and reuse milestone-two feedback without duplicate completion sounds.
- **Tests:** hand versus suitable/unsuitable tool duration, all material-group boundaries, unmineable blocks, mid-mine tool switches, pause/release, preserved tool quantity, and save/Continue.
- **Gate:** both upgrades reduce gathering friction without durability, ore gates, or a tiered equipment tree.

### M3-06 — give the home a saved storage crate

- [x] Complete
- **Dependencies:** M3-05. **Scope:** crate block/recipe, sparse container domain, two-inventory UI, interaction, persistence, tests.
- Place a normal solid crate with the usual reach/residency/collision/item-consumption rules. Its contents belong to CPU gameplay state, not chunk meshes or the generator.
- Open a targeted resident crate within reach through an explicit interaction rule frozen in M3-01. Provide an unambiguous way to request placement against a crate without opening it accidentally.
- Transfer items conservatively between player and crate inventories. Inventory/containers pause simulation and preserve capture/input safety; closing, Escape, focus loss, and snapshots cannot lose cursor items.
- Initial removal rule: **a crate must be empty before mining**. Reject nonempty removal with clear feedback rather than destroying or scattering contents. Preserve empty-crate mining/drop behavior.
- Save unique, bounded container records; validate positions and association with placed crate blocks, and preserve contents across eviction, save/quit, and Continue. No duplicate/orphan contents or cached generated chunks in the file.
- **Tests:** both transfer directions, full destinations, split/merge/close conservation, out-of-reach/unloaded interaction, nonempty removal rejection, empty removal, negative coordinates/seams, invalid records, and isolated reload round trips.
- **Gate:** a player can leave finds at a base, travel away, return, and restart without loss or duplication.

### M3-07 — add lanterns that light the world

- [x] Complete
- **Dependencies:** M3-06. **Scope:** lantern block/recipe, CPU bounded light field/invalidation, worker/mesh/shader payloads, feedback, tests and GPU evidence.
- Start with one simple permanent, warm light source; no fuel, toggles, colored-light catalogue, or electrical system.
- Propagate/attenuate light through the frozen bounded voxel neighborhood with solid occlusion. Neighboring surfaces must visibly brighten; the source's emissive texture alone cannot pass acceptance.
- Use consistent edit/revision snapshots for neighboring chunks and light halos. Placing/removing a light or adding/removing a wall updates affected fields/meshes; stale worker results are rejected and eviction/reload restores the same result.
- Keep caches bounded and GPU uploads presentation-only. Do not scan all world edits every frame, generate terrain on the render thread, or loop over an unbounded emitter list in each shader fragment.
- Preserve daylight, headlamp, cave darkness outside light reach, underwater behavior, and creative cutaways. If current dependency guardrails need a justified new CPU edge, update the explicit policy/test rather than adding an exemption.
- **Tests:** falloff, solid walls, openings, overlapping sources, seams/negative Y, edits/dirty neighbors, source removal, stale results, and eviction/reload consistency.
- **GPU evidence:** identical dark-room/cave pose with and without a lantern, headlamp off, HUD excluded; lit neighboring textured surfaces, occluded far side, and unlit distant cave. Inspect real frames, not just numeric brightness of the source cube.
- **Gate:** lights are useful for a camp/cave route and do not globally brighten the underground or introduce unbounded lighting work.

### M3-08 — verify the complete expedition-base loop

- [ ] Complete
- **Dependencies:** M3-07. **Scope:** CPU/native survival capstone, regression checks, current docs, owner playtest.
- Exercise gathering → material crafting → tool crafting/use → base building → crate transfer → lantern placement → travel/eviction → save/Continue using isolated storage in automated checks.
- Verify item conservation and state consistency through failed craft/placement/transfer/save paths; no duplicate success sounds or click-through world edits.
- Run normal Rust checks, affected explicit example tests, and native smoke/readback for new inventory/crafting/storage/held-tool/light scenes. Regress coastal generation, ordinary water/caves, creative movement/phasing, and save safety.
- **Owner playtest:** build a modest home near the coastal journey, craft the conveniences, store a find, light an approach, visit the coast/caves, return, quit, and Continue. Ask whether upgrades improved the trip without making preparation feel compulsory.
- **Gate:** the loop is playable, preserved across restart, and owner-accepted. Record any balance/UI frustration before choosing another wonder, recipe tier, or living-world system.

## Scope pressure

This is the broadest milestone because all four conveniences were selected. Deliver and review the tasks sequentially; do not implement them in one large patch. The lighting/storage tasks are real systems work, not small recipe additions. Replan task boundaries at M3-01 if needed without silently removing selected features or adding progression gates.

## Implementation records

### M3-01 — completed, 1 November 2026

- **Owner steering:** “Sounds good, you can proceed” authorizes progression; no physical/listening acceptance report is invented.
- **Outcome:** [`expedition-base-contract.md`](expedition-base-contract.md) freezes tagged items/save versions, split/cursor reservations, lossless crafting, six staged recipes, appropriate 3× tool groups, 18-slot/4096-crate limits and Shift-placement/open semantics, and an actual bounded eight-block CPU/worker lantern field separate from GPU skylight/probes. No dependencies or terrain-generation changes.
- **Inspection:** current inventory/pickup/interaction/snapshot/save format, app/input/HUD/shared layout, atlas/material/held/drop GPU code, existing edit halo/stale streaming policy, and concurrent renderer probe emission were inspected. M2 closeout passes 702 ordinary + 34 example tests, fmt/strict Clippy/release, fresh native GPU and explicit hardware check; human acceptance remains unverified.
- **Next task:** M3-02 item model/split transfers/tagged save, not all base features at once.

### M3-02 — completed, 1 November 2026

- **Outcome:** explicit block/tool `Item`, per-item caps, item pickups/snapshots, original tool HUD/held/drop silhouettes, right-click half/single transfers with reserved-source conservation, block-only placement. No tool recipes or speed benefit exposed before M3-05.
- **Compatibility:** save format **1 → 2**, generation stays **2**; tagged items cannot reinterpret old block IDs. Unknown tools/tags/invalid counts/truncation fail closed; old slots remain occupied and require confirmed replacement. Real user files untouched.
- **Checks:** fmt/strict all-target Clippy/locked tests pass (**718 tests**, one ignored timing diagnostic); mixed 8000-operation transfer conservation, full-capacity/split swaps/close, tool cap/placement/drop tests, isolated tagged saves/old-format rejection, all existing gameplay regressions. Logs `roadmap-m3-02-complete-{fmt,clippy,tests}.log` in the approved temporary directory.
- **Native:** locked release smoke **313 presented frames**, resized completion on Apple M3 Max/Metal; coast/water/caves/lighting/save/audio-fake regressions plus same-pose distinct held-tool pixels and private split/tool save round trip. Inspected `roadmap-m3-02-{stone-pickaxe,stone-axe,item-split-inventory}.png`; log `roadmap-m3-02-native.log`. Physical inventory input remains unverified.
- **Next task:** M3-03, two crafted building materials; terrain goldens remain unchanged.

### M3-03 — completed, 1 November 2026

- **Outcome:** append-only Planks 23 / Dressed Limestone 24, atlas tiles 24/25 with original processed textures, shared icon/held/drop/phase mappings, collision/mining/sound/save/edit-reload behavior. Terrain generation and goldens unchanged. Diagnostic material lists updated without repainting generated biomes.
- **Checks:** full passing regression at M3-04 below includes processed mine/drop/place/snapshot/remesh integration and old atlas-art checksums. Explicit biome/cave/water/coast examples **34 pass** (`roadmap-m3-03-examples.log`); fmt/Clippy pass. An initial material test kept mining after the first break; corrected to stop at real success, retaining exact single-break assertion. An old COUNT assertion deliberately extended to append-only IDs with legacy prefix intact.
- **Native:** release smoke **314 presented frames**, resize verified (`roadmap-m3-03-native.log`). Inspected `roadmap-m3-03-placed-{planks,dressed-limestone}.png`; both distinct materials and held cubes render. Current M3-04 smoke also regresses them.

### M3-04 — completed, 1 November 2026

- **Outcome:** inventory Crafting button / C opens a paused recipe list; one Wood→four Planks and one Limestone→one Dressed Limestone. Exact declarative costs, capacity after consumption, cursor/reservation exclusion, atomic failure no-ops, and live success/ingredient/capacity status. No placeholder tool/crate/light recipes.
- **Checks:** fmt, strict all-target Clippy, locked whole suite **727 tests pass**, one ignored timing diagnostic (`roadmap-m3-04-{fmt,complete-clippy,complete-tests}.log`). Tests cover full output rejection, freed input slot, stacking, invalid/repeated recipe, held split failure/focus/save/close, shared hit regions and no click-through. Existing item/mining/save/coast/water/cave regressions pass.
- **Native:** locked release smoke **328 presented frames**, resized completion (`roadmap-m3-04-native.log`); actual crafted outputs and private save round trip. Inspected available/after recipe GPU readbacks, including disabled missing-limestone status. Physical recipe clicks/navigation remain unverified.
- **Next task:** M3-05 permanent 3× appropriate tools and their recipes.

### M3-05 — completed, 1 November 2026

- **Outcome:** live pickaxe/axe recipes; permanent nonstackable equipped tools grant 3× suitable-material gathering only, retain hand mining/unmineable boundaries, reset mid-mine tool changes, preserve quantity and one drop/success. Mining elapsed time now accumulates in f64 so partition rounding cannot stall a nominally complete hand/tool duration.
- **Checks:** suitable/unsuitable duration groups, normalized progress/switch/pause, no tool consumption/placement, exact multi-input recipes/full-capacity no-op, tagged save. Full final M3-06 suite below includes these regressions; formatting/strict Clippy pass. Native attempts timed out waiting for presentation (`roadmap-m3-05-native*.log`), retained as failures. Explicit offscreen release passes **328 GPU frames**, resized completion, inspected four live recipes and tool silhouettes (`roadmap-m3-05-offscreen.log`). No fake native or physical-input acceptance.

### M3-06 — completed, 1 November 2026

- **Outcome:** 18-slot solid crate/8-plank recipe; sparse 4096-record CPU ownership; right-click open versus Shift+right-click placement; safe paused two-inventory UI; whole/half/single transfers and explicit player/crate cursor source. Full destinations/split swaps are no-ops; focus/close/snapshot normalize without loss. Nonempty removal reports “EMPTY THE CRATE BEFORE MINING”; empty removal drops one crate and deletes its record.
- **Compatibility:** save **format 3**, generation still **2**; unique bounded records validate against placed crate overrides, including negative seams and unknown IDs/counts. Contents survive eviction/private save/Continue; invalid/orphan/duplicate/missing records fail closed. Real save untouched.
- **Checks:** final verified suite **743 tests pass**, one ignored timing diagnostic; fmt/strict all-target Clippy pass (`roadmap-m3-06-verified-{tests,fmt,clippy}.log`). Includes 10,000 mixed cross-inventory transfer/insertion conservation, full capacity/reservations, reach/stale/residency, invalid records, modifier/open and app focus/input tests. Additional save hardening remains in the ongoing whole-suite verification.
- **GPU:** explicit offscreen **315 GPU frames**, resize; ordinary placed crate/transfer, half-held cursor-normalized private-save round trip and evicted resumed contents pass (`roadmap-m3-06-verified-offscreen.log`). Inspected crate transfer/resumed readbacks. Native attempt timed out waiting for presentation (`roadmap-m3-06-native.log`), so current native/physical crate input remains unverified.
- **Next task:** M3-07 real bounded lantern illumination, then complete capstone and owner gate.

### M3-07 — completed, 1 November 2026

- **Outcome:** original full-cube Lantern/recipe, warm eight-step CPU flood through the edited 32×32×depth halo; solids occlude and water attenuates. Worker-only sparse 3×3 source discovery; no work field for source-free chunks. Separate RGB vertex payload (84-byte layout) does not brighten camera/sky/fog or long-range probes; placement/removal/walls use existing stale-result/halo revision policy.
- **Tests:** reach/falloff/overlap/walls/openings/water/bad bounds, negative seam field equality, exact mesh reload/removal, source/wall neighbor invalidation, actual worker source-removal replacement. Current whole suite/examples/fmt/Clippy evidence is in [`expedition-base-evidence.md`](expedition-base-evidence.md).
- **GPU:** same-pose headlamp-off/source-out-of-view receiver luminance **0.0040 → 0.0417**, warm RGB `[0.05413,0.04021,0.02024]`; sealed far side and distant room unchanged; removal restores darkness. Explicit offscreen default/42 passes, inspected lit receiver and black far-side frames. Earlier fixture mistakenly removed boundary roof/floor while reopening the wall; corrected geometry restoration rather than relaxing the zero-retained-light check. No native presentation/physical lantern acceptance is claimed.

### M3-08 — automated/GPU capstone delivered; awaiting owner playtest

- **Outcome:** public CPU test gathers real drops → crafts materials/tools/crate/light → verifies suitable tool speed → transfers/reserves storage → places lantern → evicts → isolated save/Continue → identical rebuilt light payload. Native/offscreen diagnostic scenes exercise all six live recipes, original materials/held tools, ordinary crafted crate/light placement, cursor-normalized storage save/eviction/reload, and true lantern receiver/occlusion/removal evidence.
- **Final checks:** **755 ordinary tests pass**, one timing diagnostic ignored; **34 explicit example tests pass**, fmt/strict all-target Clippy/locked release pass. Logs `roadmap-final-{tests,examples,fmt,clippy,release}.log`. Preserve default/42 generation goldens and legacy material/atlas IDs. Press-edge Shift modifier and shared hovered-recipe Enter activation have regression tests.
- **GPU limitation:** latest default/42 offscreen **315/315 GPU frames**, resize and all scenes pass; final real-device audio recheck **1,501 callbacks**, stream stopped/dropped. Final native launch times out waiting for presentation (`roadmap-m3-08-final-native.log`); earlier M3-02/03/04 native passes do not certify later tool/crate/light presentation. Exact commands and inspected captures: [`expedition-base-evidence.md`](expedition-base-evidence.md).
- **Owner gate:** physical capture/menus/crafting/storage/modifier/light interaction and the coastal base expedition/return/save/Continue experience are not owner-reported. M3-08 stays unchecked. No additional wonders, recipes, weather/wildlife, progression gates, or framework work is eligible without a new owner decision.
