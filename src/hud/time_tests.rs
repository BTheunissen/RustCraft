use super::{HudInfo, build_hud, readout_rows};
use crate::game::WorldTime;
use crate::player::SpeedMultiplier;

#[test]
fn both_day_and_night_show_the_clock_and_night_exposes_the_headlamp_readout() {
    let mut view = HudInfo {
        captured: true,
        flying: false,
        swimming: false,
        phasing: false,
        headlamp: false,
        underground: false,
        biome: "Plains",
        position: [0.0; 3],
        fps: 60.0,
        loaded_chunks: 9,
        pending_chunks: 0,
        speed: SpeedMultiplier::Normal,
        time: WorldTime::default(),
    };
    assert!(
        readout_rows(&view)
            .iter()
            .any(|(name, value)| *name == "TIME" && value == "10:00 1X (F6)")
    );
    assert!(!readout_rows(&view).iter().any(|(name, _)| *name == "LAMP"));
    view.time.command("/time set night").unwrap();
    view.time.command("/time speed 60").unwrap();
    let rows = readout_rows(&view);
    assert!(
        rows.iter()
            .any(|(name, value)| *name == "TIME" && value == "22:00 60X (F6)")
    );
    assert!(
        rows.iter()
            .any(|(name, value)| *name == "LAMP" && value == "OFF (H)")
    );
    for (width, height) in [(160, 96), (640, 400), (1280, 720)] {
        let vertices = build_hud(width, height, &view);
        assert!(!vertices.is_empty() && vertices.len() < 40_000);
    }
}
