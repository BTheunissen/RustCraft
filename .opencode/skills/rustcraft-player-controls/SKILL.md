---
name: rustcraft-player-controls
description: Implement and review renderer-independent RustCraft first-person movement, mouse look, jumping, creative flight, and voxel collision. Use for player-controller subagent work.
---

# RustCraft Player Controls

Load `rustcraft-rust` first. Keep event handling in the application and movement/collision math in a testable library module.

## Contracts

- +Y is up; yaw zero looks along -Z. Positive yaw turns right; positive pitch looks up. Mouse Y is inverted when applying look deltas.
- Player position is the center of the feet. Keep body dimensions, eye height, speed, gravity, and jump impulse explicit.
- Normalize horizontal input so diagonal movement is not faster. Ground movement follows yaw, not camera pitch.
- Bound pitch, reject non-finite input, and bound/substep elapsed time to prevent collision tunneling after a stall.
- Test collision through a block-solid callback. Separate jump edge intent from held flight ascent.
- Creative flight may ignore collision, but turning flight off must not strand the player inside a solid block.
- Focus loss and pointer release must clear held keys and pending jumps at the application boundary.

## Validation

- Test cardinal/diagonal movement, yaw-relative strafing, pitch limits, jump/landing, walls/ceilings, sliding, and negative-coordinate collision.
- Test flight ascent/descent, flight-to-walk safety, and large frame times. Assertions should cover outcomes rather than internal implementation details.
- Add reusable guidance here when testing reveals a concrete pitfall; keep it concise.

## Controller integration

- `src/player/` uses `glam::Vec3` and read-only solidity/fluid callbacks; `Game` owns CPU residency, never the renderer. `Ctrl` emits `CycleSpeed` once per physical press; persistent `SpeedMultiplier` scales strokes/flight/walking, not dt, gravity, buoyancy, or currents.
- `MovementInput.jump` accepts held Space or a one-update pulse. Walking tracks false-to-true internally and does not buffer airborne presses; flight uses the held value for ascent. The controller must observe a released update to rearm a held jump.
- Pause/focus loss stops simulation, so clearing application key state alone cannot rearm the controller's previous-held jump. Call `Player::clear_input` at that boundary as well; it preserves position and momentum.
- A held jump can be released and pressed again between frames. Preserve the release edge (`InputState::take_jump_release`) and rearm the controller before the next update rather than passing only the final held state.
- H emits `Action::ToggleHeadlamp` only on a physical, nonrepeated press; capture gating belongs in the application. Unrelated actions must preserve pending Space press/release edges and the double-Space timestamp.
- Elapsed time is capped at 0.25 seconds and walking is substepped at 1/120 second. Excess stall time is deliberately dropped; each collision axis also sweeps its full voxel path.
- Collision bounds use `floor(min + epsilon)..=floor(max - epsilon)`. Widen to `f64` before applying epsilon: `f32` rounds it away at ordinary coordinates such as 512. Floor before integer conversion for negative coordinates; exclude merely touching faces and probe slightly below feet for reliable grounding.
- Flight exit checks the current body, then upward voxel boundaries within 64 blocks. If clearance is unavailable, it stays flying; read `Player.flying` after toggling. Respawn resets walking/momentum/jump state but preserves yaw and pitch.
- Compute immersion from actual water intervals intersecting the body, including fractional mean tops. Integrate current and drag with bounded substeps and swept collision; never infer full immersion from a distant surface above an air gap.
- Held Space/Shift controls swim ascent/descent. A low-bank exit impulse must be collision-checked and latched until release; creative flight must query neither water nor solidity and must clear fluid momentum.
- A collision-checked exit may land on a submerged step before reaching land. Rearm the exit only while supported and still substantially wet; keep the held-jump latch on dry land. Cover an actual generated shore, not only a flat pool wall.
