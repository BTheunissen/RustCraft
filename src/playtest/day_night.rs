//! Solar-only same-pose readback and a complete accelerated interactive probe sweep.

use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

use super::{capture_scene, check_deadline};
use crate::game::WorldTime;
use crate::renderer::{AirStats, FrameDifference, FrameState, FrameStats, Renderer};

pub(super) fn verify(
    renderer: &mut Renderer,
    base: &FrameState<'_>,
    capture: Option<&Path>,
    native: bool,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let day = FrameState {
        hud: &[],
        headlamp: false,
        time_of_day: 10.0,
        ..*base
    };
    let night = FrameState {
        time_of_day: 22.0,
        ..day
    };
    let day_stats = capture_scene(renderer, capture, "time-day", &day)?;
    let day_air = renderer.measure_air(&day)?;
    let night_stats = capture_scene(renderer, capture, "time-night", &night)?;
    let night_air = renderer.measure_air(&night)?;
    let difference = renderer.compare_frames(&day, &night)?;
    require_cycle(day_stats, night_stats, difference, day_air, night_air)?;
    let night_lamp = FrameState {
        headlamp: true,
        ..night
    };
    let lamp_change = renderer.compare_frames(&night, &night_lamp)?;
    if lamp_change.mean_absolute_error < 0.015 || lamp_change.changed_fraction < 0.10 {
        return Err(io::Error::other(format!(
            "night headlamp did not light exposed outdoor receivers: {lamp_change:?}"
        ))
        .into());
    }
    capture_scene(renderer, capture, "time-night-headlamp", &night_lamp)?;
    let (width, height) = renderer.size();
    let console = crate::hud::build_time_console(width, height, "/time speed 60", None, "22:00 1X");
    capture_scene(
        renderer,
        capture,
        "time-console",
        &FrameState {
            hud: &console,
            ..night
        },
    )?;
    for (suffix, hour) in [
        ("time-dawn", 6.5),
        ("time-noon", 12.0),
        ("time-sunset", 17.5),
    ] {
        check_deadline(deadline)?;
        capture_scene(
            renderer,
            capture,
            suffix,
            &FrameState {
                time_of_day: hour,
                ..day
            },
        )?;
    }
    let morning = FrameState {
        time_of_day: 7.0,
        ..day
    };
    let evening = FrameState {
        time_of_day: 17.0,
        ..day
    };
    let solar_motion = renderer.compare_frames(&morning, &evening)?;
    if solar_motion.mean_absolute_error < 0.005 || solar_motion.changed_fraction < 0.10 {
        return Err(io::Error::other(format!(
            "opposite equal-elevation sun positions did not move scene lighting: {solar_motion:?}"
        ))
        .into());
    }
    verify_accelerated_sweep(renderer, &day, deadline)?;
    if native && !renderer.render(&night)? {
        return Err(io::Error::other("day-night completion frame was not presented").into());
    }
    println!(
        "Day-night GPU verified: solar-only coast luminance {:.4} → {:.4}, {:.1}% changed; day air peak {:.5}, night air zero; moving shadows/sky/water and a complete 60x interactive sweep",
        day_stats.mean_luminance,
        night_stats.mean_luminance,
        difference.changed_fraction * 100.0,
        day_air.peak_radiance,
    );
    Ok(())
}

fn verify_accelerated_sweep(
    renderer: &mut Renderer,
    base: &FrameState<'_>,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let mut clock = WorldTime::default();
    clock.command("/time speed 60").map_err(io::Error::other)?;
    renderer.measure_frame(base)?;
    renderer.with_interactive_lighting(|renderer| {
        for step in 1_u32..=144 {
            check_deadline(deadline)?;
            clock.advance(20.0 / 144.0);
            let frame = FrameState {
                time_of_day: clock.hour(),
                elapsed: base.elapsed + step as f32 * (20.0 / 144.0),
                ..*base
            };
            if step.is_multiple_of(36) {
                let (_, reference) = renderer.compare_interactive_frame(&frame)?;
                if reference.mean_absolute_error > 0.012
                    || !reference.mean_absolute_error.is_finite()
                {
                    return Err(io::Error::other(format!(
                        "accelerated solar probes retained stale light at {}: {reference:?}",
                        clock.label(),
                    ))
                    .into());
                }
            } else {
                renderer.render_offscreen(&frame)?;
            }
        }
        if (clock.hour() - WorldTime::default().hour()).abs() > 0.00001 {
            return Err(
                io::Error::other("accelerated diagnostic did not complete a solar cycle").into(),
            );
        }
        // Also exercise partial accelerated batches at a representative 60 Hz step.
        clock.set_hour(16.0).map_err(io::Error::other)?;
        renderer.measure_frame(&FrameState {
            time_of_day: clock.hour(),
            ..*base
        })?;
        for step in 0..16 {
            clock.advance(1.0 / 60.0);
            renderer.render_offscreen(&FrameState {
                time_of_day: clock.hour(),
                elapsed: base.elapsed + 20.0 + step as f32 / 60.0,
                ..*base
            })?;
        }
        let frame = FrameState {
            time_of_day: clock.hour(),
            elapsed: base.elapsed + 20.0 + 16.0 / 60.0,
            ..*base
        };
        let (_, reference) = renderer.compare_interactive_frame(&frame)?;
        if reference.mean_absolute_error > 0.012 || !reference.mean_absolute_error.is_finite() {
            return Err(io::Error::other(format!(
                "60 Hz accelerated partial probe budget lagged: {reference:?}"
            ))
            .into());
        }
        Ok(())
    })?;
    Ok(())
}

fn require_cycle(
    day: FrameStats,
    night: FrameStats,
    difference: FrameDifference,
    day_air: AirStats,
    night_air: AirStats,
) -> io::Result<()> {
    if !day.mean_luminance.is_finite()
        || !night.mean_luminance.is_finite()
        || !night.luminance_deviation.is_finite()
        || !difference.changed_fraction.is_finite()
        || !difference.mean_absolute_error.is_finite()
        || !day_air.peak_radiance.is_finite()
        || !night_air.peak_radiance.is_finite()
        || !night_air.lit_fraction.is_finite()
        || day.mean_luminance < 0.20
        || night.mean_luminance >= day.mean_luminance * 0.65
        || night.luminance_deviation < 0.003
        || difference.changed_fraction < 0.60
        || difference.mean_absolute_error < 0.08
        || day_air.peak_radiance < 0.001
        || night_air.peak_radiance > 0.000001
        || night_air.lit_fraction != 0.0
    {
        return Err(io::Error::other(format!(
            "day-night GPU contrast or sun extinction failed: day {day:?}, night {night:?}, change {difference:?}, air {day_air:?} → {night_air:?}",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_rejects_identical_phases_flat_night_stale_sun_and_invalid_readback() {
        let day = FrameStats {
            mean_luminance: 0.55,
            visible_fraction: 1.0,
            luminance_deviation: 0.12,
        };
        let night = FrameStats {
            mean_luminance: 0.08,
            visible_fraction: 0.5,
            luminance_deviation: 0.03,
        };
        let difference = FrameDifference {
            mean_absolute_error: 0.30,
            changed_fraction: 0.98,
        };
        let air = AirStats {
            mean_radiance: 0.005,
            peak_radiance: 0.03,
            deviation: 0.001,
            lit_fraction: 0.3,
        };
        let dark_air = AirStats {
            mean_radiance: 0.0,
            peak_radiance: 0.0,
            deviation: 0.0,
            lit_fraction: 0.0,
        };
        assert!(require_cycle(day, night, difference, air, dark_air).is_ok());
        assert!(require_cycle(day, day, difference, air, dark_air).is_err());
        assert!(
            require_cycle(
                day,
                FrameStats {
                    luminance_deviation: 0.0,
                    ..night
                },
                difference,
                air,
                dark_air
            )
            .is_err()
        );
        assert!(require_cycle(day, night, difference, air, air).is_err());
        assert!(
            require_cycle(
                day,
                night,
                FrameDifference {
                    mean_absolute_error: f32::NAN,
                    ..difference
                },
                air,
                dark_air
            )
            .is_err()
        );
        assert!(
            require_cycle(
                day,
                night,
                difference,
                AirStats {
                    peak_radiance: f32::NAN,
                    ..air
                },
                dark_air
            )
            .is_err()
        );
    }
}
