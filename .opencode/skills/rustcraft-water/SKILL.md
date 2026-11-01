---
name: rustcraft-water
description: Build and validate RustCraft deterministic aquatic basins, drainage, current-aware swimming, fractional water meshes, and freshwater/marine optics.
---

# RustCraft Water

Load `rustcraft-rust` and `rustcraft-voxel-world`; also use player-controls or native-playtest for those layers. Keep hydrology and water math renderer-independent.

- Water-biome metadata is separate from the terrestrial biome and material ID. Preserve append-only block/atlas IDs; material ID 8 remains water.
- Generate drainage, mean surface, bed, and current together. Verify non-increasing downstream stages and real source/outlet connectivity; a meandering blue footprint does not prove flow.
- Separate wet basin footprints from terrain-blending halos. Check actual wet/dry neighbors at the water stage, and seal local-level beds without globally filling caves.
- Keep overhanging roofs separate from ground/water beds. Marine water beneath an arch must retain its original metadata; a crown height is not a fluid bottom, and full/dense/direct/halo sampling must agree on the actual opening.
- Store resident water metadata under the same eviction policy as CPU chunks/GPU meshes. Missing residency is not water; known dry voxels should not trigger procedural fluid sampling.
- Shared water corners require identical mean heights, depth/salinity, and wave weights on both sides of a chunk seam. Pin shore/ceiling vertices, and expose only the strip above a lower fractional neighbor.
- Currents animate rendered flow and carry swimmers, but creative flight ignores them. Integrate actual body immersion, drag, and current with bounded collision substeps; preserve dry jump and flight-exit semantics.
- Keep visual waves bounded and distinguish their small displacement from the mean hydrological waterline. Do not describe steady-state generated currents as a mass-conserving runtime fluid solver.
- Compare water-animation frames with HUD removed and sky time fixed. Compare freshwater/marine optics at the same submerged pose, and inspect beds plus the surface from below.
- Prove reverse water surfaces with a same-pose draw-pass comparison, not just a fog-colored screenshot. Test walking entry and swimming exit on generated shores; supported submerged steps must rearm shore exits without repeating dry-land jumps.
- Run `cargo run --locked --example water_probe` for default/seed 42 and `cargo test --locked --example water_probe` explicitly. CPU footprint/depth/flow/bed metrics supplement, not replace, native GPU readback and hands-on input checks.
