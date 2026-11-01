---
name: rustcraft-underground
description: Build and validate RustCraft negative-Y cave generation, geology, cave biomes, underground lighting, and cave exploration. Use for underground-generation and cave-validation subagent work.
---

# RustCraft Underground

Load `rustcraft-rust` and `rustcraft-voxel-world`. Keep generation and lighting data renderer-independent.

## Coordinate and generation contracts

- The vertical interval is `MIN_WORLD_Y..WORLD_HEIGHT`; `WORLD_DEPTH` is its length, not the top coordinate. Chunk X/Z reads are local, but chunk Y reads are world coordinates, including negatives.
- Preserve the bottom bedrock layer and keep surface terrain, trees, and safe surface spawning usable. Cave openings must genuinely connect to traversable underground space, not just decorative pits.
- Generate tunnels and chambers from seeded world-space features or coherent fields. Chunk order cannot affect caves, ore veins, liquids, or decorations.
- Direct sampling and dense chunk generation must have exactly the same carving/decorating precedence. Cache reusable feature/column computations before filling voxel volumes.
- Use a shared vertical-column sampler for skylight and mesh boundary rays; rebuilding the cave network for every Y sample can dominate otherwise cheap chunk generation.
- Batch a mesh's nearby halo rays through one bounded feature region. Reject or shorten formations against only intersecting route spans; reprojecting the whole cave network for every formation-footprint voxel is unnecessary.
- Use a deterministic network skeleton for connectivity, then vary rotated, bounded-warp room envelopes and passage cross sections. Retained pillars, shelves, loops, and separated bedding levels distinguish cave families more effectively than extra noise octaves on identical ellipsoids.
- Reserve actual supported walking corridors after subtracting pillars and ledges. At intersecting ramps, merge nearby floor levels before restoring support; overlapping air masks alone can leave sudden drops or one-block headroom.
- Anchor tapered formations in world-space cells to actual floor/ceiling rock, not independent air-voxel hashes. Population halos must include both the growth's maximum reach and the topology around its root; direct Y-clipped queries can otherwise lose ceiling roots and disagree with chunks.
- Slanted conical tips need a six-neighbor-connected voxel spine. Preserve routes by shortening a rooted growth or rejecting a whole column, not clipping individual voxels into floating fragments.
- Cave water remains a calm, bounded freshwater pool with swimming support; lava remains non-damaging scenery. Seal surface-water beds at their local basin level rather than assuming sea level, and do not fill every underground void.
- Cave biomes need visible materials and actual nearby explorable spaces, not merely labels. Ores are decorative blocks, not an inventory or mining feature.

## Lighting and verification

- Darkness depends on occluded daylight, not just absolute Y. Roofed positive-Y caves are dark; exposed negative-Y pits can receive daylight.
- A headlamp follows the camera direction, attenuates with distance, and can be disabled without changing the world or controls.
- Keep emissive surfaces and transparent liquid interfaces distinct from opaque cave walls. Neighbor sampling, lighting, and geometry must agree at chunk seams.
- Test negative-Y bounds/indexing, bedrock, seed variation, cave connectivity/entrances, ore/biome/liquid coverage, direct/chunk agreement, and player clearance.
- Connectivity checks need standing-height headroom, solid support, and bounded step transitions, not just flood-filled air. Include the entire sloped entrance run even when its mouth is close to spawn.
- Diagnostic cave poses need jump clearance, a long open view, and representative wall materials; a feet-position biome label at a zone boundary does not prove the screenshot depicts that biome.
- Check landform morphology separately from camera clearance: tall narrow versus low wide profiles, full-height retained pillars, multiple air intervals separated by rock, and connector tortuosity. A named pose does not prove its geometry family.
- Inspect real GPU cave frames with the headlamp on and off. Surface-only screenshots do not validate caves. Measure generation and mesh costs over an underground region before claiming interactive streaming performance.
- Add only concise, reusable lessons discovered through this work; do not turn this skill into a task log.
