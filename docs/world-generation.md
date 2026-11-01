# World generation: research and design

This is a deliberately bounded, exploration-first interpretation of procedural worlds and geology—not a replica of another game's generator or a geological simulation. The world remains seeded, streamed, CPU-generated, and block-shaped, with Y=-64..80 and no additional dependencies.

## What the research changes

### Minecraft: combine different families of space

Minecraft's modern underground combines large noise cavities, winding passages, smaller conduits, retained rock pillars, and older carvers. Frequency, hollowness, and thickness vary rather than giving every room one scale or shape. Terrain and decoration are separate steps; aquifers decide fluids and separating rock rather than flooding every cavity to sea level.

**Application:** distinguish passage width from height, create more than one cave silhouette, retain interesting solid rock inside large spaces, and keep local pools supported. More noise alone would still make every place feel statistically similar.

Sources: [Mojang's 21w06a introduction](https://www.minecraft.net/en-us/article/minecraft-snapshot-21w06a), [Minecraft Wiki: caves](https://minecraft.wiki/w/Cave#Noise_caves), [world generation](https://minecraft.wiki/w/World_generation). The Wiki documents mechanics; it is not an official implementation specification. Mojang's article was located through search, but its full page timed out during this research.

### No Man's Sky: coherent scale and bounded generation

Hello Games' GDC sessions describe mathematical terrain synthesis and a continuous pipeline from voxel generation through polygonization, texturing, population, and simulation. The useful lesson for this project is to preserve coherent, independently reproducible world data while spending work near the player—not to implement planets, a new engine, or copy undocumented proprietary algorithms.

**Application:** broad structure, medium-scale relief, and small detail have separate roles; generated voxels remain independent of the renderer. Generation and meshing are measured separately, with bounded residency and worker queues.

Sources: [Sean Murray, *Building Worlds Using Math(s)*, GDC 2017](https://gdcvault.com/play/1024514/Building-Worlds-Using), [Innes McKendrick, *Continuous World Generation in No Man's Sky*, GDC 2017](https://gdcvault.com/play/1024265/Continuous-World-Generation-in-No), [Game Developer's account of the pipeline](https://www.gamedeveloper.com/programming/video-how-continuous-world-generation-works-in-i-no-man-s-sky-i-). The readable session descriptions were reviewed; video-only details are not treated as verified implementation facts.

### Deep Rock Galactic: constrained structure, then variation

Ghost Ship's developer article explains room templates built from overlapping volumes and planes, inner/outer noise envelopes, optional protrusions, and defined exits. Rooms can be warped, mirrored, and overlapped. Biomes affect geometry, lighting, and debris, not merely wall color. Traversal, intuitive wayfinding, and dramatic experience are explicit goals.

**Application:** retain trustworthy connecting routes, but vary the surrounding envelope and interior. A gallery, fissure, columned hall, and branching network should differ in silhouette and navigation. Decoration must not erase the playable route.

Source: [Ghost Ship Games, *Below Decks: Cave generation*, 6 September 2024](https://store.steampowered.com/news/posts/?feed=steam_community_announcements&appids=548430&enddate=1726743173). This is a developer-authored article, not an inferred fan explanation.

### Procedural graphics: distort a useful structure, not its guarantees

Domain warping evaluates a shape at displaced coordinates, `f(p + h(p))`. Multiple scales add relief and detail; independent seeds and anisotropic scaling reduce correlated or uniformly round results. A density representation can describe shelves, arches, and multiple surfaces that a heightfield cannot.

**Application:** bounded, seeded shape variation, asymmetric cross-sections, and restrained relief. Expand feature bounds for every displacement. Keep the existing voxel mesh rather than switching to marching cubes or a GPU generation system.

Sources: [Iñigo Quilez: domain warping](https://iquilezles.org/articles/warp/), [fBM](https://iquilezles.org/articles/fbm/), [Ryan Geiss: GPU Gems 3, chapter 1](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu), [Red Blob Games: noise-based terrain and its limitations](https://www.redblobgames.com/maps/terrain-from-noise/).

## Geological vocabulary, simplified for play

Natural solution caves develop along joints, faults, and bedding surfaces in soluble rocks. Broad, roughly horizontal passages often record former groundwater levels; descending shafts and narrow, tall canyons occur above the water table. Lowering the water table can leave several abandoned passage levels. Breakdown produces fallen blocks and irregular chambers. Calcite deposition produces supported stalactites, stalagmites, columns, and flowstone.

These observations motivate several silhouettes:

| Landform | Geometric intent | Explorer experience |
|---|---|---|
| Gallery | Broad, comparatively low, irregular passage | Long sightlines, changing width, side alcoves |
| Rift | Tall, narrower fracture/canyon | Vertical scale, ledges, views across levels |
| Cathedral | Large hall with retained rock/columns | Interrupted vistas and recognizable landmarks |
| Labyrinth | Branching, looping conduits | Choices and alternating constriction/opening |

Folded or tilted material layers give exposed walls continuity. Limestone, slate, basalt, and calcite provide distinct rock/deposit palettes alongside the existing deep stone, ores, moss, and crystals. These are stylistic approximations: not every cave material/shape combination implies a physically correct formation history. Luminous crystal growth is intentionally fantastical. Real moss requires light; underground lush scenery is a creative aesthetic rather than a biological model.

Lava tubes are generally formed by draining molten flows beneath a solid crust, often near the surface. Deep lava pockets in RustCraft remain fantasy scenery; depth alone is not presented as a scientifically accurate lava-tube origin.

Sources:

- [Davies and Morgan, USGS, *Geology of Caves*](https://pubs.usgs.gov/gip/7000072/report.pdf), reviewed via extracted PDF text. Historical cave-size rankings in this publication are not used as current facts.
- [British Geological Survey: how caves form](https://www2.bgs.ac.uk/mendips/caveskarst/caveform.htm).
- [National Park Service: solution caves](https://www.nps.gov/subjects/caves/solution-caves.htm), [Lehman Caves geology](https://www.nps.gov/grba/planyourvisit/cave-geology-in-depth.htm), [lava caves/tubes](https://www.nps.gov/subjects/caves/lava-caves-or-tubes.htm).
- [Paris et al., *Synthesizing Geologically Coherent Cave Networks*, Computer Graphics Forum 40(7), 2021](https://hal.science/hal-03331697/document), DOI [10.1111/cgf.14420](https://doi.org/10.1111/cgf.14420), reviewed via extracted PDF text. The paper separates geological network construction, conduit classification, and warped cross-section geometry. RustCraft takes that separation as inspiration; it does not implement the paper's anisotropic shortest-path simulation.

## Creative phasing: show real rock without rebuilding the world

Ordinary voxel meshing removes faces between neighboring solid blocks. Back-face culling also hides an exposed surface when viewed from inside its solid side. Simply drawing double-sided surfaces solves only the latter; it cannot show buried blocks that have no mesh.

The cutaway therefore combines:

1. The regular opaque cave/surface scene.
2. A translucent reverse-face pass while creative flight's camera is inside known solid terrain.
3. A bounded, local voxel view that traverses actual resident block data and composites a few textured grid boundaries, with subtle block edges and restrained opacity.

The small voxel volume is rebuilt only when its spatial anchor or overlapping resident data changes. It never generates terrain on the render thread, treats missing data distinctly from air, and is evicted with the same chunk residency. Walking, outdoor flight, air, and scenery liquids keep their ordinary rendering. No terrain is removed, no collision changes, and no new key is required.

Outdoor flight and low cave entrances also need different background fallbacks: fog-colored sky hides far unloaded surface terrain, while a downward view near known ground fades dark. Resident ground clearance ignores tree crowns, so a forest canopy is not mistaken for a subterranean ceiling.

Reference: Amanatides and Woo, *A Fast Voxel Traversal Algorithm for Ray Tracing*, 1987; [annotated modern explanation](https://m4xc.dev/articles/amanatides-and-woo/). Axis-aligned rays, negative coordinates, exact-boundary starts, corner ties, and unknown voxels require explicit tests.

## Acceptance checks

This section and the observations below describe the earlier underground/explorer upgrade. Its exclusions of persistence, mining, and inventory are historical scope limits, not the current game's capabilities; see `README.md` and `docs/architecture.md` for the saved survival foundation. Historical timing/results are not fresh roadmap validation.

- Same seed, coordinates, and generation order produce identical blocks; direct samples, projected columns, and neighboring chunks agree.
- Keep the bedrock boundary, dry spawn, surface biome/water coverage, supported liquid basins, and existing movement contracts.
- Keep a standing-height route from natural entrances into shallow/deep cave space. Extra vertical/rough side areas can reward creative flight.
- Measure actual height/width/open-column distributions and exposed materials—not just landform enum labels.
- Inspect native GPU views of every cave silhouette, geology, entrances, and phasing. A surface screenshot or successful shader submission is insufficient.
- Compare lamp-on/off readback separately from phase visibility; phase visualization must not turn ordinary caves into permanently bright x-ray views.
- Keep generation/meshing latency, geometry, cached CPU voxels, and the phase volume bounded. No planet-scale erosion, fluid simulation, persistence, mining, inventory, or new dependencies in this pass.

## Observed results

The default-seed diagnostic survey samples 45 chunk columns around spawn, biome views, and geometry landmarks. It is a bounded sample, not a claim about every seed or the entire world:

- Gallery open-column height p10/median/p90: **2 / 8 / 10** blocks; widths **8 / 24 / 38**.
- Rift p90 height: **25** blocks, including a **31-block** open column.
- Cathedral median/p90 height: **20 / 20** blocks, with separated air intervals and full-height retained rock.
- Labyrinth observation: **219** junction samples and **3** rock islands surrounded by narrow-corridor loops. These are local geometry measurements, not a global graph reconstruction.
- The observed material survey contains **319 supported / 0 unsupported calcite tips** and **156 attached / 0 floating crystal voxels**. Attachment tests follow six-neighbor material components to host rock, rather than checking color labels alone.

After batching nearby halo columns, optimized-development generation and meshing measured approximately **0.79 ms / 0.86 ms per chunk**, respectively. The previous generator's corresponding sample measured **0.31 ms / 0.88 ms**. Sampling locations and geometry changed, so these are diagnostic comparisons, not identical-work benchmarks.

On the Apple M3 Max Metal backend, the bounded cutaway scene rendered and completed in approximately **0.74 ms median / 1.31 ms p95** over 48 offscreen samples. This measures a small diagnostic scene with the same shaders, not interactive input latency or total native frame time. The phase voxel cache has one **108 KiB** upload allocation, and resident CPU block data follows the same eviction policy as the GPU meshes.

Physical mouse/keyboard event injection is unavailable in this environment. Native presentation, controlled simulation, GPU readback, and numerical tests do not substitute for a hands-on playtest.
