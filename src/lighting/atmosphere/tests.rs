use super::*;

fn marched_scattering(
    distance: f32,
    alignment: f32,
    mut visibility: impl FnMut(f32) -> f32,
) -> f32 {
    let distance = distance.clamp(0.0, MAX_DISTANCE);
    let mut total = 0.0;
    for index in 0..MARCH_STEPS {
        let Some([start, end]) = march_interval(index, distance, MAX_DISTANCE) else {
            break;
        };
        total += (transmittance(start) - transmittance(end)) * visibility((start + end) * 0.5);
    }
    total * SUN_SCATTER_SHARE * phase_weight(alignment)
}

#[test]
fn resolution_covers_odd_edges_and_never_allocates_empty_targets() {
    assert_eq!(half_resolution(2400, 1520), [1200, 760]);
    assert_eq!(half_resolution(1201, 761), [601, 381]);
    assert_eq!(half_resolution(1, 1), [1, 1]);
    assert_eq!(half_resolution(0, 0), [1, 1]);
    assert_eq!(half_resolution(u32::MAX, 3), [u32::MAX.div_ceil(2), 2]);
}

#[test]
fn air_never_overrides_water_lava_or_buried_creative_cutaways() {
    for enabled in [false, true] {
        for underwater in [false, true] {
            for lava in [false, true] {
                for phasing in [false, true] {
                    let settings = parameters(enabled, underwater, lava, phasing);
                    assert_eq!(
                        settings[..3],
                        [AIR_EXTINCTION, SUN_SCATTER_SHARE, PHASE_ANISOTROPY]
                    );
                    assert_eq!(
                        settings[3] > 0.5,
                        enabled && !underwater && !lava && !phasing
                    );
                }
            }
        }
    }
}

#[test]
fn clear_air_has_little_near_extinction_and_bounded_distant_haze() {
    assert_eq!(transmittance(-2.0), 1.0);
    assert_eq!(transmittance(0.0), 1.0);
    assert!(transmittance(20.0) > 0.97);
    assert!(transmittance(MAX_DISTANCE) > 0.83);
    assert_eq!(transmittance(1000.0), transmittance(MAX_DISTANCE));
    let mut previous = 1.0;
    for distance in 0..=256 {
        let transmission = transmittance(distance as f32);
        assert!(transmission <= previous);
        previous = transmission;
    }
}

#[test]
fn forward_phase_is_finite_restrained_and_continuous() {
    assert!(phase_weight(1.0) > phase_weight(0.0));
    assert!(phase_weight(0.0) > phase_weight(-1.0));
    assert_eq!(phase_weight(-100.0), phase_weight(-1.0));
    assert_eq!(phase_weight(100.0), phase_weight(1.0));
    for index in 0..=200 {
        let phase = phase_weight(index as f32 * 0.01 - 1.0);
        assert!((0.25..=2.5).contains(&phase));
    }
}

#[test]
fn nonuniform_march_conserves_the_same_fog_transmittance() {
    for distance in [0.0, 0.05, 3.0, 20.0, 52.0, 128.0, 400.0] {
        for alignment in [-1.0, 0.0, 0.65, 1.0] {
            let reference = unoccluded_scattering(distance, alignment);
            assert!(
                (marched_scattering(distance, alignment, |_| 1.0) - reference).abs() < 0.000001
            );
            assert_eq!(marched_scattering(distance, alignment, |_| 0.0), 0.0);
            assert!(
                (marched_scattering(distance, alignment, |_| 0.5) - reference * 0.5).abs()
                    < 0.000001
            );
        }
    }
}

#[test]
fn occluded_intervals_cannot_scatter_and_ray_ends_never_sample_the_wall() {
    let distance = 30.0;
    let mut samples = Vec::new();
    let partial = marched_scattering(distance, 0.7, |sample| {
        samples.push(sample);
        if (8.0..16.0).contains(&sample) {
            1.0
        } else {
            0.0
        }
    });
    assert!(
        samples
            .iter()
            .all(|sample| *sample > 0.0 && *sample < distance)
    );
    assert!(samples[1] - samples[0] < samples[samples.len() - 2] - samples[samples.len() - 3]);
    assert!(partial > 0.0 && partial < unoccluded_scattering(distance, 0.7));
    let front = marched_scattering(4.0, 0.7, |sample| if sample > 8.0 { 1.0 } else { 0.0 });
    assert_eq!(front, 0.0);
}

#[test]
fn depth_motion_changes_only_the_last_march_interval_not_foreground_beams() {
    for distance in [0.0, 0.05, 3.0, 20.0, 52.0, 127.8] {
        let mut completed = 0;
        for index in 0..MARCH_STEPS {
            let Some(first) = march_interval(index, distance, MAX_DISTANCE) else {
                break;
            };
            let second = march_interval(index, distance + 0.01, MAX_DISTANCE).unwrap();
            if first[1] < distance {
                assert_eq!(first, second);
                completed += 1;
            }
        }
        if distance > 3.0 {
            assert!(completed > 5);
        }
    }
    assert_eq!(
        march_interval(MARCH_STEPS, MAX_DISTANCE, MAX_DISTANCE),
        None
    );
    assert_eq!(march_interval(0, 100.0, 0.0), None);
}

#[test]
fn fixed_intervals_cover_small_residency_and_clipped_tail_without_gaps() {
    for limit in [20.0, 52.0, 116.0, MAX_DISTANCE] {
        for distance in [0.03_f32, 4.0, 10.0, 1000.0] {
            let mut previous = 0.0;
            for index in 0..MARCH_STEPS {
                let Some([start, end]) = march_interval(index, distance, limit) else {
                    break;
                };
                assert_eq!(start, previous);
                assert!(end > start && end <= distance.min(limit));
                previous = end;
            }
            assert_eq!(previous, distance.min(limit));
        }
    }
}

#[test]
fn ambient_and_sun_share_one_extinction_instead_of_two_fog_layers() {
    let surface = 0.4;
    let ambient = 0.7;
    let sun = 1.4;
    for distance in [0.0, 5.0, 20.0, 128.0] {
        let t = transmittance(distance);
        let base = surface * t + ambient * (1.0 - t) * (1.0 - SUN_SCATTER_SHARE);
        let reference = base + ambient * (1.0 - t) * SUN_SCATTER_SHARE;
        assert!((reference - (surface * t + ambient * (1.0 - t))).abs() < 0.000001);
        let lit = base + sun * unoccluded_scattering(distance, 0.0);
        let blocked = base;
        assert!(lit >= blocked);
        assert!(lit <= surface * t + (ambient + sun) * (1.0 - t));
    }
}

#[test]
fn shader_budget_is_generated_from_the_tested_cpu_parameters() {
    let constants = shader_constants();
    assert!(constants.contains(&format!("AIR_MAX_DISTANCE: f32 = {MAX_DISTANCE:.1}")));
    assert!(constants.contains(&format!("AIR_MARCH_STEPS: u32 = {MARCH_STEPS}u")));
}
