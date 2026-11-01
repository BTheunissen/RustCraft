//! Public CPU interface state and screen dispatch, independent of native input.

use crate::game::{GameMode, Inventory, StorageView};

use super::interface_layout::UiLayout;
use super::{HudInfo, HudVertex, Painter, build_hud, inventory, menus};

/// The visible interface. The application owns transitions and world pausing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    Loading,
    Playing,
    Inventory,
    Crafting,
    Storage,
    Paused,
    /// Paused, keyboard-only solar debug controls; no gameplay click-through.
    TimeConsole,
    /// Read-only audio control values; adjustments remain application-owned.
    Audio(super::AudioControls),
    /// A new mode is selected but the existing local slot is not replaced yet.
    ConfirmReplace(GameMode),
}

impl Screen {
    /// These screens borrow the menu's preview world, never an active session.
    pub const fn is_menu_preview(self) -> bool {
        matches!(self, Self::MainMenu | Self::ConfirmReplace(_))
    }
}

/// Short persistence messages; detailed filesystem errors are reported to stderr.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SaveNotice {
    Saved,
    SaveFailed,
    LoadFailed,
    /// Development format/generation changed; the occupied slot still needs confirmation.
    Incompatible,
    Unavailable,
}

/// Read-only state needed to paint a screen, without changing gameplay.
pub struct UiState<'a> {
    pub screen: Screen,
    pub mode: GameMode,
    /// Slots 0..9 are the hotbar; slots 9..36 are storage.
    pub inventory: Option<&'a Inventory>,
    pub storage: Option<StorageView<'a>>,
    pub interaction_notice: Option<&'a str>,
    /// Physical pixels from the viewport's upper-left corner, like `hit_test`.
    pub cursor: Option<[f32; 2]>,
    /// Keyboard focus. An in-bounds hovered button takes visual precedence.
    pub focused_button: usize,
    /// Continue remains visibly disabled until a local slot exists.
    pub can_continue: bool,
    pub save_notice: Option<SaveNotice>,
}

/// An actionable region; clicking gaps or backgrounds never returns a hit.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UiHit {
    Button(usize),
    InventorySlot(usize),
    Crafting,
    Recipe(usize),
    CraftBack,
    CrateSlot(usize),
}

/// Main menu and pause have four buttons; audio has eight; replacement has two.
/// Loading, gameplay, and inventory have no menu buttons.
pub const fn button_count(screen: Screen) -> usize {
    match screen {
        Screen::MainMenu => 4,
        Screen::Paused => 4,
        Screen::Audio(_) => 8,
        Screen::ConfirmReplace(_) => 2,
        Screen::Crafting => crate::game::RECIPES.len() + 1,
        _ => 0,
    }
}

/// Finds the visible button or inventory slot at a physical-pixel cursor.
///
/// Rectangles use half-open edges and exactly the same resize-dependent layout
/// as rendering. Invalid/outside cursors and viewports below 160x96 return none.
pub fn hit_test(width: u32, height: u32, screen: Screen, cursor: [f32; 2]) -> Option<UiHit> {
    UiLayout::new(width, height)?.hit(screen, cursor)
}

/// Builds clipped, finite NDC triangles for menus and gameplay.
///
/// Dimensions and cursor coordinates are physical pixels, not logical points.
/// The layout is readable at 640x400 and scales its pixel art at larger sizes.
/// The terrain remains the renderer's background; menus only add translucent
/// shading. Playing creative retains the existing captured `build_hud` output,
/// even during a transient capture loss. Only `build_hud` keeps the legacy
/// click-to-explore overlay; explicit screens never show that card.
pub fn build_interface(
    width: u32,
    height: u32,
    info: &HudInfo<'_>,
    ui: &UiState<'_>,
) -> Vec<HudVertex> {
    if ui.screen == Screen::Playing && matches!(ui.mode, GameMode::Creative) {
        return build_hud(
            width,
            height,
            &HudInfo {
                captured: true,
                ..*info
            },
        );
    }
    let Some(layout) = UiLayout::new(width, height) else {
        return Vec::new();
    };
    let mut painter = Painter {
        width: layout.width,
        height: layout.height,
        vertices: Vec::with_capacity(24_000),
    };
    match ui.screen {
        Screen::TimeConsole => {}
        Screen::Playing => inventory::survival(&mut painter, &layout, info, ui),
        Screen::Inventory => inventory::inventory(&mut painter, &layout, ui),
        Screen::Crafting => super::crafting::draw(&mut painter, &layout, ui),
        Screen::Storage => super::storage::draw(&mut painter, &layout, ui),
        Screen::Loading => menus::loading(&mut painter, &layout, info, ui.mode),
        Screen::Audio(controls) => super::audio::draw(&mut painter, &layout, ui, controls),
        Screen::MainMenu | Screen::Paused | Screen::ConfirmReplace(_) => {
            menus::menu(&mut painter, &layout, ui);
        }
    }
    painter.vertices
}
