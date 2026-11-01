//! Surface, pause, and coastal captures after the native smoke journey.

use super::{Error, FrameState, Instant, Player, Renderer, Terrain, Vec3, load_scene};
use crate::hud::{HudInfo, Screen, UiState, build_hud, build_interface};
use crate::world::{ChunkCoord, WaterBiome};

pub(crate) fn verify_surface(
    renderer: &mut Renderer,
    terrain: Terrain,
    capture: &Option<std::path::PathBuf>,
    frame: &FrameState<'_>,
    hud_info: HudInfo<'_>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    let column = terrain.column(
        frame.player.position.x.floor() as i32,
        frame.player.position.z.floor() as i32,
    );
    if let Some(path) = capture {
        renderer.save_screenshot(path, frame)?;
        let pause_hud = build_hud(
            width,
            height,
            &HudInfo {
                captured: false,
                ..hud_info
            },
        );
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let pause_path = path.with_file_name(format!("{stem}-paused.png"));
        let pause_frame = FrameState {
            hud: &pause_hud,
            ..*frame
        };
        renderer.save_screenshot(&pause_path, &pause_frame)?;
        let mut overview_player = frame.player.clone();
        overview_player.position.y = overview_height(
            frame.player.position.y,
            column.height,
            frame.render_distance,
        );
        overview_player.yaw = 1.1;
        overview_player.pitch = -0.45;
        let overview_hud = build_hud(
            width,
            height,
            &HudInfo {
                position: overview_player.position.to_array(),
                biome: column.biome.name(),
                underground: false,
                swimming: false,
                phasing: false,
                ..hud_info
            },
        );
        let overview_frame = FrameState {
            player: &overview_player,
            hud: &overview_hud,
            underwater: false,
            water: None,
            sky_exposure: 1.0,
            in_lava: false,
            ..*frame
        };
        let overview_path = path.with_file_name(format!("{stem}-overview.png"));
        renderer.save_screenshot(&overview_path, &overview_frame)?;
        if let Some(water_position) = water_capture_position(terrain) {
            let mut water_player = Player::new(water_position);
            water_player.flying = true;
            water_player.pitch = -0.55;
            let center = ChunkCoord::from_world(water_position.x, water_position.z);
            let radius = frame.render_distance.min(6);
            // This bounded diagnostic runs only after the smoke test, never while playing.
            let world = load_scene(renderer, terrain, center, radius, deadline)?;
            let water_hud = build_hud(
                width,
                height,
                &HudInfo {
                    position: water_position.to_array(),
                    biome: WaterBiome::Ocean.name(),
                    loaded_chunks: ((radius * 2 + 1).pow(2)) as usize,
                    underground: false,
                    swimming: false,
                    phasing: false,
                    ..hud_info
                },
            );
            let water_frame = FrameState {
                player: &water_player,
                world: &world,
                hud: &water_hud,
                render_distance: radius,
                underwater: false,
                water: None,
                sky_exposure: 1.0,
                in_lava: false,
                ..*frame
            };
            let water_path = path.with_file_name(format!("{stem}-water.png"));
            renderer.save_screenshot(&water_path, &water_frame)?;
        }
    }

    Ok(())
}

pub(crate) fn overview_height(player_y: f32, surface_height: i32, render_distance: i32) -> f32 {
    if render_distance < 4 {
        surface_height as f32 + 8.0
    } else {
        (player_y + 18.0).max(65.0)
    }
}

pub(crate) fn verify_pause_menu(
    renderer: &mut Renderer,
    frame: &FrameState<'_>,
    hud_info: HudInfo<'_>,
    capture: Option<&std::path::Path>,
    native: bool,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = renderer.size();
    for (screen, label) in [
        (Screen::Paused, "pause-menu"),
        (
            Screen::Audio(crate::hud::AudioControls {
                muted: false,
                master: 55,
                actions: 70,
                ambience: 50,
                unavailable: false,
                preferences_unavailable: false,
            }),
            "audio-controls",
        ),
        (
            Screen::ConfirmReplace(crate::game::GameMode::Survival),
            "confirm-replace",
        ),
    ] {
        let hud = build_interface(
            width,
            height,
            &HudInfo {
                captured: false,
                ..hud_info
            },
            &UiState {
                screen,
                mode: crate::game::GameMode::Creative,
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: if matches!(screen, Screen::ConfirmReplace(_)) {
                    1
                } else {
                    0
                },
                can_continue: true,
                save_notice: Some(crate::hud::SaveNotice::Saved),
            },
        );
        let overlay = FrameState {
            hud: &hud,
            ..*frame
        };
        super::capture_scene(renderer, capture, label, &overlay)?;
        let difference = renderer.compare_full_frames(frame, &overlay)?;
        if difference.changed_fraction < 0.01 {
            return Err(super::io::Error::other(
                "pause/confirmation overlay did not change GPU output",
            )
            .into());
        }
        if native && !renderer.render(&overlay)? {
            return Err(
                super::io::Error::other("pause/confirmation overlay was not presented").into(),
            );
        }
    }
    Ok(())
}

pub(crate) fn water_capture_position(terrain: Terrain) -> Option<Vec3> {
    let site = terrain.water_landmark(WaterBiome::Ocean)?;
    let sample = terrain.water_column(site.water[0], site.water[2])?;
    Some(Vec3::new(
        site.water[0] as f32 + 0.5,
        sample.surface + 14.0,
        site.water[2] as f32 + 16.5,
    ))
}
