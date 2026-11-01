---
name: rustcraft-native-playtest
description: Review and verify RustCraft native winit/wgpu integration, pointer capture, GPU frame readback, and playable exploration. Use for rendering and end-to-end QA subagent work.
---

# RustCraft Native Playtest

Load `rustcraft-rust`. Check the versions in Cargo.lock and the actual crate source before relying on remembered wgpu APIs.

## Lifecycle and input

- Create native windows in `ApplicationHandler::resumed`; initialize the surface only once a valid window/display exists.
- Preserve window ownership for the surface lifetime without unsafe lifetime extension.
- Skip zero-sized/occluded surfaces. Handle timeout/outdated/lost acquisition explicitly, and recreate depth attachments on resize.
- Apply `Some(size)` returned by `request_inner_size` immediately; winit does not guarantee a later `Resized` event (notably on Wayland).
- On macOS, `request_redraw` immediately wakes winit; `WaitUntil` does not delay it. Gate retry requests against a persistent deadline, and truly wait while zero-sized/occluded.
- Pace against the current system refresh rate, not the maximum advertised video mode. Keep fractional mHz rates, refresh on monitor/fullscreen changes with a cached periodic fallback, and use the same interval for failed-acquisition backoff. VSync does not remove a separate CPU-side 60 FPS cap.
- Capture the mouse only after a user click. On macOS, center the pointer before locking: cursor warping after locking re-associates it. Try locked mode before confined mode.
- Escape, focus loss, occlusion, and `ApplicationHandler::exiting` must release capture, show the pointer, and clear both key state and the controller jump latch. macOS native Quit need not run `App::drop`.
- Prefer physical movement keys and raw device deltas. Do not scale mouse look by frame time.
- Test keyboard state together with the player's held-jump latch: a Space release/repress entirely between updates must still rearm walking jump.

## Real GPU verification

- Run fmt, Clippy, unit/doc tests, and a release build before claiming readiness.
- Run `cargo run --locked -- --smoke-test --screenshot <approved-temp-path>.png` to exercise the real window, terrain worker, camera, resizing, shader validation, and presentation.
- A locked/asleep macOS display can yield zero presented frames. Check display/session state read-only; use explicit `--offscreen-smoke-test` for shaders/readback and report it separately, never as a native presentation pass.
- Inspect the resulting frame, not merely the exit code: verify outward faces, terrain/trees/water, readable HUD, correct sky/depth, and absence of empty geometry or texture bleeding.
- Underground validation renders actual cave rooms, compares headlamp-on/off scene luminance outside HUD margins, and keeps emissive minerals visible. A biome label alone is not evidence of its materials.
- Also inspect a cave entrance from outdoors: roofed geology should fade dark rather than inherit bright sky fog, while canopy-shadowed trees should keep outdoor atmosphere.
- Volumetric acceptance needs air-only HDR readback: close/reopen a voxel aperture at the same roofed pose, compare half-resolution upscaling with full-resolution rays at depth edges, and keep foreground beams when the distant receiver has already faded to cave fog.
- Image stability needs seeded movement across light-volume anchors with the interactive probe budget and frozen animation. Preserve probe storage, sweep cursor, and exposure around full-update references; otherwise diagnostics can silently hide a cache-clear flash. Check reverse motion and first-frame edit invalidation too.
- Direction-only sky/air reconstruction should exclude camera translation; fixed sun matrices must translate an unchanged light rotation. Texture minification needs tile-isolated linear-light mipmaps, not whole-atlas averaging or a temporal blur that conceals unstable light.
- Phasing needs real buried voxel data, not only double-sided exposed faces. Test the same embedded camera with flight off/on, fully solid terrain with the lamp off, and restored ordinary rendering after exit.
- Measure lighting/texture acceptance without any HUD, including the central crosshair. Accumulate full-resolution readback statistics in `f64`; uniform gray frames must report near-zero contrast.
- Isolate the water-animation clock when comparing flow/wave readbacks: leave sky/cloud/lava time fixed and remove the HUD. Also compare freshwater/marine camera optics at the same submerged pose so terrain differences cannot pass the palette check.
- Use GPU texture readback rather than desktop screenshots. Align copy rows to 256 bytes, remove row padding, and swizzle BGRA to RGBA before encoding PNG.
- A simulated smoke run does not prove real physical mouse/keyboard input. Verify those when desktop automation is available; otherwise state the exact unverified interaction.
- Assert smoke phases separately; native smoke requires a presented completion frame and one at a genuinely changed size. Early close/quit is a failed test; an offscreen screenshot or accumulated startup frames do not prove final presentation.
- Keep bounded smoke-test paths and concise logs; watchdog deadlines must still wake the loop when redraws stop at zero size. Do not leave unattended mouse-capturing windows running after verification.
- Synchronous diagnostic generation and readback must share the smoke deadline; an event-loop watchdog cannot run while a redraw callback is blocked.
- Install GPU error/device-loss callbacks immediately after device creation. Propagate fatal errors through both native and offscreen render results; submission alone does not prove GPU completion.

## Review focus

- Check frustum/depth conventions (right-handed camera, WebGPU zero-to-one depth), CCW winding, atlas material IDs, and uniform alignment together.
- Check every fog mode against residency at the minimum render distance; fixed cave fog must not extend past the nearest unloaded chunk boundary.
- Check the downward sky from elevated surface flight as well as cave entrances: the unloaded background should meet the active fog color rather than form a black horizon ring.
- A global downward-sky fix can backlight unloaded cave mouths. Use known near-ground clearance to distinguish a low downward cavity from high outdoor flight, and ignore tree crowns in that clearance.
- Keep terrain generation off the render thread, queues/residency bounded, stale results discarded, and worker shutdown free of channel deadlocks.
- Record concrete reusable discoveries here, not successful command logs or speculative rules.
- A supported diagnostic viewpoint can still face a nearby ledge. Check actual line of sight and inspect the readback before counting a scenic view; camera relocation never proves a walking route.
