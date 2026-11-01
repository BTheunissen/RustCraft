# Nearby biome and water validation

Run the CPU-only probe from the repository root:

```sh
cargo run --locked --example biome_probe
cargo test --locked --example biome_probe
```

The probe uses the default seed and launch radius, surveys every resident-area column, and prints clear representative biome sites and the nearest water within 400 horizontal blocks. Coordinates labelled `feet` are player positions; `height` is the highest terrain block index. Sample meshes are checked for finite vertex data, valid indices/atlas IDs, outward triangles, separate water geometry, and grounded flight-to-walk landings. Timings cover generation and meshing separately, not GPU upload or an FPS benchmark.

- A biome label is not visual evidence. Count actual grass, sand, stone, and snow surfaces; alpine columns below the snow threshold are rocky. Count tree roots using wood immediately above the terrain surface, not leaf coverage.
- The initial resident region can contain all four biomes but no water. Search for an actual water block at `SEA_LEVEL - 1`; a column just below sea level can still have no water above its terrain. Use the discovered site for a separate GPU readback rather than expecting one spawn screenshot to show everything.
- Mesh material IDs are atlas indices, not `Block` enum values: grass top is 0, leaves 8, water 9. Water belongs in the separate draw pass.
- A CPU-valid mesh and landing do not verify physical input, GPU water blending, or visibility from a particular camera. Inspect bounded native smoke-test readbacks separately. Example unit tests require the explicit `--example biome_probe` target; plain `cargo test` does not run them.

For underground changes, also run `cargo run --locked --example cave_probe` with the default seed and `-- --seed 42`, plus `cargo test --locked --example cave_probe`. The bounded survey reports actual landform height/width quantiles, split air intervals, corridor junctions/loops, exposed rock materials, and formation attachment. Diagnostic lookup time is separate from generation and meshing. Those shape metrics complement standing-height walking-connectivity tests and real GPU captures; a landmark label alone proves neither geometry nor reachability.

For aquatic changes, run `cargo run --locked --example water_probe` for default/seed 42 and `cargo test --locked --example water_probe`. Observe wet footprint, actual depth/local stage, currents, marine blend, sediment support, fractional geometry, and cave-pool preservation. A bed cap alone does not prove lateral containment. Check downstream positions along the continuous normalized current—not only a rounded cardinal direction, which can select a bank rather than the actual trajectory at a bend.
