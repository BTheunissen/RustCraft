use super::*;

#[test]
fn daylight_preserves_the_existing_palette_and_night_removes_direct_sunlight() {
    let day = CelestialLight::default();
    assert_eq!(day.sun_radiance, Vec3::new(1.65, 1.40, 1.10));
    assert_eq!(day.sky_top, Vec3::new(0.12, 0.32, 0.67));
    assert_eq!(day.daylight, 1.0);
    assert_eq!(day.stars, 0.0);
    let night = CelestialLight::sample(22.0);
    assert!(night.sun_direction.y < 0.0);
    assert_eq!(night.sun_radiance, Vec3::ZERO);
    assert_eq!(night.daylight, 0.0);
    assert_eq!(night.stars, 1.0);
    assert!(night.sky_irradiance.max_element() < day.sky_irradiance.min_element() * 0.2);
    assert!(night.sky_horizon.z > night.sky_horizon.x);
}

#[test]
fn low_sun_warms_the_light_and_moves_shadows_to_the_opposite_side_at_sunset() {
    let morning = CelestialLight::sample(6.5);
    let evening = CelestialLight::sample(17.5);
    let noon = CelestialLight::sample(12.0);
    assert!(morning.sun_direction.x < -0.9 && evening.sun_direction.x > 0.9);
    assert!(noon.sun_direction.y > 0.9);
    for low in [morning, evening] {
        assert!(low.sun_radiance.x > low.sun_radiance.z * 3.0);
        assert!(low.sky_horizon.x > low.sky_horizon.z);
        assert!(low.sky_top.max_element() < noon.sky_top.max_element());
    }
}

#[test]
fn the_complete_orbit_is_continuous_bounded_and_periodic_even_at_midnight() {
    let mut previous = CelestialLight::sample(0.0);
    for step in 1..=2400 {
        let hour = f64::from(step) / 100.0;
        let light = CelestialLight::sample(hour);
        assert!((light.sun_direction.length() - 1.0).abs() < 0.000001);
        assert!(light.sun_direction.is_finite());
        assert!((light.sun_direction - previous.sun_direction).length() < 0.003);
        for (color, before) in [
            (light.sun_radiance, previous.sun_radiance),
            (light.sky_irradiance, previous.sky_irradiance),
            (light.sky_top, previous.sky_top),
            (light.sky_horizon, previous.sky_horizon),
            (light.clouds, previous.clouds),
        ] {
            assert!(color.is_finite() && color.min_element() >= 0.0 && color.max_element() < 2.0);
            assert!((color - before).abs().max_element() < 0.04);
        }
        assert!((0.0..=1.0).contains(&light.daylight));
        assert!((0.0..=1.0).contains(&light.stars));
        let periodic = CelestialLight::sample(hour + 24.0);
        assert!(
            (light.sun_direction - periodic.sun_direction)
                .abs()
                .max_element()
                < 0.000001
        );
        assert!(
            (light.sun_radiance - periodic.sun_radiance)
                .abs()
                .max_element()
                < 0.000001
        );
        previous = light;
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(CelestialLight::sample(invalid), CelestialLight::default());
    }
    assert_eq!(CelestialLight::sample(-2.0), CelestialLight::sample(22.0));
}

#[test]
fn debug_jumps_and_horizon_crossings_refresh_but_ordinary_ticks_keep_the_fixed_budget() {
    assert!(refresh_probes(None, 10.0));
    for (previous, hour) in [(10.0, 22.0), (22.0, 10.0), (5.99, 6.01), (17.99, 18.01)] {
        assert!(refresh_probes(Some(previous), hour));
    }
    for (previous, hour) in [(10.0, 10.0), (10.0, 10.04), (23.99, 0.01), (0.01, 23.99)] {
        assert!(!refresh_probes(Some(previous), hour));
    }
}

#[test]
fn accelerated_time_raises_the_bounded_budget_instead_of_leaving_stale_bounce() {
    use crate::lighting::volume::{PROBE_COUNT, PROBES_PER_FRAME};
    assert_eq!(solar_probe_budget(None, 10.0), PROBE_COUNT);
    assert_eq!(solar_probe_budget(Some(10.0), 10.0), PROBES_PER_FRAME);
    assert_eq!(
        solar_probe_budget(Some(10.0), 10.0 + 24.0 / 1200.0 / 60.0),
        PROBES_PER_FRAME
    );
    let accelerated = solar_probe_budget(Some(10.0), 10.02);
    assert!(accelerated > PROBES_PER_FRAME && accelerated < PROBE_COUNT);
    assert_eq!(solar_probe_budget(Some(10.0), 10.2), PROBE_COUNT);
    assert!(solar_probe_budget(Some(23.999), 0.001) <= PROBES_PER_FRAME);
    for step in 0..=1200 {
        let delta = f64::from(step) / 1200.0;
        let budget = solar_probe_budget(Some(10.0), 10.0 + delta);
        assert!((PROBES_PER_FRAME..=PROBE_COUNT).contains(&budget));
        assert!(budget.is_multiple_of(PROBES_PER_FRAME));
        if delta <= 0.06 {
            assert!(delta * f64::from(PROBE_COUNT.div_ceil(budget)) <= 0.0600001);
        }
    }
}
