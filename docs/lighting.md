# Unified voxel-aware lighting

The renderer retains the original pixel-art atlas and forward scene layout. Lighting adds real sunlight visibility, approximate diffuse transport, an HDR display transform, and a shared day–night solar sample without changing world generation, materials, collision, or water occupancy. Solar phase is saved separately from rendering.

## Sunlight

- Two 2048×2048 `Depth32Float` maps cover a camera-centered near region and the fog-visible far region. Coverage is independent of view rotation, with light-plane texel snapping and 64-block depth anchoring. Each solar sample constructs its light rotation at the origin, then translates it; reconstructing it from translated world points introduced small camera-dependent rounding changes even after texel snapping. The solar orbit is tilted so noon cannot collapse the light basis.
- Caster selection uses each light frustum, including resident geometry outside the camera frustum. Water, sky, first-person hand, and HUD do not cast; opaque terrain/trees do. Pickups receive shadows but do not cast in this milestone.
- Receivers use comparison sampling with a 3×3 filter, normal/depth bias, blended near/far coverage, and a fade to original skylight at the residency limit. This is filtered shadow mapping, not physically varying penumbrae.
- Exposed-face meshing can omit a buried roof's upper surface when it extends past residency. A conservative local voxel segment supplements sunlight visibility on roofed receivers; outside that local volume, unlit skylight is the safe fallback. Closed underground must not become sunlit at render distance two.
- Direct warm sunlight and indirect cooler fill are separate. The mesh's `light` value is still vertical sky transmission, used for fog, water sky reflection, and headlamp weighting; cast shadows never rewrite that value.

## Solar clock contract

`game/time` owns a wrapped `f64` hour and a validated 0–120× debug multiplier.
The ordinary cycle is 1,200 active simulation seconds; hitches are capped at the
same 0.25-second gameplay limit. The application never advances it during any
overlay, loading, or focus/capture loss. `elapsed` still drives ordinary water,
cloud, pickup, and lava animation; it is not multiplied by the solar rate.

`lighting/celestial` supplies continuous linear-light sky/cloud palettes,
warm low-sun radiance, a tilted unit direction, and dim night sky irradiance.
At night direct sunlight and its volumetric source are zero, not merely a dark
sky texture. Shadows and probe transport use the same moving direction as
surfaces/water/sky. Twilight smoothly changes color and intensity; stars and a
decorative moon do not add another shadow pass. Voxel skylight, cave classification,
and fog distances remain occlusion/residency contracts rather than time-of-day
tests. Night headlamp weighting uses remaining daylight, allowing exposed outdoor
surfaces to receive the lamp without classifying them as caves. Local lantern,
lava, and crystal light is not multiplied by daylight. Exposure remains bounded.

Large solar phase jumps (over fifteen game minutes) and sun-horizon crossings trigger one full bounded probe
refresh; ordinary advancing time retains the spatially interleaved 512-probe
budget. Accelerated ticks increase the update count in complete 512-probe batches,
up to 4,096, targeting at most 0.06 solar hours (0.9 degrees) per stale sweep.
This trades bounded extra debug-time work for responsive moving bounce rather
than broad lagging daylight patches. Reference readback saves/restores the previous
solar sample alongside probe/exposure timing so it cannot mask interactive stale light.

The time-only console (`Slash` or backtick) pauses while editing, clears held
movement/mining, and consumes keyboard/mouse events instead of forwarding them
into world actions. A valid mutation closes it; query/help/errors stay visible.
Only a console opened from captured play retains recapture permission, and focus
loss clears that permission. F6 cycles bounded showcase rates. Saved solar hours
resume at 1× unless the launch explicitly supplies `--time-speed`; format 3 loads
at 10:00 with all its progress intact and saves as format 4.

## Bounded diffuse transport

`lighting/volume` copies a 64³ local `Rg8Uint` volume: voxel material ID and full-height vertical sky transmission. It borrows dense residency only, rebuilding on an eight-block anchor change or overlapping chunk revision. An overlap band retains the anchor while the eye remains 24–40 blocks from its origin on each axis, preventing small reversals at grid edges from repeatedly scrolling. Columns above the local Y interval still occlude sky; missing residency is unknown. The Y origin is clamped to `MIN_WORLD_Y..WORLD_HEIGHT`, not confused with storage depth.

The GPU stores 4,096 probes, four blocks apart, with six directional irradiance lobes. Each valid probe uses five fixed rays per lobe and at most 40 blocks/112 voxel crossings. Blocked nominal probes relocate at most two blocks to nearby air. Rays carry:

- sky radiance when the full-height transmission permits it;
- one diffuse bounce from voxel-ID reflectance and the same shadowed sunlight used by visible geometry;
- bounded warm lava or blue/cyan crystal radiance, attenuated with distance.

This is a sparse one-bounce approximation, not recursive multi-bounce GI. Material reflectance is a representative linear value, not a second sampled texture atlas. Water attenuates transported light. Foliage is opaque to these local rays and to shadow casting; its original vertical sky transmission still supplies canopy fill.

At a surface, up to eight probes are interpolated with directional weighting only when a conservative one-block-resolution segment connects that probe to the receiver. Edge/corner-tied DDA crossings check all touched cells. Solid walls and unknown data reject contributions; probes in neighboring rooms cannot simply bleed through the wall. The boundary fades back to the established sky fill.

Ordinary frames update **512 probes**, visiting the full grid in **eight frames**. Each batch visits one probe in every 2×2×2 neighborhood, rather than refreshing two spatial Z slabs. Probe storage uses world-grid modulo addresses: a camera-volume scroll retains overlapping probes and computes every entering probe plus the ordinary batch **before drawing**. Changes to old transport residency, edits, a new scene, or a nonoverlapping teleport recompute all 4,096 probes in that frame; no stale wall/emitter light or eight-frame missing-light flash is shown. This bounded extra work is deliberate. Probes have no temporal random sampling or history blending. Full diagnostic updates are intentionally deterministic; movement acceptance preserves/restores the interactive cache around each independent reference render, so the reference cannot secretly refresh the next movement frame. Very small features and narrow corridors may have sparse/absent valid probes; fallback preserves safe cave darkness rather than inventing light.

## HDR and display

Sky, terrain, water, cutaways, pickups, mining feedback, and the first-person hand render into linear `Rgba16Float`. Emissive radiance can exceed 1.0 without clamping to an 8-bit target. A quarter-resolution bright-pass/blur adds restrained bloom; an ACES-style filmic curve compresses highlights. Exposure follows available daylight smoothly but stays in `0.8..=0.96`, deliberately preventing automatic exposure from brightening an unlit cave or turning night into day. This is bounded environmental exposure, not image-histogram metering.

The final surface applies sRGB encoding exactly once (explicitly only on a non-sRGB fallback surface). HUD/menu geometry is drawn afterward, outside bloom and tone mapping. Size-dependent HDR/bloom targets are recreated with depth attachments on resize. Native, explicit offscreen, and screenshot paths use the same passes.

## Shadowed clear air

One moving sun illuminates a uniform, deliberately thin atmosphere. `lighting/atmosphere` owns the CPU parameters: extinction **0.0014 per block**, maximum sun share **0.32** (fading with daylight), forward-scattering anisotropy **0.55** with a capped phase peak, and **64 quadratic ray intervals**. Intervals are anchored to the active fog horizon, not scaled by a changing receiver depth: only the clipped tail moves when the receiver moves, not every foreground visibility sample. Transmission is above 97% over 20 blocks and above 83% over the maximum 128-block march; this is clear air with occasional beams, not weather or a permanent fog bank.

The forward scene writes a full-resolution `Rgba16Float` guide beside HDR: world-ray distance, streaming/cave-fog transmission, cave classification, and air participation. It includes pickups and the visible water interface but excludes HUD, mining feedback, and the first-person hand. Sky uses a bounded far distance instead of reconstructing an infinite ray. An unfiltered **minimum depth in each 2×2 footprint** chooses the half-resolution endpoint, staying in front of foreground block silhouettes. The ray remains at the footprint center when the minimum-depth pixel changes; the upscale uses that same center convention.

Each midpoint reuses the existing blended sunlight depth cascades, without the surface-only normal-facing test. In the local resident voxel volume, only known air scatters: solids, liquids, and unknown data cannot. Conservative sunward voxel segments supplement buried roofs omitted by exposed-face meshes. Full-height sky transmission at the segment exit also rejects sealed rock above the local Y range. A roofed camera does **not** globally disable scattering: a verified aperture can still illuminate air in front of it. Outside local coverage, fully enclosed cave rays use a dark fallback.

Downward sky-background rays also check a bounded set of resident air samples for full-height roof occlusion and geological side walls. This catches broad cave mouths whose interior lies beyond the camera-center cavity hint and prevents sky-colored unloaded patches. Open coasts, high flight, and canopy leaves do not satisfy that enclosure test; their background continues to meet outdoor fog. Conversely, an upward ray from a roofed camera may recover outdoor sky only through a clear resident segment to an exit with full-height sky transmission. Looking out of a cave is not the same as lighting an enclosed cave.

Near clear-air extinction and existing long-distance streaming/cave fog are a **single atmosphere**. The forward shader reserves the sun's share of the same Beer–Lambert extinction instead of adding sun energy on top of complete ambient fog; the march supplies that share only where the sun is visible. Existing streaming fog still hides residency and roofed surfaces still fade dark. The ray integrates the streaming fade along its length, so a visible foreground beam survives even if its distant receiver has faded to cave darkness. The procedural sky reserves the same bounded foreground sun share instead of being fogged a second time.

A 3×3 spatial/depth bilateral gather upscales air. Depth-incompatible samples are rejected; a one-pixel feature with no compatible half-resolution sample re-marches that pixel, rather than borrowing background light and forming a halo. Composition is additive radiance **into HDR before the first-person hand, bloom, and tone mapping**. Underwater, lava, and embedded creative cutaway views retain their original media with air disabled. All size-dependent air resources rebuild on resize, including odd dimensions. There is no temporal history, animated noise, weather, or volumetric cloud system.

Sky, air, and cutaway rays use an eye-independent inverse projection/view rotation. Reconstructing a distant world point, dividing by its tiny homogeneous W, then subtracting the camera position amplified matrix rounding into ray jitter during otherwise ordinary translation. The shared direction projection remains byte-identical while the camera translates at a fixed orientation; visible geometry retains its ordinary world projection.

This first version does not model multiple scattering, point-light volumes, varying aerosol density, or physically refracted water paths. Leaves retain their existing opaque shadow-caster behavior: shafts come through actual canopy gaps, not an invented foliage translucency model. Thin beams can be undersampled by the fixed march and sparse residency intentionally errs toward darkness.

## Texture stability

The original 16×16 source art and HUD icons are unchanged. GPU material tiles occupy separate array layers, each with a complete 16→8→4→2→1 mip pyramid averaged from the original texels in **linear light**. A smooth projected-texel-footprint transition keeps magnified art at exact texel loads and filters minification with trilinear mip selection and up to 8× anisotropy. Array layers prevent unrelated materials from bleeding into mip levels; no whole-atlas downsampling is used. This stabilizes distant grass/geology without adding camera jitter, temporal ghosting, or a full-frame blur.

## Verification

CPU tests cover material-ID tables/energy bounds, WebGPU cascade depth/coverage, shadow-texel stability, negative/extreme coordinates, full-height roof occlusion, resident cache invalidation, probe budget/sweep/reset, corner-safe ray visibility, bounded exposure, highlight rolloff, GPU-layout offsets, and half-float readback.

Solar tests also cover continuous/periodic palettes and finite cascades through
the full orbit, invalid command/rate/time rejection, simulation partitioning and
wraparound, every paused-screen boundary, animation/physics independence,
persisted phase with unsaved debug speed, and exact format-3 progress compatibility.
The GPU diagnostic holds pose, animation, water, and HUD fixed for five phase
captures. Day/night must differ substantially with contrast retained at night and
zero night air radiance. A complete 60× interactive sweep is checked against
independent full-grid reference frames. Sealed lava/crystal and lantern receivers
must retain their lighting when only solar phase changes, with bloom/air disabled
to isolate receiver light from the atmosphere's separately reserved sun share.
Receiver comparisons permit less than one 8-bit level of mean error and no
meaningful two-level changed-pixel area, accounting for bounded sky bounce and
half-float/output quantization rather than tolerating visible emitter dimming.

Solar GPU readback on Apple M3 Max/Metal, default and seed 42, measured same-pose
coastal scene luminance **0.3389 → 0.0280** and **0.3559 → 0.0307** from day to
night, with all central scene pixels changing and **zero nighttime sunlight air
radiance**. Outdoor headlamp, five phases, equal-elevation morning/evening lighting,
accelerated full/partial probe sweeps, lantern receivers, and existing lighting,
water, survival, and persistence regressions run in the bounded offscreen suite.
Offscreen frame readback does **not** verify native presentation or physical input;
the native attempt was blocked by the locked macOS session.

The normal smoke test also includes:

1. Same-pose sunlight-shadow and diffuse-bounce on/off comparisons with bloom disabled.
2. Sub-texel camera movement and eight-frame interactive probe convergence.
3. A sealed negative-Y room at render distance two with no headlamp.
4. Off-camera lava/crystal sources illuminating only non-emissive walls, with headlamp/bloom off and receiver RGB palette assertions.
5. Actual pre-tone-map HDR texture readback: finite, nonnegative radiance and highlights above 1.0.
6. A solid edited partition giving zero changed receiver pixels when emitter transport is toggled; reopening restores identical irradiance and emitter removal clears it.
7. Air-only half-float readback for an edited roof aperture with a roofed camera; closing the aperture must produce zero scattered energy, reopening restores it, and sealed positive/negative-Y rooms stay dark at minimum residency.
8. Half-resolution versus full-resolution air comparisons, including HDR errors on actual depth discontinuities and odd-size resize. Generated forest, entrance, and coastal captures retain contrast with bloom/headlamp disabled; underwater/lava rendering is invariant when air is toggled.
9. Existing cave lamp/darkness, water-animation/underwater, cutaway, coast at render distances 2/8/12, survival, persistence, resize, and native presentation checks.
10. Actual controller-driven seeded walking across lighting anchors with the interactive update budget, frozen water/sky/lava animation, HUD/headlamp/bloom off, independent same-pose full-update references, and stationary sweep checks. The `stability-walk` PNG is an animated GPU readback, not a desktop recording. A separate 4×4 subpixel reference compares filtered/nearest texture area error and variation on minified green surfaces. Edit checks compare the **first** interactive frame after closing a partition or removing an emitter.

Run the native check with an existing output directory:

```sh
cargo run --locked -- --smoke-test --screenshot /path/to/frame.png
```

Use the explicitly separate `--offscreen-smoke-test` only when native presentation is unavailable. GPU screenshots and simulated motion do not certify physical mouse/keyboard input or the subjective quality of movement/lighting transitions.

### Surface-lighting baseline before air integration, 31 October 2026

Apple M3 Max / Metal, with native presentation and GPU readback:

- `cargo fmt --all -- --check`, strict all-target Clippy, and the locked release build passed.
- `cargo test --locked`: **625 passed**, zero failures, one existing ignored test. The water-probe example adds six passing tests; default/42 CPU aquatic probes also passed.
- Default seed, render distance eight: **432 presented frames**, resize verified. Seed 42, render distance two: **251 presented frames**, resize verified. Both runs also exercised the coastal diagnostics at radii 2/8/12, all underground/water/cutaway scenes, and survival/save round trips.
- Sunlight-shadow removal changed 82.27% / 74.77% of the default/42 coastal comparison region; disabling the diffuse bounce changed 79.45% / 75.16%. Bloom was disabled for both sides of each comparison.
- The 512-probe interactive schedule converged in eight frames with **zero final mean pixel difference** from the full-update reference. Sub-texel camera motion had mean differences 0.000400 / 0.000371.
- Off-camera lava/crystals changed all pixels in the non-emissive receiver comparison region with headlamp/bloom off. The sealed wall reduced emitter-toggle differences to **zero pixels**; reopening restored identical receiver color and removing emitters returned room luminance to **0.0120**.
- Actual HDR readback found peak radiance **5.348**, with **53.5%** of the lava-view comparison region above 1.0 before tone mapping.
- Final default debug render-plus-completion samples: coastal median/p95 **4.93/5.93 ms**, ocean **6.07/6.78 ms**. These include deterministic full-grid updates and CPU submission/completion waits, not isolated GPU timestamp timings or an all-hardware performance guarantee. Uncontended earlier runs measured roughly 4–5 ms.

Physical input, subjective transition quality, and non-Metal hardware remain unverified. This lighting change adds no save-format or generation-version change.

### Shadowed-air acceptance, 1 November 2026

Apple M3 Max / Metal, actual native presentation and 2400×1520 GPU readback:

- Default seed at render distance eight: **314 presented frames**, resize verified. Seed 42 at distance two: **247 presented frames**, resize verified. Both passed the existing underground, aquatic, coastal, surface-lighting, survival, and persistence checks as well as air acceptance.
- The roof-aperture fixture illuminated **46.85%** of the air comparison region, with peak linear radiance **0.01849**. Closing it gave **zero scattered energy** and sealed-room sRGB luminance **0.0269**; reopening restored the original beam energy. A sealed negative-Y cave also gave zero air radiance. Toggling air changed **zero water/lava pixels**.
- Generated coast/entrance/forest half-versus-full-resolution mean display errors were **0.000012 / 0.000019 / 0.000052** for the default seed and **0.000011 / 0.000016 / 0.000056** for seed 42. Peak linear HDR errors on tens of thousands of actual depth-edge pixels stayed at or below **0.002441**. The separate 641×403 odd-size resize also passed.
- Real roofed cave-entrance views retained outdoor sky through the visible opening and produced nonuniform air radiance with peak **0.07428 / 0.02248**. The default outdoor entrance's unloaded interior sample stayed dark (HDR peak **0.01648**), rather than exposing a sky-colored patch.
- Canopy shadows reduced mean air radiance from **0.01316 to 0.01139** for default and **0.01468 to 0.01090** for seed 42; disabling surface shadows cannot stand in for this air-only evidence.
- Coastal render-plus-completion medians, seven samples with deterministic full-grid probe updates: ambient reference **7.15 / 7.19 ms**, shadowed air **16.93 / 17.20 ms** (default/42). These are total CPU submission/completion timings, not isolated GPU timestamps; native smoke measured roughly **71–75 FPS** at this resolution. This first bounded version trades extra GPU work for geometry-aware air, and performance on other hardware remains unverified.

Captures use the `air-aperture`, `air-sealed`, `air-forest`, `air-coast`, `air-entrance`, and `air-entrance-inside` suffixes. Matching `-ambient` captures restore the unshadowed share of the same fog, not a second fog effect. All air acceptance captures exclude HUD, headlamp, and bloom. No generation/save-version change or new dependency was needed for the air implementation.

Focused final checks: **51 lighting tests, 42 renderer tests, and seven air-acceptance tests passed**; the water-probe example's six tests and default/42 CPU probes passed. Release build and strict all-target Clippy passed. A whole-crate run passed **640 library tests** plus architecture/exploration/generation checks, then stopped at concurrently added `tests/processed_materials.rs::processed_blocks_mine_collect_place_save_and_reload_as_their_own_material` (expected pickup count 1, got 2). Whole-repository formatting reported unrelated crafting/inventory work and that new test file; rendering files passed targeted rustfmt. These are not recorded as whole-repository green checks.

### Image-stability pass, 1 November 2026

Apple M3 Max / Metal, native presentation at 1200×760 with 960×608 motion diagnostics:

- An actual 16-block spawn walking route reproduced the cache-clear flash: worst interactive versus same-pose full-update mean display error **0.026586**. Retained world-grid probes, immediate entering-shell population, anchor hysteresis, and interleaved refresh reduced it to **0.000793**, approximately **97% lower** on the captured default route. Seed 42 measured **0.000364**. Each diagnostic renders 96 actual controller poses, restores probe storage/sweep/exposure after references, and freezes animation, headlamp, bloom, and HUD.
- Reverse-path sampling measured **0.000541 / 0.001042** (default/42). The following stationary eight-frame sweeps measured **0.000000 / 0.000002**, rather than periodic disappear/reappear transitions. CPU checks cover negative and multi-axis world addresses, unique toroidal slots, interleaved neighborhood coverage, hysteresis, roof edits, eviction, scene changes, and nonoverlapping teleports.
- Across **10,501 / 4,106** minified green-surface pixels, a 16-position subpixel comparison reduced sample variation from **0.016913→0.013803 / 0.007793→0.005977** (approximately **18% / 23%**). Error against the original texture's pixel-area average fell **29% / 39%**. Nearby art retains original texels, not a full-frame smoothing filter.
- Canopy, cave-mouth, and coast stationary captures repeated with **zero display difference**. Isolated linear air changes for ±0.001/0.004-block motion were at most **0.000015 / 0.000022** (default/42) across those views. The fixed horizon lattice, centered half-resolution rays, matching upscale coordinates, and translation-free ray reconstruction also retained aperture closure/reopening, sealed cave darkness, water/lava isolation, and depth-edge acceptance.
- Closing a voxel emitter partition and removing emissive voxels both matched the full-update reference on their **first interactive frame**, with no stale irradiance or delayed flash. Existing native cave/water/coast/cutaway/lantern/survival/persistence acceptance passed. Default smoke presented **422 frames** at render distance eight; seed 42 presented **263 frames** at distance two; both presented their resize and completion phases.
- Animated `stability-complete-stability-walk.png` and `stability-complete-seed42-stability-walk.png` captures, a `stability-before-after.png` contact sheet, and matching smoke logs are under the approved OpenCode temporary directory. They are actual GPU readbacks, not desktop recordings. Lossless fast PNG compression keeps these diagnostic captures inside the unchanged 60-second smoke watchdog.
- Final whole-repository checks: **778 tests passed**, zero failures, one existing ignored diagnostic; strict all-target Clippy, rustfmt, `git diff --check`, and locked release build passed. The water-probe example's six tests and default/42 CPU runs passed. Rust LSP was used for navigation and diagnostics; two previously corrected diagnostic locations remained cached as stale errors while the current sources passed Cargo/Clippy and native GPU verification, so a completely clean LSP snapshot is not claimed.

Ordinary frames retain the 512-probe budget, but scrolls fill entering probes and scene/occupancy invalidations recompute the bounded grid immediately. This is an intentional stability/cost tradeoff, not a GPU frame-time guarantee. One expanded smoke attempt hit the absolute watchdog during slow animated-PNG encoding; it was not counted as a pass, and fast lossless encoding allowed subsequent native runs to complete. Physical mouse/keyboard feel, subjective smoothness on arbitrary scenes, and non-Metal hardware remain unverified. No dependencies, generator versions, save formats, or terrain fingerprints changed in this pass.

## Deliberate limits

No hardware ray tracing, path tracing, temporal denoiser, screen-space reflections, volumetric clouds, weather, or moon-shadow pass is included. Global illumination is local to the bounded camera volume, not the entire resident world. Bloom is an optical effect and never evidence that an emitter illuminates nearby geometry or air. Native Metal validation is required; other GPU backends still need hardware-specific playtesting.
