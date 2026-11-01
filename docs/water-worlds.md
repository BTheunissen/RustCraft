# Water worlds: research and implementation scope

**Historical feature closeout:** this document records the earlier water/explorer upgrade. Statements excluding editing/survival refer to that pass; the current game has saved survival mining and placement. Water still does not spread after edits. The acceptance counts and benchmarks below are historical, not validation of the current roadmap working tree.

This upgrade treats water as an exploration environment rather than a blue block that the player falls through. It separates three concerns: seeded drainage and basins, water motion used by both rendering and swimming, and surface/underwater presentation. The block world, cave depth, creative flight, and exploration-only gameplay remain intact.

## Research findings

### Minecraft: local fluids are different from whole-world drainage

Minecraft water has source and flowing states, fractional surface heights, downward spreading, and bounded horizontal spreading. Its current determines visual flow and pushes swimmers; flying creative players ignore it. Ocean terrain includes coasts, shelves, different depths, and varied seabed materials. Importantly, generated Minecraft rivers normally contain still water at sea level: their river-shaped terrain is not a global hydrological simulation.

**Adaptation:** preserve non-solid water, add swimming and a real current vector, keep fractional mean surfaces where needed, and classify freshwater/marine environments independently of the land biome. A flowing river in RustCraft should show directional movement and carry a swimmer, not merely have a river label.

Sources reviewed: [Minecraft Wiki: Water](https://minecraft.wiki/w/Water), [Ocean](https://minecraft.wiki/w/Ocean), [River](https://minecraft.wiki/w/River). The Wiki is community documentation, not an official source-code specification. No mobs, structures, buckets, survival, or infinite-source gameplay are introduced.

### Procedural drainage: terrain, paths, and pools must agree

Red Blob Games explores constructing drainage basins and river networks before assigning compatible elevation, including explicit source/bend/junction relationships. Nick McDonald's hydrology experiment couples terrain, stream flow, and pooled water; it highlights the difficulty and cost of flooding large basins and shows why still pools and moving streams deserve different treatment.

**Adaptation:** use a deterministic, bounded steady-state representation of basins and draining paths. Water surface elevation must not rise along its downstream path; waterfalls need a connected upper source and lower receiving water, and lake/ocean beds need actual support. Generate the terrain and its water together rather than painting waterways onto unrelated noise.

Sources: [Red Blob Games: procedural river drainage basins](https://www.redblobgames.com/x/1723-procedural-river-growing/), [Nick McDonald: Procedural Hydrology](https://nickmcd.me/2020/04/15/procedural-hydrology/). These are implementation explorations with stated limitations, not claims that a simple algorithm reproduces every natural drainage pattern. RustCraft does not implement their full erosion or particle-based simulations.

### Water rendering: geometry, directional flow, and optics have separate roles

Mark Finch's *GPU Gems* chapter, developed for Cyan Worlds' *Uru*, combines geometric surface waves with finer normal detail. Depth controls transparency and wave attenuation near shores; view angle changes reflection. Catlike Coding's flow tutorials distinguish isotropic texture distortion from properly oriented ripple fronts and discuss spatial discontinuities in changing flow fields.

Hello Games' official *Worlds Part I* notes describe a mesh-based water overhaul with waves, foam, sun/ambient reflections, and depth-dependent conditions. This establishes useful visual goals, not a license to infer its unpublished algorithms.

**Adaptation:** bounded world-space surface waves, smaller freshwater ripples, directional river movement, downward waterfall streaks, restrained shoreline/rapid foam, and shallow-to-deep transparency. Shared world-space corners must remain identical across chunks. Analytic sky/sun highlights are not advertised as scene reflections or screen-space refraction.

Sources:

- [Mark Finch: Effective Water Simulation from Physical Models, GPU Gems chapter 1](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models).
- [Catlike Coding: Directional Flow](https://catlikecoding.com/unity/tutorials/flow/directional-flow/), [Waves](https://catlikecoding.com/unity/tutorials/flow/waves/).
- [Hello Games: Worlds Part I, water rendering and dynamic water](https://www.nomanssky.com/worlds-part-i-update/).

### Fresh water, salt water, and colour are not interchangeable

USGS describes streams as gravity-driven flow toward lakes and seas, with tributaries combining and groundwater sustaining some sources. NOAA describes estuaries as transition zones with variable mixing, salinity, and circulation. Ocean colour depends on wavelength-dependent absorption, sediments, and other particles—not simply salt concentration.

**Adaptation:** freshwater lakes/rivers have a different artistic palette and sediment treatment from marine water, with a gradual brackish blend at mouths. A normalized marine blend is metadata for appearance and classification, not a salt-concentration solver. No claim is made that all fresh water is green or every ocean has one clarity value.

Sources: [USGS: Rivers, Streams, and Creeks](https://www.usgs.gov/special-topics/water-science-school/science/rivers-streams-and-creeks), [Saline Water and Salinity](https://www.usgs.gov/special-topics/water-science-school/science/saline-water-and-salinity), [NOAA: estuary circulation](https://oceanservice.noaa.gov/education/tutorial_estuaries/est05_circulation.html), [why the ocean is blue](https://oceanservice.noaa.gov/facts/oceanblue.html).

## Medium-scope design

| Environment | Generation / flow | Visual and exploration intent |
|---|---|---|
| Freshwater lake | Supported basin with its own local level; calm current | Quieter surface, visible nearshore bed, swimming and diving |
| River | Meandering supported channel with a downstream current | Directional ripples, changing width, sediment and rapids |
| Waterfall | Connected downhill drop into receiving water | Downward water streaks and foam; not a floating decorative curtain |
| Estuary | River-to-marine transition | Broader mouth, gradual marine blend, mixed sediment |
| Ocean | Broad basin, shore/shelf/deeper floor | Larger wave motion, marine palette, substantial underwater space |
| Cave pool | Existing bounded underground water | Calm freshwater metadata; preserve cave darkness and dry walking routes |

The current field is steady-state but affects movement continuously. Water occupancy is generated deterministically; there is no unbounded runtime flood propagation, editable terrain, mass-conserving fluid solver, tide/weather system, erosion, boats, aquatic NPCs, fishing, oxygen meter, or drowning damage. Lava remains non-damaging scenery.

## Contracts and validation

- World Y remains `MIN_WORLD_Y=-64..WORLD_HEIGHT=80`, with an exclusive top and 144-block storage depth. Seafloor coordinates can be negative without changing that contract.
- Preserve water material ID 8 and the existing geology IDs. Add sediment IDs only at the end; block IDs and atlas tile IDs are distinct.
- A water sample describes the actual wet voxel's basin surface, bottom, current, aquatic biome, and marine blend. Missing residency is not invented water or air.
- CPU chunks and water metadata have the same bounded lifetime as GPU meshes. Physics reads resident samples first and falls back to deterministic sampling only when a chunk is unavailable.
- A lake above sea level needs a sealed bed just as an ocean does. Surface water does not indiscriminately fill caves or drain through a carved cap.
- A basin's terrain-blending halo is not its wet footprint. Filling an outer halo after it blends back to low ground creates unsupported vertical water walls. Validate wet/dry shoreline neighbors as well as solid beds.
- Swimming and currents use the same field as the rendered flow. Creative flight ignores both; returning to walking/swimming retains solid-body safety checks.
- River current direction varies through bends and gently returns edge samples toward the channel. Validate the continuous downstream trajectory against actual wet occupancy and non-increasing stage; rounding a vector to a cardinal step can select the wrong voxel at a bend.
- Observe real ocean extent/depth, freshwater levels, downstream continuity, connected waterfall water, and sediment coverage—not just enum labels.
- Compare direct, dense, batched, and neighbor sampling at positive/negative chunk boundaries. Verify partial tops, water/air faces, water/solid interfaces, winding, finite metadata, and shared wave corners.
- Read back GPU frames above and below freshwater/marine surfaces. Compare two water-animation times with **sky time and HUD held fixed** so clouds or the crosshair cannot pass a water-motion check.
- Compare freshwater and marine camera optics on the same underwater geometry as well as in different regions. A different terrain view alone cannot prove that the fluid-specific palette works.
- Compare the same submerged pose with the water draw pass on/off to prove the reverse-facing surface exists. Fog alone must not satisfy water-surface acceptance, and water metadata must not leave underwater optics active after surfacing.
- Exercise walking entry and held-Space exits on generated lake/ocean shores. Rearm a shore exit after a supported submerged step, while preventing repeated held jumps on dry land.
- Keep submerged diagnostic eyes close enough to the actual bed to resolve its texture; a barely submerged view across an entire lake measures fog more than underwater exploration.
- Check the old surface, cave, headlamp, entrance, and rock-cutaway scenes as regressions. Native presentation, controlled simulation, and GPU readback do not prove physical keyboard/mouse interaction.

## Scope boundary: what “flowing” means here

Generated waterways are already filled into a supported steady state. Their flow vectors animate water in the downstream direction and advect swimmers, including downward drops. They do not continually add and remove voxel water after a world edit, because world editing is not part of this explorer. This distinction keeps the upgrade bounded and independent of chunk load order while providing observable water motion and water-specific traversal.

The generator uses seeded catchments with bounded, rotated basin and channel templates, not a global reconstruction of every terrain watershed. Spawn selection stays dry, but there is no blanket 180-block exclusion zone: two smaller inland lakes make water common in the starting area, and the connected drainage/coastal system is translated 128 blocks inland without compressing its gradients. Basin beds remain sealed; surface entrances that would intersect their seals are rejected rather than leaking water. Shallow shelves, a deeper floor, and an island interrupt the marine basin, while multiple inland lake levels and a tributary/confluence lead toward an estuary.

## Observed acceptance results

The bounded default/seed-42 surveys find all five surface water environments and retained cave pools. The main freshwater lake is at mean Y **43 / 44**, respectively—well above sea level **18**. Before the nearby-water bias, the default seed had no surface water within 256 blocks. Sampling at four-block spacing now finds its nearest water about **66 blocks** away and roughly **7.5% wet coverage within 128 blocks**, with river and marine water inside 256 blocks. These are sampled observations, not exact shoreline areas or claims about every possible seed.

The observed marine footprints extend roughly **300 blocks across**, with a connected 100-block fully marine core, shallow shelves, an island, and a deeper basin exceeding **25 blocks**. Tests follow drainage to the marine outlet, sample the continuous current through bends, and check fractional tops and shared corner metadata across chunk seams.

Native Metal verification on an Apple M3 Max has exercised both default and seed 42, with surface/underwater captures for fresh and marine water, all cave families, lamp-on/off views, natural entrances, and rock-cutaway restoration. In the river diagnostic, an idle swimmer moved approximately **0.71 blocks downstream in 0.5 seconds**. Water-only animation changed the actual GPU pixels with the sky clock held fixed; alternative freshwater/marine optics also changed the same submerged view.

The final acceptance pass also isolates the water draw pass for above/below-water views and checks that ordinary air optics return after surfacing. Generated shore traversal exposed and fixed an exit-latch issue on stepped ocean shallows; both the actual shore and a focused controller regression now exercise that case.

The final native runs retain the display-matched **165 Hz** limit: the default development run presented **360 frames at about 140 FPS**, and seed 42 presented **246 frames at about 150 FPS**. These are bounded smoke-run observations, not a guarantee that every view sustains the cap.

The final 55-chunk release surveys measured approximately **1.03 ms generation / 1.18 ms meshing per chunk** for the default seed and **1.04 / 1.20 ms** for seed 42. The ocean diagnostic rendered and completed in approximately **0.81–1.17 ms median**, with p95 around **1.6–1.7 ms**, over 32 offscreen samples per seed. These measure bounded scenes and sampled chunks, not total interactive frame cost or physical input latency. Numerical acceptance excludes HUD geometry and accumulates readback statistics in `f64`.

Closeout checks passed **341 unit/integration/doc tests** and **32 explicit example tests**. The normally ignored generation-timing diagnostic was also run separately and passed; no functional acceptance test was skipped. Formatting, all-target Clippy, a fresh Rust Analyzer diagnostic scan, WGSL validation, and the locked release build passed. The persistent MCP language server continued reporting stale pre-integration import errors, so those cached reports were not treated as current compiler evidence.

Physical mouse capture, keyboard swimming, and hands-on bank exits still require a human playtest: macOS event-posting access is unavailable, and permissions were not changed.
