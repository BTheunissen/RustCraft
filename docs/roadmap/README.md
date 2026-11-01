# RustCraft roadmap: places worth exploring

Agreed direction: **31 October 2026**. The briefs define the ordered plan; their completion records contain current implementation evidence. Planning started with all tasks uncompleted—do not infer acceptance from a heading alone.

## Current implementation status

- **M1-00–04 complete:** existing saves validated; bounded seeded cliffs/stacks/cove/arch, coherent marine occupancy and overhang light, reversible controller-verified approaches implemented.
- **M1-05 coastal automated/native verification delivered; awaiting owner expedition playtest.** See [coastal completion evidence](coastal-evidence.md). Late concurrent HDR/shadow/probe renderer edits are separately unverified by that earlier evidence; combined-tree revalidation is recorded there. The task checkbox remains open until the hands-on gate is accepted.
- **M2 audio implementation and controls delivered; listening acceptance unverified.** See [audio evidence](audio-evidence.md). M2-03–05 subjective acceptance boxes stay open. The owner's “Sounds good, you can proceed” authorizes sequential M3 progression without inventing a physical/listening report. M1's physical expedition also remains explicitly unverified.
- **M3-01–07 complete; M3-08 capstone delivered/awaiting owner:** [base contract](expedition-base-contract.md), tagged items, safe splits, processed materials, all six recipes, permanent 3× appropriate tools, saved 18-slot crates, and actual occluded local lantern light. Save format **3**, generation **2**; no migrations/automatic replacement. See [base evidence](expedition-base-evidence.md): **755 tests + 34 examples**, strict local checks, default/42 offscreen GPU passes. Latest native launches time out; physical base-building/playtest acceptance remains unverified. All selected feature implementation is delivered; remaining roadmap work is native/owner acceptance and any resulting tuning.

## North star

Build a **mostly peaceful, exploration-first voxel adventure**. Memorable natural places are the main attraction; a persistent home and lightweight crafting make expeditions more satisfying, not mandatory.

The intended loop is:

> Prepare at home → venture somewhere interesting → discover and gather → return → improve equipment and base → venture out again.

RustCraft should retain the freedom and editability of its Minecraft-inspired world. Designed landmarks and procedural generation are complementary: **author the rules and recognizable identity of a place; let the seed choose its arrangement and variations**.

## Agreed choices

| Decision | Direction |
| --- | --- |
| Primary identity | Exploration-first adventure, not a combat or upkeep-centered survival game |
| First discovery hook | Coastal sea stacks inspired by the Twelve Apostles in Victoria, Australia |
| First coastal scope | A coherent cliff-and-stack region, accessible cove, and a coastal arch |
| Discoverability | One early coast reachable on foot from a fresh-world spawn; not only a showcase seed |
| Navigation | Landscape cues only for this roadmap; no new compass, map, journal, waypoint, or dedicated trail-marker system |
| Adventure pressure | Mostly peaceful; no health, death penalties, hunger, drowning damage, or combat in these milestones |
| Atmosphere | Sound and ambience before crafting, wildlife, weather, or a new day/night system |
| Crafting payoffs | Faster gathering tools, home storage, placeable lights, and useful building materials |
| Progression | Convenience, not access gates; retain the headlamp and omit durability |
| Compatibility | Fast iteration: no backward-compatibility or migration requirement at this stage |
| Planning horizon | Three playable milestones, with the first detailed most deeply |

Earlier interview suggestions such as a discovery journal and home compass were **not selected**. They are not implicit requirements for agents.

## Starting point: inspected, not freshly validated

The working tree contains substantial in-progress/uncommitted work. Preserve it and recheck the baseline before implementation; do not assume a clean checkout or use this roadmap as permission to reset anything.

| Existing capability | Source evidence |
| --- | --- |
| Seeded surface, geology, cave, and aquatic generation | `src/world/generation/`, `docs/world-generation.md`, `docs/water-worlds.md` |
| Chunk streaming and exposed-face voxel meshes | `src/streaming.rs`, `src/mesh.rs` |
| Walking, jumping, swimming, currents, creative flight | `src/player.rs`, `src/player/` |
| Survival mining, pickups, inventory, placement | `src/game.rs`, `src/game/`, `tests/survival_building.rs` |
| One local save slot, snapshots, atomic writes, autosave, Continue | `src/save.rs`, `src/save/`, `src/game/snapshot.rs`, `src/app/persistence.rs`, `src/app/session.rs` |
| Native/offscreen smoke checks and GPU readback | `src/playtest/`, `src/app/smoke.rs`, `README.md` |

The save system exists in the inspected source; `README.md` and `docs/architecture.md` now describe local persistence. Older world/water scope descriptions are explicitly historical. The first task validates and closes out that work rather than rebuilding persistence. No fresh Cargo test, benchmark, or graphics result was produced during the planning exercise; implementation evidence is recorded with each task below.

## Priorities and sequence

| Order | Milestone | Player outcome | Why it comes here | Main risk |
| --- | --- | --- | --- | --- |
| 1 | [Coastal wonders](01-coastal-wonders.md) | Walk to a memorable coast, explore cliffs/stacks/an arch, return, and resume later | Directly establishes the exploration identity using the existing world and movement systems | Coast/water/overhang consistency, traversal, and repetitive or pasted-in shapes |
| 2 | [Sound and ambience](02-sound-and-ambience.md) | Hear movement, gathering, surf, water, wind, and caves respond to the journey | Makes the first destination and existing actions feel alive before a larger gameplay expansion | Audio lifecycle, asset provenance, repetition, and hardware validation |
| 3 | [Expedition base](03-expedition-base.md) | Craft conveniences, gather faster, keep finds at home, light a route, and build with processed materials | Gives returning home a practical purpose after discovery and atmosphere work | Block-only inventory evolution, lossless transfers, container persistence, and actual lighting |

Persistence closeout is a readiness step **inside milestone one**, not a fourth feature milestone. General polish is part of each feature's acceptance, not an open-ended rewrite first.

### Dependency chain

```text
M1-00 baseline/save closeout
  → M1-01 coastal contract
  → M1-02 cliffs and stacks
  → M1-03 arch and occupancy/lighting integration
  → M1-04 early walking route
  → M1-05 coastal acceptance + owner playtest
  → M2-01 audio contract/backend decision
  → M2-02 playback lifecycle
  → M2-03 action feedback
  → M2-04 environmental ambience
  → M2-05 controls + listening acceptance
  → M3-01 item/crafting/base contract
  → M3-02 item model
  → M3-03 building materials
  → M3-04 crafting
  → M3-05 tools
  → M3-06 storage
  → M3-07 lights
  → M3-08 expedition-base acceptance
```

These are bounded handoffs, not calendar estimates or promises that each task takes one agent session. Split a task further if it cannot be reviewed and verified coherently; retain the same scope and gate.

## Definition of a successful milestone

- A complete, playable improvement exists, rather than only new enums, scaffolding, or a diagnostic label.
- Automated tests check the relevant behavior and failure paths; native GPU evidence covers visual changes.
- The owner plays the intended loop and confirms the experience is worth extending. Automated evidence does not certify fun, physical input, or audible output.
- Existing movement, water, editing, save safety, and architecture contracts remain intact unless a deliberate scoped change is recorded.
- Documentation describes the implemented state accurately and records anything unverified.

Milestone one uses a roughly **30-minute expedition**. Reaching the first rewarding coastal view in about **5–10 minutes including navigation and observation** is a playtest tuning target, not a unit-test timer. Do not pad the world with empty walking distance to hit that number.

## Compatibility and save safety

Do not maintain old generators or build migrations solely for backward compatibility in this development phase. Deliberately increment the generation version for incompatible terrain changes and the file-format version for incompatible progress changes. Update golden fingerprints only for intentional changes, never merely to make a failing test pass.

This policy does **not** authorize deleting progress. Incompatible/corrupt saves remain occupied, load errors explain when a fresh development world is needed, and replacement still requires the player's explicit confirmation. Tests and smoke launches must not read or overwrite the real local save. Do not silently apply old edits to a newly interpreted world.

Stable material IDs remain append-only; voxel IDs are not atlas tile IDs. Fast iteration is not a reason to casually renumber materials or weaken determinism tests.

## Not committed in these three milestones

- Additional wonder families, ruins, story quests, loot progression, or a general landmark/plugin framework.
- New navigation UI, automatic breadcrumbs, trail-marker items, home teleport destinations, or exploration checklists. Ordinary player-built block trails remain possible; the existing return-to-spawn action stays available.
- Wildlife, NPCs, hostile mobs, health, food, farming, tool durability, crafting tiers, or equipment-locked regions.
- Weather, seasons, a new day/night gameplay system, boats, fishing, runtime water spreading, erosion, or structural collapse.
- Multiple save slots, cloud sync, multiplayer, modding, an engine rewrite, additional crates, CI, or corporate integrations.

Later ideas worth revisiting after playtests: a second contrasting natural-wonder family, passive wildlife, changing daylight, richer home blocks, and broader coastal exploration. These are options, not an implementation queue. Revisit navigation assistance only if the owner wants it after trying landscape-led exploration.

## How to use this plan

1. Start with **M1-00**, not a new feature branch full of scaffolding.
2. Read the relevant milestone and [agent workflow](agent-workflow.md).
3. Implement one task at a time; its dependency gates must be satisfied.
4. Record evidence and mark the task complete in its milestone document.
5. Stop at milestone playtest gates before expanding into the next milestone.

The selected features and ordering are agreed scope. Numeric tuning defaults, implementation layouts, and backend choices in the briefs are proposals to validate, not facts already observed. Milestones two and three have explicit contract checkpoints before agents start their broader changes.
