# M2 sound and ambience evidence

**State:** implementation and automated/native/hardware checks delivered;
**awaiting owner listening acceptance**. M2-01–02 are checked. M2-03–05 remain
unchecked because their responsive/distinct/pleasant/repetition gates cannot be
certified by fake PCM, a device callback, or a screenshot. The owner's later
“Sounds good, you can proceed” authorizes M3 progression without inventing that report.
The earlier M1 physical expedition also remains explicitly unverified.

## Delivered behavior

- Two-block supported-distance footsteps from the actual controller's bounded
  collision substeps, with stone/earth/sand/wood support selected from edited
  voxel footprints. Idle, walls, air/jumps, flight, and swimming are not footsteps.
  Supporting material is read only when a pulse is due, not every substep.
- Globally rate-limited mineable-target textures; one break/place/collection
  event only after an actual successful voxel edit/item transfer. Failed placement,
  full inventory, unmineable targets, and drop merges have no success sound.
- Hysteretic body-water entry and bounded distance-based swim strokes, excluding
  idle/current-only/blocked movement. Camera underwater listening is a distinct
  actual fractional-water state and recovers after surfacing.
- Wind/exposure, dry-cliff/cove marine surf, freshwater, estuary fractions, and
  subdued cave beds. At most 289 resident surface-column reads at 4 Hz; edits
  and eviction remove real sources. No generated-world search, audio-owned chunk
  cache, or procedural world generation in native callbacks.
- One owner-approved CPAL/CoreAudio output stream, initialized by a worker;
  unsupported/unavailable/failed output is nonfatal and once-reported. No
  microphone, codec, subprocess, music/weather/wildlife, or framework.
- Four environmental plus 12 transient voices; 64-event expiring/coalescing
  queues; priority replacement with short releases; finite normalized gains and
  headroom limiter. Callback and main-thread handoff never wait on one another.
  Atomic lifecycle epochs clear stale scenes; mute is exact sample silence;
  pause/inventory/focus loss fades within 0.10 s even without redraws. Stale
  heartbeat suppresses unattended output; native exit stops/drops playback.
- Pause Audio controls: mute, master/action/ambience in 5% steps. Common draw/hit
  regions and keyboard focus; closing returns to pause without capture or held
  world input. Independently bounded/versioned atomic `audio.prefs` never changes
  world snapshots or blocks world saves. Save failure exits the panel so its
  paused error remains visible.

## Source and compatibility

Owning files are `src/game/feedback.rs` / `feedback/`, internal
`src/player/observation.rs` / movement reporting, success hooks in
`src/game/{interaction,pickup}.rs`, actual surface-water readouts in
`src/world/{chunk,resident}.rs`, private `src/app/audio.rs` / `audio/`, app
frame/native/session/screens, `src/hud/audio.rs` / shared menu layout, and launch
options. Architecture guards reject native audio crates in CPU layers and
correctly distinguish external `glam::camera` from RustCraft's `camera` module.

`Cargo.toml` adds only the explicitly approved
`cpal = { version = "=0.18.2", default-features = false }`; Cargo locked 11
necessary additional/platform packages without a codec/framework. Original
source-generated MIT assets are documented in `assets/audio/README.md`.
World save **format 1 / generation 2** and append-only material IDs/goldens are
unchanged. All progress/preference tests use disabled or exclusively owned
temporary paths; smoke/hardware checks read/write no player save or preferences.
No commits, pushes, remotes, user-change resets, or corporate tooling.

## Reproduction and final CPU evidence

All local logs and GPU PNGs below use the existing approved output directory:

```text
/private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/
```

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
cargo test --locked app::audio -- --nocapture
cargo test --locked game::feedback -- --nocapture
cargo test --locked --example biome_probe --example cave_probe --example water_probe --example coast_probe
```

- **702 ordinary tests pass:** 622 library, 79 integration, one doc test;
  zero failures, one intentionally ignored timing diagnostic.
  `roadmap-m2-05-verified-tests.log`.
- Formatting, strict all-target locked Clippy, and locked release build pass:
  `roadmap-m2-05-verified-{fmt,clippy,release}.log`.
- **33 focused audio tests pass**, including the real fake-buffer callback,
  priority/expiry, contention, stale heartbeat/epochs, integer mono/stereo
  conversion, underwater recovery, failed output/terminated worker, exact
  mute/shutdown, atomically failed preferences, foreign staging collision,
  preserved world sentinel, and gameplay → PCM integration.
  `roadmap-m2-05-verified-audio.log`.
- **20 focused CPU feedback tests pass**: partitions, real motion/support,
  success/failure/collection, actual coast/lake, camera water/surfacing, bounded
  reads/extreme coordinates/eviction. `roadmap-m2-05-final-feedback-tests.log`.
  Additional UI/application tests cover finite draw/hit layout, gain boundaries,
  capture/input safety, and preference/world-save failure separation.
- **34 explicit example tests pass:** biome 7, cave 19, coast 2, water 6.
  `roadmap-m2-05-example-tests.log`.
- Rust LSP is not available in the tool catalog after its earlier broken-pipe
  failure; no invented LSP result. Cargo is the fresh compiler evidence.

## Actual bank and output measurements

33 clips: 29 short actions plus four beds; **24,892,796 decoded bytes
(23.74 MiB)**, below the 32 MiB bank limit. Mono f32 at 48 kHz; action durations
at most 0.65 s; bed durations 24/31/29/37 s. Original seeded synthesis is preloaded
once, including end/start overlap and DC removal. No runtime sound-file decoding.

| Bed | Duration | Peak | RMS |
| --- | --- | --- | --- |
| Wind | 24 s | 0.160 | 0.02762 |
| Surf | 31 s | 0.220 | 0.03059 |
| Freshwater | 29 s | 0.180 | 0.03203 |
| Cave | 37 s | 0.100 | 0.02052 |

Worst-case synchronized 12 entry transients plus all four beds, all settings at
one and no underwater attenuation: measured output peak **0.70242** after the
finite 0.9-ceiling soft limiter. No NaN/infinite/clipped output; source endpoints,
loop-seam jump versus interior changes, and DC bias pass. Evidence:
`roadmap-m2-05-verified-audio.log`. These are digital measurements, not acoustic
SPL, latency, comfortable volume, convincing surf, or repetition acceptance.

## Silent native GPU/fake-output checks

```sh
cargo run --release --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m2-05-verified.png
cargo run --release --locked -- --smoke-test --seed 42 --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m2-05-42.png
```

Apple M3 Max/Metal, macOS, pinned Rust 1.99.1, release, display-reported 165 Hz.
Final default **332 presented frames**, 289 chunks, 150.3 sampled FPS, resize
verified (`roadmap-m2-05-verified-native.log`). Seed 42 **324 presented frames**,
289 chunks, 141.1 sampled FPS, resize verified (`roadmap-m2-05-native-42.log`).
The 42 capture predates the final CPU-only cadence/numeric/exit hardening;
final default rechecks those changes. Sampled FPS is not a performance guarantee.

Both logs verify discarded real action/ambience PCM and pause/scene/mute/shutdown
without opening a device or preferences. GPU checks regress readable Audio/pause
UI, coast land/cove/arch/distances 2/12, edited private-save exact resumed pixels,
eviction/reload, freshwater/marine/camera surfaces, cave/headlamp/cutaway, survival,
and the concurrently added shadow/probe/HDR/volumetric pipelines.
Inspected `roadmap-m2-05-verified-audio-controls.png`, default coastal land,
`roadmap-m2-05-42-audio-controls.png`, and seed-42 arch readback; GPU pixels are
neither physical input nor listening evidence.

## Explicit audible hardware check

```sh
cargo run --release --locked -- --audio-check
```

This deliberately audible 16-second run has no window, pointer capture,
preferences, or world save. It sequences material steps, mining/break/place/
pickup, wind/surf/freshwater/cave, entry/swimming/underwater, pause, resume, mute,
and a fresh scene. Ordinary tests/graphics smoke never select it.

`roadmap-m2-05-verified-hardware.log`: **MacBook Pro Speakers, 48,000 Hz,
two channels, 1,503 native callbacks**, no reported stream error, confirmed
native stream **stopped/dropped** before completion. An earlier pass had 1,504
callbacks but only requested shutdown; retained in `roadmap-m2-05-hardware-audio.log`,
not substituted for the final shutdown confirmation. No person has reported
what they heard; callback delivery alone does not complete listening acceptance.

## Retained intermediate failures

- Fresh baseline HDR failure was already corrected by concurrent work; the
  architecture scanner false positive for `glam::camera` was narrowly fixed
  with external/grouped/internal/native-path regressions, not an exemption.
- Initial audio compilation exposed private re-export visibility; fixed locally.
  Clippy found a derivable movement report and preference formatting/array-chunk
  idioms; corrected without suppressions.
- Environment extreme finite coordinates initially overflowed widened offsets;
  bounded world-coordinate admission now prevents that, and the test remains.
  Initial dry-cove falloff was too weak for its deliberate minimum; smooth
  distance falloff was tuned, retaining real default/42 cove/cliff checks.
- Concurrent volumetric rendering edits temporarily left a missing camera
  argument and dead diagnostic fields/imports; their author completed that
  work. No renderer rollback or broad fixes were made by the audio task. Failed
  intermediate logs remain `roadmap-m2-03-{action-tests,feedback-retest,clippy}.log`
  and later transient formatter logs. Final checks and new GPU captures above
  supersede them; old M1 captures were never treated as proof of the new renderer.

## Owner gate / next eligible task

**Listening acceptance: not reported.** Use headphones/speakers during an actual
session: walk different supports, mine/place/collect, approach the coast from
dry land, enter freshwater and ocean, surface, enter a cave, open inventory,
pause/change gains/mute, alt-tab, switch sessions, and quit. Report loudness,
distinctness, sense of place, repetition, transition artifacts, and whether
audio truly stops after exit. Protect/back up any occupied world before choosing
a fresh generation-2 development world; no replacement is automatic.

The owner subsequently authorized **M3-01** progression; its contract explicitly
retains unverified listening/physical acceptance. Please still report tuning
findings; broad roadmap approval is not automatically a completed listening pass.
