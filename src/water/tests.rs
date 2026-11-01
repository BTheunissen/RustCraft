use super::*;

fn sample(biome: WaterBiome, salinity: f32) -> WaterSample {
    WaterSample {
        bottom: -4,
        surface: 18.0,
        flow: [0.0; 3],
        biome,
        salinity,
    }
}

#[test]
fn waves_are_bounded_finite_and_smaller_in_freshwater() {
    let mut maximum_fresh = 0.0_f32;
    let mut maximum_marine = 0.0_f32;
    for x in (-96..=96).step_by(7) {
        for z in (-96..=96).step_by(11) {
            for time in [0.0, 0.75, 9.2, 107.0] {
                let position = [x as f32, z as f32];
                let fresh = wave_displacement(position, time, 0.0, 8.0, 1.0);
                let marine = wave_displacement(position, time, 1.0, 8.0, 1.0);
                assert!(fresh.is_finite() && marine.is_finite());
                assert!(fresh.abs() <= MAX_WAVE_DISPLACEMENT * 0.22 + f32::EPSILON);
                assert!(marine.abs() <= MAX_WAVE_DISPLACEMENT + f32::EPSILON);
                assert!((fresh - marine * 0.22).abs() < 0.000001);
                maximum_fresh = maximum_fresh.max(fresh.abs());
                maximum_marine = maximum_marine.max(marine.abs());
            }
        }
    }
    assert!(maximum_fresh > 0.03 && maximum_fresh < 0.037);
    assert!(maximum_marine > 0.14);
}

#[test]
fn shoreline_weights_and_depth_boundaries_pin_the_surface() {
    for time in [0.0, 4.0, 38.0] {
        for depth in [-2.0, 0.0] {
            assert_eq!(wave_displacement([-16.0, 32.0], time, 1.0, depth, 1.0), 0.0);
        }
        for weight in [-1.0, 0.0] {
            assert_eq!(
                wave_displacement([-16.0, 32.0], time, 1.0, 12.0, weight),
                0.0
            );
            assert_eq!(
                wave_normal([-16.0, 32.0], time, 1.0, 12.0, weight),
                [0.0, 1.0, 0.0]
            );
        }
    }
    let deep = wave_displacement([4.0, 3.0], 1.0, 1.0, 5.0, 1.0);
    let shallow = wave_displacement([4.0, 3.0], 1.0, 1.0, 1.25, 1.0);
    assert!((shallow - deep * 0.5).abs() < 0.000001);
    assert_eq!(deep, wave_displacement([4.0, 3.0], 1.0, 2.0, 100.0, 4.0));
}

#[test]
fn adjacent_negative_chunks_share_world_space_waves_without_uv_resets() {
    for seam in [-32, -16, 0, 16] {
        let from_left = [(seam - 16) as f32 + 16.0, -19.0];
        let from_right = [seam as f32, -19.0];
        for time in [0.0, 0.7, 3.5] {
            assert_eq!(
                wave_displacement(from_left, time, 0.6, 3.0, 0.75),
                wave_displacement(from_right, time, 0.6, 3.0, 0.75)
            );
            assert_eq!(
                wave_normal(from_left, time, 0.6, 3.0, 0.75),
                wave_normal(from_right, time, 0.6, 3.0, 0.75)
            );
        }
    }
    let before = wave_displacement([-16.001, -19.0], 0.7, 1.0, 3.0, 1.0);
    let after = wave_displacement([-15.999, -19.0], 0.7, 1.0, 3.0, 1.0);
    assert!((before - after).abs() < 0.0003);
}

#[test]
fn analytic_normals_match_finite_difference_height_gradients() {
    let epsilon = 0.002;
    for position in [[-17.25, -33.5], [0.0, 0.0], [12.5, 7.25]] {
        let normal = wave_normal(position, 2.7, 0.7, 4.0, 0.8);
        let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 0.000001);
        assert!(normal[1] > 0.99);
        for axis in 0..2 {
            let mut before = position;
            let mut after = position;
            before[axis] -= epsilon;
            after[axis] += epsilon;
            let derivative = (wave_displacement(after, 2.7, 0.7, 4.0, 0.8)
                - wave_displacement(before, 2.7, 0.7, 4.0, 0.8))
                / (2.0 * epsilon);
            let normal_axis = if axis == 0 { 0 } else { 2 };
            assert!((derivative + normal[normal_axis] / normal[1]).abs() < 0.0001);
        }
    }
}

#[test]
fn waves_change_with_time_and_surface_height_keeps_the_mean_profile() {
    let water = sample(WaterBiome::Ocean, 1.0);
    let first = surface_height(water, [-31.5, 4.2], 0.0, 1.0);
    let later = surface_height(water, [-31.5, 4.2], 1.4, 1.0);
    assert!((first - later).abs() > 0.015);
    assert_eq!(surface_height(water, [-31.5, 4.2], 1.4, 0.0), water.surface);
    assert!((later - water.surface).abs() <= MAX_WAVE_DISPLACEMENT);
    assert_eq!(water.surface, 18.0);
    assert_eq!(biome_wave_weight(WaterBiome::Waterfall), 0.0);
    assert_eq!(biome_wave_weight(WaterBiome::CavePool), 0.2);
    for biome in [
        WaterBiome::Lake,
        WaterBiome::River,
        WaterBiome::Estuary,
        WaterBiome::Ocean,
    ] {
        assert_eq!(biome_wave_weight(biome), 1.0);
    }
}

#[test]
fn flow_coordinates_advect_downstream_and_reverse_without_block_tiling() {
    assert_eq!(
        flow_coordinates([-16.0, 3.0], [2.0, 0.0], 0.5),
        [-17.0, 3.0]
    );
    assert_eq!(
        flow_coordinates([-16.0, 3.0], [-2.0, 0.0], 0.5),
        [15.0, -3.0]
    );
    assert_eq!(flow_coordinates([-16.0, 3.0], [0.0, 2.0], 0.5), [2.0, 16.0]);
    let first = flow_coordinates([-16.01, 3.0], [0.6, -0.8], 2.0);
    let second = flow_coordinates([-15.99, 3.0], [0.6, -0.8], 2.0);
    assert!((first[0] - second[0]).abs() < 0.021);
    assert!((first[1] - second[1]).abs() < 0.021);
    let forward = flow_coordinates([3.0, -9.0], [0.6, -0.8], 0.0);
    let reversed = flow_coordinates([3.0, -9.0], [-0.6, 0.8], 0.0);
    assert_eq!(forward, reversed.map(|value| -value));
    let still = flow_coordinates([3.0, -9.0], [0.0; 2], 12.0);
    assert_eq!(still, flow_coordinates([3.0, -9.0], [0.0; 2], 0.0));
    assert!(still.into_iter().all(f32::is_finite));
}

#[test]
fn freshwater_and_marine_optics_blend_continuously_and_darkening_tracks_depth() {
    let fresh = underwater_optics(Some(sample(WaterBiome::Lake, 0.0)), 18.0);
    let marine = underwater_optics(Some(sample(WaterBiome::Ocean, 1.0)), 18.0);
    let estuary = underwater_optics(Some(sample(WaterBiome::Estuary, 0.5)), 18.0);
    assert_eq!(fresh.color, [0.04, 0.16, 0.11]);
    assert!((marine.color[2] - 0.21).abs() < f32::EPSILON);
    assert!(fresh.color[1] > fresh.color[2]);
    assert!(marine.color[2] > marine.color[1]);
    assert_eq!(fresh.density, 0.065);
    assert_eq!(marine.density, 0.04);
    for axis in 0..3 {
        assert!(
            (estuary.color[axis] - (fresh.color[axis] + marine.color[axis]) * 0.5).abs() < 0.000001
        );
    }
    let deep = underwater_optics(Some(sample(WaterBiome::Ocean, 1.0)), -2.0);
    assert_eq!(deep.depth, 20.0);
    assert!(deep.density > marine.density);
    assert!(
        deep.color
            .into_iter()
            .zip(marine.color)
            .all(|(deep, shallow)| deep < shallow)
    );
    assert_eq!(underwater_optics(None, -100.0), fresh);
    assert_eq!(
        underwater_optics(Some(sample(WaterBiome::Lake, 0.0)), 30.0).depth,
        0.0
    );
}

#[test]
fn nearby_beds_retain_contrast_in_both_water_palettes() {
    let fresh = underwater_optics(Some(sample(WaterBiome::Lake, 0.0)), 13.0);
    let marine = underwater_optics(Some(sample(WaterBiome::Ocean, 1.0)), 13.0);
    let fresh_transmission = (-8.0 * fresh.density).exp();
    let marine_transmission = (-8.0 * marine.density).exp();
    assert!(fresh_transmission > 0.55);
    assert!(marine_transmission > 0.70);
    assert!(marine_transmission > fresh_transmission);
    assert!(fresh.color[1] > fresh.color[2]);
    assert!(marine.color[2] > marine.color[1]);
}

#[test]
fn invalid_and_extreme_inputs_remain_finite_without_moving_invalid_surfaces() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(wave_displacement([invalid, 0.0], 0.0, 1.0, 3.0, 1.0), 0.0);
        assert_eq!(wave_displacement([0.0; 2], invalid, 1.0, 3.0, 1.0), 0.0);
        assert_eq!(wave_displacement([0.0; 2], 0.0, 1.0, invalid, 1.0), 0.0);
        assert_eq!(wave_displacement([0.0; 2], 0.0, 1.0, 3.0, invalid), 0.0);
        assert!(wave_displacement([0.0; 2], 0.0, invalid, 3.0, 1.0).is_finite());
        let mut water = sample(WaterBiome::Estuary, invalid);
        water.surface = invalid;
        let optics = underwater_optics(Some(water), invalid);
        assert_eq!(optics, underwater_optics(None, 0.0));
        assert!(surface_height(water, [0.0; 2], 0.0, 0.0).is_finite());
        assert_eq!(flow_coordinates([invalid, 0.0], [0.0; 2], 0.0), [0.0; 2]);
        assert_eq!(flow_coordinates([0.0; 2], [1.0, 0.0], invalid), [0.0; 2]);
        assert!(
            flow_coordinates([1.0, 2.0], [invalid, invalid], 1.0)
                .into_iter()
                .all(f32::is_finite)
        );
    }
    assert_eq!(
        wave_displacement([f32::MAX; 2], f32::MAX, 1.0, 3.0, 1.0),
        0.0
    );
    assert_eq!(
        flow_coordinates([f32::MAX; 2], [8.0; 2], f32::MAX),
        [0.0; 2]
    );
    let mut extreme = sample(WaterBiome::Ocean, 2.0);
    extreme.surface = f32::MAX;
    extreme.bottom = i32::MIN;
    assert!(surface_height(extreme, [0.0; 2], 1.0, 1.0).is_finite());
    assert_eq!(underwater_optics(Some(extreme), -f32::MAX).depth, 0.0);
    extreme.surface = 200.0;
    assert_eq!(underwater_optics(Some(extreme), 0.0).depth, 144.0);
    assert_eq!(underwater_optics(Some(extreme), 0.0).salinity, 1.0);
}
