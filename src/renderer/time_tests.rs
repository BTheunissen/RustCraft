use super::{CameraUniform, FrameState};
use crate::lighting::CelestialLight;
use crate::player::Player;
use crate::world::{Block, ResidentWorld};
use glam::Vec3;

#[test]
fn solar_time_changes_sky_and_light_but_not_enclosure_fog_camera_or_animation() {
    let world = ResidentWorld::default();
    let player = Player::new(Vec3::new(-16.5, 40.0, -0.5));
    let day = FrameState {
        world: &world,
        player: &player,
        hud: &[],
        survival: None,
        elapsed: 2.2,
        time_of_day: 10.0,
        water_time: Some(3.5),
        render_distance: 8,
        underwater: false,
        water: None,
        sky_exposure: 1.0,
        headlamp: false,
        in_lava: false,
    };
    let daytime =
        CameraUniform::from_frame(&day, 1.5, [0; 3], false, true, |_, _, _| Some(Block::Air));
    for hour in [0.0, 6.0, 12.0, 17.5, 22.0] {
        let frame = FrameState {
            time_of_day: hour,
            ..day
        };
        let uniform =
            CameraUniform::from_frame(&frame, 1.5, [0; 3], false, true, |_, _, _| Some(Block::Air));
        let light = CelestialLight::sample(hour);
        assert_eq!(uniform.sun, light.sun_direction.extend(0.0).to_array());
        assert_eq!(
            uniform.sky_top,
            light.sky_top.extend(light.stars).to_array()
        );
        assert_eq!(
            uniform.sky_horizon,
            light.sky_horizon.extend(light.daylight).to_array()
        );
        assert_eq!(uniform.clouds, light.clouds.extend(0.0).to_array());
        assert_eq!(uniform.environment, daytime.environment);
        assert_eq!(uniform.settings, daytime.settings);
        assert_eq!(uniform.look, daytime.look);
        assert_eq!(uniform.view_projection, daytime.view_projection);
        assert_eq!(uniform.water_optics, daytime.water_optics);
        assert_eq!(uniform.lamp, daytime.lamp);
        if hour == 22.0 {
            assert_eq!(uniform.atmosphere[1], 0.0);
        }
    }
    let invalid = FrameState {
        time_of_day: f64::NAN,
        ..day
    };
    let uniform = CameraUniform::from_frame(&invalid, 1.5, [0; 3], false, true, |_, _, _| {
        Some(Block::Air)
    });
    assert_eq!(uniform.sun, daytime.sun);
    assert_eq!(uniform.sky_top, daytime.sky_top);
}
