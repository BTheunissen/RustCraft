//! CPU-built pixel-font menus, inventory, and exploration overlays.

use crate::player::SpeedMultiplier;

/// A colored HUD triangle vertex, with no texture coordinates or font atlas.
///
/// Positions are normalized device coordinates: `[-1, 1]`, with positive Y up.
/// Colors are normalized, straight-alpha RGBA; the renderer should enable alpha
/// blending and draw the HUD after the world, without depth testing.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    /// Two-dimensional position in normalized device coordinates.
    pub position: [f32; 2],
    /// Normalized, unpremultiplied RGBA color.
    pub color: [f32; 4],
}

/// Renderer-independent information displayed by the HUD.
#[derive(Copy, Clone, Debug)]
pub struct HudInfo<'a> {
    /// Whether the mouse is captured and the player is exploring.
    pub captured: bool,
    /// Whether creative flight is enabled.
    pub flying: bool,
    /// Walking body is sufficiently submerged for swimming and current response.
    pub swimming: bool,
    /// Creative flight currently intersects solid terrain, activating the cutaway.
    pub phasing: bool,
    /// Whether the player's headlamp is enabled.
    pub headlamp: bool,
    /// Whether the player is underground, enabling the lamp readout.
    pub underground: bool,
    /// Current surface or cave biome name; ASCII letters are displayed uppercase.
    pub biome: &'a str,
    /// Player world position, in block units, ordered X, Y, Z.
    pub position: [f32; 3],
    /// Recent frames per second. Invalid values are displayed as `--`.
    pub fps: f32,
    /// Number of chunks currently loaded.
    pub loaded_chunks: usize,
    /// Number of chunks waiting to load or rebuild.
    pub pending_chunks: usize,
    /// Persistent exploration speed selected with Ctrl.
    pub speed: SpeedMultiplier,
    /// World-local solar clock and debug rate, independent of movement speed.
    pub time: crate::game::WorldTime,
}

type Color = [f32; 4];

const TEXT: Color = [0.96, 0.945, 0.89, 1.0];
const MUTED: Color = [0.64, 0.73, 0.72, 1.0];
const GREEN: Color = [0.56, 0.76, 0.60, 1.0];
const PANEL: Color = [0.027, 0.047, 0.067, 0.76];
const BORDER: Color = [0.34, 0.49, 0.42, 0.48];
const CROSSHAIR: Color = [0.97, 0.96, 0.90, 0.94];
const MAX_TEXT_CHARS: usize = 64;

const WALK_CONTROLS: &str = "WASD MOVE  SPACE JUMP  F FLY  H LAMP  CTRL SPEED  ESC PAUSE";
const FLY_CONTROLS: &str = "WASD  SPACE UP/SHIFT DOWN  F FLY  H LAMP  CTRL SPEED  ESC PAUSE";
const SWIM_CONTROLS: &str = "WASD SWIM  SPACE UP/SHIFT DIVE  F FLY  H LAMP  CTRL SPEED  ESC PAUSE";
const HEADLAMP_HINT: &str = "H HEADLAMP - DARK CAVES WHEN OFF";

/// Builds a clipped triangle list for a readable, lightweight overlay.
///
/// Viewport dimensions are physical pixels. The pixel font scales with both
/// dimensions (2x at 1280x720, 3x at 1920x1080), with compact layouts for small
/// windows. Viewports below 160x96 produce no geometry. A captured mouse shows a
/// crosshair; an uncaptured mouse shows the start/pause card instead. Input
/// handling remains the application's responsibility, including paused-only Q
/// and captured-only headlamp toggling.
///
/// The built-in 5x7 font covers ASCII letters, digits, and HUD punctuation;
/// unsupported characters become question marks. Labels and numeric readouts
/// are bounded, so even extreme input values cannot produce unbounded geometry.
pub fn build_hud(width: u32, height: u32, info: &HudInfo<'_>) -> Vec<HudVertex> {
    if width < 160 || height < 96 {
        return Vec::new();
    }

    let mut painter = Painter {
        width: width as f32,
        height: height as f32,
        vertices: Vec::with_capacity(20_000),
    };

    if width < 480 || height < 280 {
        compact_hud(&mut painter, info);
        return painter.vertices;
    }

    let scale = (painter.width / 480.0)
        .min(painter.height / 360.0)
        .floor()
        .clamp(1.0, 8.0);
    let margin = 8.0 * scale;
    let badge_width = brand_badge(&mut painter, margin, scale);
    readouts(&mut painter, info, margin, scale, badge_width);
    controls_bar(&mut painter, info.flying, info.swimming, margin, scale);

    if info.captured {
        crosshair(&mut painter, scale);
    } else {
        pause_card(&mut painter, info.flying, info.swimming, scale);
    }

    painter.vertices
}

mod audio;
mod font;
pub use audio::AudioControls;
mod crafting;
mod icons;
mod interface;
mod interface_layout;
mod inventory;
mod layout;
mod menus;
mod painter;
mod readouts;
mod storage;
mod time_console;
pub use time_console::build_time_console;

pub use interface::{SaveNotice, Screen, UiHit, UiState, build_interface, button_count, hit_test};

use font::glyph;
use layout::{brand_badge, compact_hud, controls_bar, crosshair, pause_card};
use painter::{Painter, fit_text, text_width, truncate_text};
use readouts::readouts;
#[cfg(test)]
use readouts::{biome_label, coordinate_label, count_label, fps_label, readout_rows};

#[cfg(test)]
mod interface_tests;
#[cfg(test)]
mod inventory_tests;
#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod time_tests;
#[cfg(test)]
mod tool_tests;
