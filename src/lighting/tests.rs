use super::{
    ExposureCache, HEADLAMP_RANGE, atmosphere_exposure, camera_ground_clearance, cave_fog_range,
    cavity_background, headlamp_strength, lower_atmosphere, sky_exposure,
};
use crate::world::Block;
use glam::Vec3;

#[test]
fn exposed_negative_y_is_bright_but_roofed_positive_y_is_dark() {
    assert_eq!(
        sky_exposure(Vec3::new(-0.1, -40.0, -0.1), |_, _, _| Block::Air),
        1.0
    );
    assert_eq!(
        sky_exposure(Vec3::new(2.0, 15.0, 2.0), |_, y, _| {
            if y == 18 { Block::Stone } else { Block::Air }
        }),
        0.0
    );
}

#[test]
fn water_and_leaves_attenuate_without_using_wrong_negative_columns() {
    let exposure = sky_exposure(Vec3::new(-0.01, -1.0, -16.1), |x, y, z| {
        assert_eq!((x, z), (-1, -17));
        match y {
            1 => Block::Water,
            2 => Block::Leaves,
            _ => Block::Air,
        }
    });
    assert!((exposure - 0.9 * 0.65).abs() < 0.0001);
}

#[test]
fn headlamp_follows_camera_and_fades_with_distance() {
    let eye = Vec3::new(-8.0, -30.0, 2.0);
    let direction = Vec3::NEG_Z;
    let near = headlamp_strength(eye, direction, eye + direction * 3.0, true);
    let far = headlamp_strength(eye, direction, eye + direction * 20.0, true);
    assert!(near > far && far > 0.0);
    assert_eq!(
        headlamp_strength(eye, direction, eye - direction * 3.0, true),
        0.0
    );
    assert_eq!(
        headlamp_strength(eye, direction, eye + direction * HEADLAMP_RANGE, true),
        0.0
    );
    assert_eq!(
        headlamp_strength(eye, direction, eye + direction, false),
        0.0
    );
    assert_eq!(
        headlamp_strength(eye, direction, eye + Vec3::X * 3.0, true),
        0.0
    );
}

#[test]
fn invalid_or_zero_direction_cannot_create_nonfinite_light() {
    assert_eq!(
        headlamp_strength(Vec3::ZERO, Vec3::ZERO, Vec3::Z, true),
        0.0
    );
    assert_eq!(
        sky_exposure(Vec3::NAN, |_, _, _| panic!(
            "invalid camera must not query the world"
        )),
        0.0
    );
    assert_eq!(headlamp_strength(Vec3::NAN, Vec3::Z, Vec3::Z, true), 0.0);
    assert_eq!(
        headlamp_strength(Vec3::ZERO, Vec3::Z, Vec3::ZERO, true),
        0.0
    );
}

#[test]
fn static_camera_skips_repeated_world_sampling_but_new_voxels_recheck() {
    let mut cache = ExposureCache::default();
    let original = Vec3::new(-0.1, -12.0, 2.1);
    assert_eq!(cache.sample(original, |_, _, _| Block::Air), 1.0);
    assert_eq!(
        cache.sample(original + Vec3::new(-0.2, 0.1, 0.2), |_, _, _| panic!(
            "cached column must not be sampled"
        )),
        1.0
    );
    assert_eq!(
        cache.sample(original + Vec3::X * 2.0, |_, _, _| Block::Stone),
        0.0
    );
}

#[test]
fn editing_a_roof_refreshes_exposure_without_moving_the_camera() {
    let mut cache = ExposureCache::default();
    let eye = Vec3::new(0.5, 10.5, -0.5);
    assert_eq!(cache.sample(eye, |_, _, _| Block::Air), 1.0);
    cache.clear();
    assert_eq!(
        cache.sample(eye, |_, y, _| if y == 14 {
            Block::Stone
        } else {
            Block::Air
        }),
        0.0
    );
    cache.clear();
    assert_eq!(cache.sample(eye, |_, _, _| Block::Air), 1.0);
}

#[test]
fn projected_terrain_daylight_matches_direct_world_rays() {
    let terrain = crate::world::Terrain::default();
    let cave = Vec3::from_array(
        terrain
            .cave_position(crate::world::CaveBiome::Rocky)
            .unwrap(),
    ) + Vec3::Y * 1.62;
    let surface = Vec3::from_array(terrain.spawn_position()) + Vec3::Y * 1.62;
    let mut cache = ExposureCache::default();
    for eye in [cave, surface, cave + Vec3::X] {
        let direct = sky_exposure(eye, |x, y, z| terrain.block_at(x, y, z));
        assert!((cache.sample_terrain(eye, terrain) - direct).abs() < 0.0001);
    }
}

#[test]
fn low_render_distance_cave_fog_hides_the_nearest_unloaded_boundary() {
    for radius in [2, 3, 8, 12] {
        let (start, end) = cave_fog_range(radius);
        assert!(start > 0.0 && start < end);
        assert!(end < radius as f32 * 16.0);
    }
    assert_eq!(cave_fog_range(2), (11.0, 20.0));
    assert_eq!(cave_fog_range(8), (14.0, 52.0));
    assert_eq!(cave_fog_range(-100), cave_fog_range(2));
}

#[test]
fn fully_blocking_and_attenuating_materials_use_one_daylight_rule() {
    use super::daylight_through;
    for block in [
        Block::Stone,
        Block::Bedrock,
        Block::DeepStone,
        Block::CoalOre,
        Block::IronOre,
        Block::GoldOre,
        Block::Crystal,
        Block::Moss,
        Block::Lava,
        Block::Limestone,
        Block::Slate,
        Block::Basalt,
        Block::Calcite,
        Block::Gravel,
        Block::Clay,
    ] {
        assert_eq!(daylight_through(1.0, block), 0.0);
    }
    assert_eq!(daylight_through(0.5, Block::Air), 0.5);
    assert_eq!(daylight_through(0.0, Block::Water), 0.0);
    assert_eq!(daylight_through(0.5, Block::Leaves), 0.325);
}

#[test]
fn leaf_canopies_do_not_replace_the_outdoor_sky_with_cave_fog() {
    assert_eq!(atmosphere_exposure(false, 0.09), 1.0);
    assert_eq!(atmosphere_exposure(false, 1.0), 1.0);
    assert_eq!(atmosphere_exposure(true, 0.0), 0.0);
    assert_eq!(atmosphere_exposure(true, 1.0), 1.0);
}

#[test]
fn downward_sky_matches_surface_fog_but_remains_dark_in_closed_caves() {
    let horizon = Vec3::new(0.61, 0.77, 0.90);
    assert!((lower_atmosphere(1.0, horizon) - horizon).length() < 0.00001);
    assert_eq!(
        lower_atmosphere(0.0, horizon),
        Vec3::new(0.005, 0.007, 0.011)
    );
    let twilight = lower_atmosphere(0.5, horizon);
    assert!(twilight.x > 0.005 && twilight.x < horizon.x);
}

#[test]
fn cavity_background_distinguishes_low_entrances_from_high_outdoor_flight() {
    let eye = Vec3::new(-0.1, 26.12, -0.1);
    let clearance = camera_ground_clearance(eye, |x, y, z| {
        assert_eq!((x, z), (-1, -1));
        Some(match y {
            23 => Block::Grass,
            27 => Block::Wood,
            30 => Block::Leaves,
            _ => Block::Air,
        })
    });
    assert!((clearance - 2.12).abs() < 0.00001);
    assert_eq!(cavity_background(clearance, -0.45), 1.0);
    assert_eq!(cavity_background(35.0, -0.45), 0.0);
    assert_eq!(cavity_background(8.0, -0.45), 0.0);
    assert_eq!(cavity_background(clearance, 0.2), 0.0);
    assert_eq!(camera_ground_clearance(eye, |_, _, _| None), 1000.0);
    assert_eq!(
        camera_ground_clearance(eye, |_, _, _| Some(Block::Air)),
        1000.0
    );
    assert_eq!(
        camera_ground_clearance(Vec3::NAN, |_, _, _| panic!("invalid camera")),
        1000.0
    );
}
