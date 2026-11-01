# M2 audio contract

**State:** M2-01 frozen, 31 October 2026; exact CPAL dependency approved by the owner.
The owner's “Looks great, keep going” authorizes preparing milestone two; it is
not evidence of a physical expedition or listening acceptance. M1-05 remains
unchecked. The owner selected **“Approve CPAL (Recommended)”** for the exact
addition below. Implementation is now delivered; see [audio evidence](audio-evidence.md).
Listening acceptance remains a separate, unreported gate.

## Outcome and ownership

Restrained movement/gathering feedback and distinct inland/coastal/freshwater/
cave/underwater sound. No music, wildlife, voices, weather, acoustic simulation,
gameplay costs, new framework, or item/world-save expansion. Sound cannot affect
gameplay success.

Inspected source determines these boundaries:

- `Game::update` delegates to bounded player integration (0.25 seconds maximum,
  steps no longer than 1/120 second). An internal step readout must report actual
  supported displacement/wet state, not keys, render FPS, or airborne travel.
- `Game::interact` advances existing pickups, mines, then consumes a placement
  press. Emit break/place after successful `edit_block`; emit collection only
  after a positive accepted inventory transfer, not a drop spawn/merge.
- `Game::environment` already distinguishes actual camera water/mean surface
  and edit-aware sky exposure from biome labels. A separate bounded resident
  water readout is needed to hear surf from dry land.
- `App::simulate` only advances focused, captured play. `release_pointer`,
  `install_scene`, and `exiting` must notify audio immediately, even with no
  future redraw or when native Quit bypasses `Drop`. `redraw` consumes feedback
  once and refreshes profiles after chunk insertion/eviction. Menu/loading are silent.

`game/feedback` owns transient CPU events, cadence, material mapping, and bounded
environment sampling. Private `app/audio` owns samples, the CPU mixer, native
output, and preferences. The same mixer runs against fake buffers in tests.
`hud/` borrows plain controls/readouts; no native audio crate belongs in a CPU
game/world/player/HUD layer. No generic event bus or extra Cargo package.

## Approved backend

```toml
cpal = { version = "=0.18.2", default-features = false }
```

- Verified against crates.io metadata and tagged source: [CPAL 0.18.2](https://docs.rs/cpal/0.18.2/cpal/),
  released 16 August 2026, Apache-2.0, Rust minimum 1.85. macOS uses CoreAudio.
- One output stream; no microphone access, external program, recording backend,
  codecs, music scheduler, or optional JACK/ASIO/PipeWire features. Platform
  bindings and `dasp_sample` are necessary transitive dependencies.
- Rodio 0.23.0 was considered. Its source queues/decoding are unnecessary for
  this small generated bank; CPAL keeps the actual bounded mixer under project
  control and directly testable without hardware.
- Use a supported/default device sample rate, preferring stereo f32 output;
  handle f32/i16/u16 conversion and mono fallback. Unsupported formats, absent
  hardware, or stream failure give a once-reported silent fallback. Do not
  change system volume or require a particular device.
- One native worker handles initialization/stream ownership outside gameplay
  and render callbacks. Pass a finite initialization timeout; tagged CoreAudio
  honors it for sample-rate negotiation, not every OS call. Do not promise a
  universal OS timeout. Main-thread shutdown must not wait indefinitely.
- Device/stream failure clears pending actions and suppresses output; report
  once, not per frame. Restart is the initial retry route; no reconnect framework.

The owner approved this exact addition before Cargo/lockfile edits and M2-02.

## Original sample/provenance plan

Generate deterministic original samples once at startup: filtered seeded noise,
short damped resonances, and slow periodic envelopes. No copied recordings,
game clips, reference-site media, downloads, or external asset packs. Record
creator (RustCraft project/agent-authored synthesis), source/seed/parameters,
project MIT license, and modifications in `assets/audio/README.md`. Reproducible
source is the asset; no binary files, per-frame decoding, or runtime sound-file I/O.

- Mono f32 at 48 kHz; linear output resampling; preload once.
- At most 32 action clips, each at most 0.75 seconds. Three footstep variants per
  surface plus mapped mining/break/place, pickup, entry, and swimming textures;
  deterministic variation avoids immediate repeats.
- Four seamless environmental beds: wind/surf/freshwater/cave, initially
  24/31/29/37 seconds with different modulation periods. Underwater is a smoothly
  low-passed/rebalanced listening state, not a fifth loop.
- Decoded bank at most **32 MiB**. Tests measure delivered memory/durations,
  finite values, DC bias, peak/RMS, and seam continuity. Those measurements do
  not establish that synthetic surf sounds convincing.

## Action semantics

Events carry a category, material family where needed, and finite world position.
They are transient, bounded, never saved or replayed on Continue.

| Event | Real condition | Initial cadence |
| --- | --- | --- |
| Footstep | Supported horizontal movement, not flight/swimming | One per 2.0 accumulated blocks |
| Mining | Mineable resident target with positive elapsed work | At most once per 0.20 simulated seconds; target changes cannot bypass it |
| Break | Voxel successfully changed to air | Once per success; omit simultaneous final mining tick |
| Place | Validated edit and one consumed held block | Once per success |
| Pickup | Inventory accepted at least one item | Coalesce simultaneous transfers; no full-inventory/merge sound |
| Water entry | Body enters meaningful actual water contact | Hysteretic transition, not every submerged frame |
| Swimming | Wet displacement with swimming input | Distance based, at most three strokes/second; no idle/current-only strokes |

Four surface families: stone (rock/ores/bedrock), earth (grass/dirt/moss/clay/
leaves), sand (sand/gravel/snow), wood. Air/liquids cannot support steps. Check
actual supporting footprints with half-open/f64 bounds, including edge support,
negative seams, and player edits; no biome/target/generated-cap shortcuts.

The player step readout makes cadence independent of render partitions and
excludes airborne travel. Reset distance/contact/mining latches on respawn,
flight changes, pause/focus, and scene changes. A teleport is not walked distance.

## Environment sampling and mixing

- Refresh nearby water at most **4 Hz**, with no catch-up burst. Bound work to
  a 17×17 resident-column grid (four-block spacing, 32-block radius), including
  the listener column (289 reads). Read fluid metadata and confirm actual wet occupancy.
  Unknown/evicted columns are not sources. No per-frame procedural generation
  or landmark/world search, and no chunk residency owned by audio.
- Ocean and the marine Estuary fraction supply surf; Lake/River/Waterfall and
  freshwater Estuary fraction supply freshwater. CavePool is not an outdoor
  source. Horizontal distance and restrained vertical falloff allow surf from
  a dry cliff/cove, not only underwater. Labels alone never enable a source.
- Wind follows sky exposure; shelter attenuates outdoor beds and enclosed
  underground space adds subdued cave ambience. Headlamp does not change sound.
  Underwater filtering follows actual camera fractional mean-water occupancy,
  not swimming state, a biome label, or visual waves after surfacing.
- Timestep-stable exponential profile fades, initially a 0.5-second time constant;
  short smooth underwater transition. Keep only fixed gains/nearest-source
  readouts. Eviction removes a contribution on the next bounded sample.

## Budgets and lifecycle

- **16 total voices:** four fixed environmental slots plus 12 transients.
  Successful actions/entry displace the oldest lower-priority mining/step/stroke;
  reject excess low-priority requests. Use a short release on replacement.
- **64 pending events** at each producer/handoff. Coalesce collections/rapid
  matching requests; overflow retains recent successes over repetitive feedback.
  Expire transients older than 0.25 seconds; no resume catch-up burst.
- Callback owns fixed mixer state and reads bounded latest-state handoff without
  waiting. No callback decoding, device creation, I/O, procedural generation,
  logging, or growing collections. Main thread never waits on a callback lock.
  Mute/lifecycle suppression bypasses full queues; epochs reject old scenes.
- Defaults master **0.55**, actions **0.70**, ambience **0.50**, all finite in
  `0..=1`. Restrained clip/bus gain and a finite soft limiter retain headroom;
  worst-case mixing is measured, comfortable loudness needs listening. Mute is
  exact zero output, not only a UI flag.
- Pause/inventory/focus clears transients immediately and fades beds to silence
  within 0.10 seconds without redraws. Resume fades fresh current state, never
  queued old actions. A stale main-thread heartbeat fades output rather than
  leaving an old ambient bed indefinitely audible.
- Scene replacement clears voices/loops before the new world. Native Quit,
  error exit, and shutdown suppress output and stop/drop the stream, not only `Drop`.

## Controls and separate preferences

A compact pause Audio panel exposes mute and stepped master/action/ambience.
Shared hit-testing/keyboard navigation stays usable at 640×400. Closing controls
returns to pause, not automatic capture; Resume retains deliberate authorization.
Clear held input, and do not let UI clicks become mining/placement presses.

Use bounded versioned `audio.prefs` in RustCraft's per-user application-data
directory, separate from `world.save`. Read once; write only after deliberate
settings changes using same-directory atomic replacement. Invalid/unreadable
preferences use safe defaults and report once. Failed writes retain in-memory
settings and cannot prevent saves or erase progress. Tests/ordinary GPU smoke
disable preferences storage. No world format/generation version change.

## Tests and acceptance

- Fake output runs the actual mixer: absent output, limits/eviction, finite
  conversions, malformed gains/time, exact mute, seams/resampling, stale state,
  pause/resume, scene/shutdown clearing, and deterministic decisions. No devices.
- Action tests: varied timestep partitions; idle/walls/flight/air/swimming;
  edited/edge support; successful/failed interactions; full inventory; water
  hysteresis; no snapshot/clear replay. Environment tests: dry coast/inland,
  freshwater versus marine, cave/roof, camera entry/surfacing, negative coordinates,
  bounded reads, eviction, and fade partition invariance.
- Isolated preferences/UI failure tests preserve sentinel world-save bytes.
  Run locked fmt/Clippy/tests and native GPU smoke/readback after UI/executable
  changes, silently. Revalidate the concurrent renderer rather than reuse old captures.
- Provide a separately selected bounded **audible** check, no player save or
  pointer capture. Ordinary smoke never opens audio. Device callbacks, generated
  buffer measurements, and what a person heard are separate evidence.
- M2-05 still needs the owner listening pass: walk different materials, mine/
  place/collect, approach the dry coast, enter freshwater/ocean, surface, visit
  a cave, pause/mute/alt-tab/switch sessions, and quit. Accept loudness, distinction,
  repetition, transitions, and silence on exit. Fake buffers/GPU pixels/device
  initialization do not certify it; M3 remains behind that gate.
