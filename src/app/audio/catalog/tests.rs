use super::*;

#[test]
fn original_catalog_is_finite_quiet_bounded_and_not_empty() {
    let bank = test_bank();
    assert!(bank.bytes() <= 32 * 1024 * 1024, "{} bytes", bank.bytes());
    println!(
        "Original audio bank: {} clips, {} decoded bytes ({:.2} MiB), 48 kHz mono f32",
        CLIP_COUNT,
        bank.bytes(),
        bank.bytes() as f64 / 1_048_576.0
    );
    for index in 0..CLIP_COUNT {
        let samples = bank.clip(index);
        assert!(
            samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 0.301)
        );
        let energy = samples
            .iter()
            .map(|&value| f64::from(value).powi(2))
            .sum::<f64>();
        let rms = (energy / samples.len() as f64).sqrt();
        if index >= ACTION_CLIPS {
            let peak = samples
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
            println!(
                "Ambient {}: {:.0}s, peak {peak:.3}, RMS {rms:.5}",
                index - ACTION_CLIPS,
                samples.len() as f64 / f64::from(SOURCE_RATE)
            );
        }
        assert!((0.001..0.10).contains(&rms), "clip {index}: RMS {rms}");
        if index < ACTION_CLIPS {
            assert!(samples.len() <= SOURCE_RATE as usize * 3 / 4);
            assert_eq!(samples[0], 0.0);
            assert_eq!(*samples.last().unwrap(), 0.0);
        } else {
            assert!(samples.len() >= 24 * SOURCE_RATE as usize);
            let mean =
                samples.iter().map(|&value| f64::from(value)).sum::<f64>() / samples.len() as f64;
            assert!(mean.abs() < 0.000_001);
            let jump = (samples[0] - samples[samples.len() - 1]).abs();
            let largest = samples
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .fold(0.0_f32, f32::max);
            assert!(
                jump <= largest,
                "clip {index}: seam {jump} versus interior {largest}"
            );
        }
    }
}

#[test]
fn mapping_keeps_surface_variants_distinct_and_all_actions_inside_the_bank() {
    for surface in SURFACES {
        let steps = std::array::from_fn::<_, 3, _>(|variant| {
            action_clip(SoundAction::Step(surface), variant)
        });
        assert!(steps[0] != steps[1] && steps[1] != steps[2] && steps[0] != steps[2]);
        assert_eq!(action_clip(SoundAction::Step(surface), 3), steps[0]);
        for action in [
            SoundAction::Mine(surface),
            SoundAction::Break(surface),
            SoundAction::Place(surface),
        ] {
            assert!(action_clip(action, usize::MAX) < ACTION_CLIPS);
        }
    }
    assert_ne!(synthesis::clip(0), synthesis::clip(1));
    assert_eq!(synthesis::clip(0), synthesis::clip(0));
}
