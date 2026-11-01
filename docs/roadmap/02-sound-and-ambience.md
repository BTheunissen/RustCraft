# Milestone 2: a world you can hear

**Outcome:** movement and gathering have clear, restrained audio feedback, and a journey from inland terrain to the sea-stack coast and into caves has distinct, smoothly changing ambience.

The original sequence begins after M1-05 and its owner playtest. The owner authorized progression and the exact CPAL 0.18.2 addition while the physical expedition stays unverified. M2-01–02 are complete; M2-03–05 implementation/automated verification is delivered, **awaiting owner listening acceptance**. Backend, assets, and budgets are frozen in [the audio contract](audio-contract.md). Follow the [roadmap](README.md) and [agent workflow](agent-workflow.md).

## Scope

- Footsteps appropriate to the actual supporting material; restrained mining, successful breaking/placement, pickup, and water-entry/swimming feedback.
- Wind and coastal surf, freshwater ambience near real water, subdued cave ambience, and an underwater listening state.
- Smooth transitions, bounded mixing, master mute/volume, and independent action/ambience gains.
- Correct pause, inventory, focus-loss, scene-switch, and shutdown behavior. Missing audio hardware must not make the game or save system fail.

No musical score, voiced characters, wildlife simulation, new weather/time systems, physically simulated acoustics, or large settings framework. Ambient sound is atmosphere, not evidence that an animal or simulation exists.

## Ownership and contracts

- `game/` produces renderer/native-independent action feedback and environment readouts. It must not open audio devices or depend on a playback library.
- A private native adapter `src/app/audio.rs` / `audio/` owns device initialization, playback resources, and lifecycle. It consumes CPU feedback/readouts, not GPU pixels.
- Environment mixing is testable without hardware. Prefer explicit events/readouts over a generic event bus or new framework.
- Delivered samples are original/generated; source, creator, MIT license, and modifications are recorded in `assets/audio/README.md`. No unlicensed game sounds or copied reference-site media.
- Existing CPU dependency direction and the single Cargo package remain. Adding a small audio dependency may be justified, but propose the exact dependency and obtain owner approval before changing Cargo/lockfile.
- Unit tests and existing graphics smoke launches use a silent/fake output and do not unexpectedly open devices or play sound. A real playback check must be explicitly separate and reported as such.

### Initial limits to confirm, not already measured

- A small preloaded clip set; no per-frame decoding, device creation, or filesystem I/O.
- Initially at most **16 simultaneous voices**, **4 environmental loops**, and **64 pending transient events**, with a documented eviction/coalescing policy.
- Short ambience crossfades, initially around **0.25–0.75 seconds**; no sharp inland/coast/cave volume discontinuities.
- Conservative default loudness with peak/headroom checks. Volume is normalized and finite; mute reaches actual silence.
- Bounded spatial/environment sampling from resident data or explicitly bounded known feature metadata. Never scan the entire generated world to find a sound source.

If these limits are unsuitable for the selected backend, revise them in M2-01 with a concrete reason. Do not leave queues, sources, or cached chunks unbounded.

## Task queue

### M2-01 — freeze the audio contract and choose a backend

- [x] Complete
- **Dependencies:** M1-05 accepted. **Scope:** a short design/backend decision, clip/provenance plan, and test strategy.
- Inspect current action, environment, screen, and frame timing APIs before choosing how feedback reaches playback.
- Propose the minimum macOS-first backend, exact dependency/version if needed, failure behavior, asset format/budget, and owner-approved dependency addition.
- Freeze event semantics, distance-based footstep cadence, looping profiles, bounded voice/event policies, volume controls, and preferences storage separate from world progress.
- Define real-listening checks and a fake sink/mixer route for CPU tests. Do not equate event counts with audible output.
- **Gate:** dependency choice is approved, the asset rights plan is clear, and device-free tests can exercise behavior independently of native playback.

### M2-02 — implement playback and lifecycle safely

- [x] Complete
- **Dependencies:** M2-01. **Scope:** native audio adapter, resources, bounded mixing, device-free tests; Cargo/lockfile only for the approved dependency.
- Initialize resources once, cap voices/queues, and make device initialization/loss a nonfatal, clearly reported condition without repetitive warning spam.
- Muting suppresses output; pause/inventory/focus loss suspend or fade world audio. Returning from a menu or another session must not play stale queued footsteps/mining sounds or leave old ambient loops active.
- Handle scene replacement and application shutdown without orphan playback or blocking a frame indefinitely.
- **Tests:** simulated unavailable device, queue/voice limits, invalid gain/time inputs, mute, scene clearing, pause/resume, and deterministic mixing decisions. Hardware remains a separate check.
- **Gate:** existing gameplay and graphics smoke checks work with audio disabled and with a fake sink; native playback has an explicit manual verification route.

### M2-03 — add action sounds tied to actual gameplay

- [ ] Complete
- **Dependencies:** M2-02. **Scope:** CPU feedback signals, movement/gathering hooks, sound mappings, tests.
- Drive footsteps by actual supported movement distance and supporting material, not render FPS or held movement keys. No footsteps while stationary, flying, blocked against a wall, paused, or swimming.
- Rate-limit mining feedback and emit break/place/pickup feedback only for real successful actions. Do not emit success sounds when placement is rejected, inventory is full, or the target is unmineable.
- Add restrained water transition/swimming feedback without repeatedly firing entry sounds every submerged frame.
- Keep feedback transient and out of saves; ensure normal gameplay results do not depend on an attached audio consumer.
- **Tests:** different timestep partitions, blocked movement, flying/swimming transitions, material mapping, failed interactions, and no duplicate success events.
- **Gate:** actions sound responsive and distinct without becoming an incessant click/footstep loop.

### M2-04 — add environmental ambience and transitions

- [ ] Complete
- **Dependencies:** M2-03. **Scope:** CPU environment mixing, bounded source sampling, native loops, tests.
- Blend inland wind, marine surf, freshwater, cave, and underwater profiles using actual local conditions. Hearing surf from a dry viewpoint must not require the player's camera to be inside a water voxel.
- Respect sheltered/underground exposure and actual water state. A label alone must not trigger surf beside a freshwater pond or underwater filtering after surfacing.
- Use bounded proximity/feature sampling and update cadence; do not procedurally generate large terrain regions in the audio callback or on each rendered frame.
- Fade sources on travel, pause, focus loss, eviction, and scene changes; retain no unbounded per-chunk audio residency.
- **Tests:** coast/inland transitions, lake versus ocean, roof/cave and underwater transitions, stable crossfades under varied dt, finite gains, and source eviction.
- **Gate:** the first coastal expedition has a clear audible sense of place without clipping, abrupt changes, or obviously short repeating loops.

### M2-05 — expose simple controls and verify the listening experience

- [ ] Complete
- **Dependencies:** M2-04. **Scope:** minimal mute/master/action/ambience controls and preference persistence, UI tests, integration/listening evidence, docs.
- Keep controls compact and usable from the pause interface. Closing controls must preserve pointer-capture and held-input safety. Preferences belong outside the saved world and cannot corrupt world progress.
- Run local Rust checks, isolated preferences failure-path tests, and native GPU smoke for UI/executable changes. Regress the coastal scenes and existing capture/pause behavior.
- Verify asset provenance and sample/voice memory limits against the actual delivered clip set.
- **Owner listening pass:** headphones/speakers; walk stone/sand/wood, mine/place/collect, approach the coast from dry land, enter freshwater and ocean water, surface, enter a cave, pause, mute, alt-tab, switch sessions, and quit. Check comfortable loudness, location response, repetition, and stopped audio after exit.
- **Gate:** the owner accepts the audible result. Fake-sink tests or a GPU screenshot alone cannot complete this gate; report unavailable audio hardware/listening access explicitly.

## Next checkpoint

Once accepted, refine the item/crafting/base contract in M3-01 using the first two milestones' playtest findings. Do not add wildlife or weather merely to supply more sound sources.

## Implementation records

### M2-01 — completed; exact dependency approved

- **Owner steering:** “Looks great, keep going” authorizes this contract preparation; physical M1 expedition acceptance is still not reported. No listening acceptance is inferred.
- **Outcome:** [`audio-contract.md`](audio-contract.md) freezes inspected action/environment/frame/screen/session APIs, exact CPAL 0.18.2 addition, original synthesis/provenance, supported-distance cadence, resident-only ambience, bounded mixing/lifecycle, separate preferences, and device-free versus audible tests. The owner selected **“Approve CPAL (Recommended)”** before dependency edits.
- **Source changes:** corrected `tests/architecture.rs` to distinguish external `glam::camera` paths/imports from outward RustCraft references. A regression still rejects `crate::camera`, `super::super::camera`, direct internal access, grouped outward imports, and native graphics imports. No renderer logic or Cargo/lockfile changes.
- **Baseline:** fresh library tests and strict all-target Clippy pass initially; the first whole suite stopped on that architecture false positive, not the earlier HDR float failure (fixed by concurrent work). The focused six-test architecture suite passes after the correction. Whole-project rerun: **625 tests pass**, one timing diagnostic ignored (`roadmap-m2-01-rechecked-tests.log`), with new unfinished renderer diagnostic dead-code warnings. Formatting remains failed in concurrent lighting/playtest/renderer files; preserved rather than broadly reformatted. Rust LSP tools are unavailable in the current catalog; Cargo is the compiler evidence.
- **Evidence directory:** `/private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/`; initial `{fmt,tests,clippy}` logs use `roadmap-m2-01-baseline-*`, and focused guardrail evidence is `roadmap-m2-01-architecture-fix-tests.log`. Initial Clippy pass predates the later dead-code warnings and is not a final certification of the changing renderer.
- **Final checkpoint checks:** locked formatting and strict all-target Clippy pass in `roadmap-m2-01-contract-{fmt,clippy}.log` after concurrent work settled; the preceding 625-test rerun passes. No fresh GPU/audio acceptance is claimed.
- **Unverified/gates:** generated clips, real output, and preferences/UI remain for implementation. M2-01 is complete on exact approval; next code task is M2-02. Physical expedition and listening acceptance remain unverified.

### M2-02 — completed, device-free and silent-native verified

- **Outcome:** approved exact CPAL 0.18.2; original preloaded 29-action/four-loop sample bank; 12 transient + four ambient voice limit; 64-event expiring/coalescing handoff; priority/release replacement; finite gains and limiter; mono/stereo f32/i16/u16 conversion. Device initialization is once on a worker, failure is nonfatal/once-reported, and mute/pause/scene/exit controls bypass a contended mailbox. Stale heartbeat fades unattended output.
- **Ownership:** private `src/app/audio.rs` and cohesive `audio/` modules; CPU `game/feedback` types; `app` native/frame/session lifecycle; `--no-audio` and separate opt-in `--audio-check`. Assets/provenance are source-generated MIT and documented in `assets/audio/README.md`; no files/decoders in callbacks, no microphone, no save/generation change.
- **Checks:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` (**646 pass**, one timing diagnostic ignored), `cargo build --release --locked`. Device-free mixer/callback tests cover limits, malformed inputs, exact mute, silence conversion, stale heartbeat, no old-scene replay, contention, priority, and shutdown. Logs `roadmap-m2-02-{fmt,clippy,complete-tests,release}.log` in the approved temporary directory.
- **Fresh native regression:** `cargo run --release --locked -- --smoke-test --screenshot /private/var/folders/wd/jg2kbqkn7312_294kzqtsz540000gn/T/opencode/roadmap-m2-02-silent.png` passes **348 presented frames**, resize verified, Apple M3 Max/Metal, 289 chunks (`roadmap-m2-02-native.log`). Coast/cove/arch/distances 2/12, isolated edited-save exact pixels, water/caves/survival/UI, and concurrent sunlight/probe/HDR/emission GPU checks pass. Inspected coastal land and headlamp-off crystal receiver-wall readbacks. No audio device is opened by that smoke.
- **Hardware route:** `cargo run --release --locked -- --audio-check` explicitly plays a bounded 16-second catalog/lifecycle sequence without any window/capture/preferences/player save. It is provided, not yet executed or certified as human listening. Actions/ambience hooks and controls follow next; M2-05 owner acceptance remains required before M3.

### M2-03 — implementation/automated verified; listening gate open

- **Outcome:** internal bounded player substep results drive two-block supported footsteps and hysteretic wet entry/strokes; actual edited support selects stone/earth/sand/wood. Mining is globally rate-limited; break/place/collection events follow real accepted changes/transfers, never rejected actions. The producer retains at most 64 events without any consumer and snapshots contain no audio state.
- **Tests:** timestep partitions, walls/idle/air/flight/jumps, edge support/negative seams, actual resident swimming, current-only/blocked-stroke silence, failed/unmineable actions, target-change rate limiting, full inventory, no duplicate success, pause/respawn/snapshot clearing; real successful mining also reaches nonzero fake PCM without changing the saved result.
- **Evidence:** `roadmap-m2-05-final-feedback-tests.log` (20 focused CPU tests pass), whole-suite and fake/native evidence in [`audio-evidence.md`](audio-evidence.md). Checkbox remains open because responsiveness/distinction/repetition need human listening, not merely event assertions.

### M2-04 — implementation/automated verified; listening gate open

- **Outcome:** 289 resident surface-water reads at most 4 Hz; dry cliff/cove surf, freshwater/estuary separation, sky/shelter/cave mixing, camera-water filtering, sample-clock crossfades, and source eviction. No landmark search, procedural audio-callback world generation, or per-chunk cache.
- **Tests:** real default/42 coastal cove/cliff, real lake viewpoint and camera entry/surfacing, edited dry top/eviction, cave roof, malformed/extreme readouts/coordinates, bounded sampling/cadence, varied sample rates/partitions, and underwater recovery. Original beds last 24/31/29/37 seconds and fit the sample budget; quality/repetition still needs an expedition listen.

### M2-05 — controls and verification delivered; awaiting owner

- **Outcome:** pause Audio panel with mute and independent finite gains, common draw/hit rectangles and keyboard focus, safe return-to-pause, and isolated versioned atomic `audio.prefs`. Preference failures cannot alter/block world progress or hide a world-save error. Normal tests/smoke touch neither player files nor audio hardware.
- **Evidence:** native default/42 GPU controls/coast/save/water/cave regressions and separately explicit CPAL hardware callbacks; [`audio-evidence.md`](audio-evidence.md) records exact commands, final counts, memory/loudness samples, retained concurrent-work failures, and unverified listening/physical input.
- **Owner gate:** not reported. The later “Sounds good, you can proceed” authorized M3 progression without inventing listening acceptance; M3 implementation is now delivered. Please still exercise the pass above and provide comfort/repetition/transition tuning feedback. Final separate hardware recheck: 1,501 callbacks with stopped/dropped native stream (`roadmap-final-audio.log`), not a human hearing report.
