---
name: rustcraft-voxel-world
description: Implement and review deterministic RustCraft terrain, chunk generation, and exposed-face voxel meshes. Use for world-generation and meshing subagent work.
---

# RustCraft Voxel World

Load `rustcraft-rust` first. Keep this subsystem CPU-only and independent of windowing and GPU resources.

## Contracts

- World axes are +Y up, +X right, and -Z forward at yaw zero. Blocks occupy integer-aligned unit cubes; their top is `y + 1`.
- Chunk columns span 16 blocks in X/Z. Use Euclidean division for negative coordinates.
- Generation must be a pure function of seed and world coordinates, not chunk load order. Decorations must agree at chunk boundaries.
- Use a small bounded vertical range, deterministic noise, and recognizable biomes. Prefer a few well-tested materials over a large registry.
- Mesh only exposed faces. Sample neighboring world blocks across chunk seams; do not treat an unloaded neighbor as air.
- Keep opaque and water geometry separate. Use outward counterclockwise winding, consistent UVs, and normals.
- For negative-Y geology/caves, also load `rustcraft-underground`; the upper Y coordinate is not the dense column length.

## Terrain generation

- Cache one terrain sample per X/Z column before filling its vertical voxels. Reuse those samples for tree roots inside the chunk; only roots in the decoration halo need extra samples.
- Select tree roots in seeded world-space cells, then enumerate every root cell within the canopy radius of the requested block or chunk. Give wood priority over overlapping leaves, and never replace terrain or water; use the same rules in direct sampling and dense generation.
- Bound terrain height below the vertical ceiling by enough room for the tallest tree plus player clearance. Spawn positions are feet at `height + 1`, not eye positions; check dry support and full standing-player headroom against decorated world samples.
- For compact biome variety, blend height contributions with smooth climate weights rather than switching height formulas at a biome label boundary. Verify actual biome and surface coverage around spawn, not just distant noise samples.

## Validation

- Test seed repeatability/variation, biome coverage, negative coordinates, spawn clearance, chunk/sample agreement, and decorations at seams.
- Test isolated cubes, adjacent cubes, empty geometry, index bounds, face winding, and boundary occlusion.
- Test neighbor-source selection as well as face counts: interior reads stay dense, empty blocks never query terrain, and opaque/water contact emits only the opaque interface.
- Cache vertical daylight and halo block columns once per mesh. Test that local-versus-halo light values agree; repeatedly sampling procedural terrain for every seam face is avoidable work.
- Batch nearby halo columns through one bounded generation region. Caching each ray still repeats expensive rooted-formation setup if every ray constructs its own region.
- Keep voxel material IDs distinct from atlas tile IDs, and share the mapping with alternative rendering paths such as creative cutaways. Retain CPU voxels under the same bounded eviction policy as GPU meshes.
- Measure generation/meshing cost before adding optimization abstractions. Avoid recomputing noise for every buried voxel.
- Test that some generated columns really fall below sea level; a correct water-fill branch is insufficient if the chosen height distribution never reaches it.
- Keep water occupancy separate from basin terrain-blending halos. If an outer halo blends back below the water stage, filling it can create unsupported walls at the feature cutoff; test wet/dry neighbors at the actual shoreline, not only bed caps.
- Find deterministic trees whose canopies cross both positive and negative chunk seams, then compare entire neighboring chunks to direct world sampling. Flat boundary columns alone do not exercise decoration halos.
- Promote coordinate products and decoration halo arithmetic to `i64` before checking conversion to the `i32` block world; valid extreme block coordinates must not overflow in debug builds.
- After a real bug or useful discovery, add a short reusable rule here; do not record transient task logs.
- For nearby biome/material coverage and representative camera sites, use the CPU probe workflow in `references/validation.md`.
