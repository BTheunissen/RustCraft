use super::{HELP, LaunchCommand, LaunchOptions, parse_arguments};
use std::path::PathBuf;

fn parse(args: &[&str]) -> Result<LaunchCommand, String> {
    parse_arguments(args.iter().map(|arg| (*arg).into()))
}

#[test]
fn defaults_to_interactive_exploration() {
    assert_eq!(parse(&[]), Ok(LaunchCommand::Run(LaunchOptions::default())));
}

#[test]
fn help_describes_modes_survival_controls_and_exploration_regressions() {
    assert!(HELP.contains("H           Toggle headlamp while exploring"));
    assert!(HELP.contains("caves down to Y=-64"));
    assert!(HELP.contains("translucent geological cutaway"));
    assert!(HELP.contains("lava remains non-damaging scenery"));
    assert!(HELP.contains("Space/Shift Surface/dive"));
    assert!(HELP.contains("Currents carry swimmers; creative flight ignores them"));
    assert!(HELP.contains("Creative remains exploration-only"));
    assert!(HELP.contains("I           Inventory (pauses)"));
    assert!(HELP.contains("9 hotbar + 27 storage slots"));
    assert!(HELP.contains("Continue resumes your one local world"));
    assert!(HELP.contains("replaces it only after confirmation"));
    assert!(HELP.contains("every 60 seconds"));
    assert!(HELP.contains("atomic replacement"));
    assert!(HELP.contains("Smoke tests use no player save"));
}

#[test]
fn parses_seed_render_distance_and_smoke_capture() {
    let LaunchCommand::Run(options) = parse(&[
        "--seed",
        "42",
        "--render-distance",
        "4",
        "--smoke-test",
        "--screenshot",
        "frame.png",
    ])
    .unwrap() else {
        panic!("expected launch options");
    };
    assert_eq!(options.seed, 42);
    assert_eq!(options.render_distance, 4);
    assert!(options.smoke_test);
    assert_eq!(options.screenshot, Some(PathBuf::from("frame.png")));
}

#[test]
fn rejects_missing_invalid_or_out_of_range_values() {
    for args in [
        vec!["--seed"],
        vec!["--seed", "-1"],
        vec!["--seed", "nonsense"],
        vec!["--render-distance"],
        vec!["--render-distance", "1"],
        vec!["--render-distance", "13"],
        vec!["--render-distance", "nan"],
        vec!["--screenshot"],
        vec!["--screenshot", "--smoke-test"],
        vec!["--screenshot", "frame.png"],
        vec!["--unknown"],
    ] {
        assert!(parse(&args).is_err(), "args: {args:?}");
    }
}

#[test]
fn help_does_not_launch_and_unsigned_extreme_seeds_are_valid() {
    assert_eq!(parse(&["--help"]), Ok(LaunchCommand::Help));
    assert_eq!(parse(&["-h"]), Ok(LaunchCommand::Help));
    assert!(parse(&["--seed", "18446744073709551615"]).is_ok());
    assert!(parse(&["--render-distance", "2"]).is_ok());
    assert!(parse(&["--render-distance", "12"]).is_ok());
}

#[test]
fn offscreen_verification_is_explicit_and_accepts_frame_readback() {
    let LaunchCommand::Run(options) =
        parse(&["--offscreen-smoke-test", "--screenshot", "gpu.png"]).unwrap()
    else {
        panic!("expected launch options");
    };
    assert!(options.offscreen && options.smoke_test);
    assert_eq!(options.screenshot, Some(PathBuf::from("gpu.png")));
    assert!(!LaunchOptions::default().offscreen);
}

#[test]
fn audible_check_is_explicit_and_cannot_make_ordinary_smoke_play_sound() {
    let LaunchCommand::Run(options) = parse(&["--audio-check"]).unwrap() else {
        panic!("expected audio check")
    };
    assert!(options.audio_check && !options.smoke_test && !options.audio_disabled);
    let LaunchCommand::Run(options) = parse(&["--no-audio"]).unwrap() else {
        panic!("expected silent launch")
    };
    assert!(options.audio_disabled && !options.audio_check);
    for args in [
        ["--audio-check", "--smoke-test"],
        ["--offscreen-smoke-test", "--audio-check"],
        ["--no-audio", "--audio-check"],
    ] {
        assert!(parse(&args).is_err());
    }
    assert!(!LaunchOptions::default().audio_check);
}

#[test]
fn debug_launch_settings_accept_named_phases_clock_times_and_bounded_rates() {
    for (value, hour) in [
        ("night", 22.0),
        ("dawn", 6.5),
        ("sunset", 17.5),
        ("05:30", 5.5),
        ("17.5", 17.5),
    ] {
        let LaunchCommand::Run(options) =
            parse(&["--time-speed", "60", "--time-of-day", value]).unwrap()
        else {
            panic!("expected launch settings")
        };
        assert_eq!(options.time_of_day, hour);
        assert_eq!(options.time_speed, 60.0);
    }
    for args in [
        vec!["--time-speed"],
        vec!["--time-speed", "nan"],
        vec!["--time-speed", "inf"],
        vec!["--time-speed", "-1"],
        vec!["--time-speed", "121"],
        vec!["--time-of-day"],
        vec!["--time-of-day", "24:00"],
        vec!["--time-of-day", "nope"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    assert!(parse(&["--time-speed", "0"]).is_ok());
    assert!(parse(&["--time-speed", "120"]).is_ok());
    assert!(HELP.contains("/time speed 60") && HELP.contains("20 minutes") && HELP.contains("F6"));
}

#[test]
fn programmatic_launches_cannot_bypass_time_validation() {
    assert!(LaunchOptions::default().validate_time().is_ok());
    assert!(
        LaunchOptions {
            time_speed: f64::NAN,
            ..Default::default()
        }
        .validate_time()
        .is_err()
    );
    assert!(
        LaunchOptions {
            time_of_day: 24.0,
            ..Default::default()
        }
        .validate_time()
        .is_err()
    );
}
