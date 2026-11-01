use super::*;

#[test]
fn ordinary_and_accelerated_cycles_wrap_without_changing_the_rate() {
    for speed in [1.0, 10.0, 60.0, MAX_TIME_SPEED] {
        let mut time = WorldTime::default();
        time.set_hour(23.99).unwrap();
        time.set_speed(speed).unwrap();
        let start = time.hour();
        for _ in 0..4800 {
            time.advance(0.25);
            assert!((0.0..24.0).contains(&time.hour()));
        }
        assert!((time.hour() - start).abs() < 0.000_000_001);
        assert_eq!(time.speed(), speed);
    }
}

#[test]
fn simulation_partitioning_is_stable_and_invalid_or_hitched_deltas_are_bounded() {
    let mut fine = WorldTime::default();
    fine.set_speed(60.0).unwrap();
    let mut coarse = fine;
    for _ in 0..64 {
        fine.advance(1.0 / 64.0);
    }
    for _ in 0..4 {
        coarse.advance(0.25);
    }
    assert!((fine.hour() - coarse.hour()).abs() < 0.000_000_001);
    let unchanged = fine;
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        fine.advance(dt);
        assert_eq!(fine, unchanged);
    }
    fine.advance(1000.0);
    coarse.advance(0.25);
    assert!((fine.hour() - coarse.hour()).abs() < 0.000_000_001);
    fine.set_speed(0.0).unwrap();
    let frozen = fine;
    fine.advance(0.25);
    assert_eq!(fine, frozen);
}

#[test]
fn setting_hour_or_rate_is_validated_before_mutation() {
    let mut time = WorldTime::default();
    for hour in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.1,
        24.0,
        1000.0,
    ] {
        let before = time;
        assert!(time.set_hour(hour).is_err());
        assert_eq!(time, before);
    }
    for speed in [f64::NAN, f64::INFINITY, -1.0, MAX_TIME_SPEED + 0.1] {
        let before = time;
        assert!(time.set_speed(speed).is_err());
        assert_eq!(time, before);
    }
    time.set_hour(0.0).unwrap();
    time.set_speed(0.5).unwrap();
    assert_eq!(time.label(), "00:00 0.50X");
    time.set_hour(23.999).unwrap();
    time.set_speed(120.0).unwrap();
    assert_eq!(time.label(), "23:59 120X");
}

#[test]
fn preset_rates_freeze_and_restore_only_the_solar_clock() {
    let mut time = WorldTime::default();
    let hour = time.hour();
    for expected in [10.0, 60.0, 120.0, 0.0, 1.0] {
        time.cycle_speed();
        assert_eq!(time.speed(), expected);
        assert_eq!(time.hour(), hour);
    }
    time.set_speed(3.5).unwrap();
    time.cycle_speed();
    assert_eq!(time.speed(), 1.0);
}

#[test]
fn time_commands_support_showcase_phases_clock_times_and_fractional_speed() {
    let mut time = WorldTime::default();
    for (phase, hour) in [
        ("dawn", 6.5),
        ("day", 10.0),
        ("noon", 12.0),
        ("dusk", 18.0),
        ("sunset", 17.5),
        ("night", 22.0),
        ("midnight", 0.0),
        ("05:30", 5.5),
        ("23:59", 23.0 + 59.0 / 60.0),
        ("16.75", 16.75),
    ] {
        let result = time.command(&format!(" /time set {phase} ")).unwrap();
        assert!(result.changed && result.message.contains(&time.label()));
        assert_eq!(time.hour(), hour);
    }
    for rate in ["0", "1", "60", "120", "0.25"] {
        let hour = time.hour();
        assert!(time.command(&format!("time speed {rate}")).unwrap().changed);
        assert_eq!(time.speed(), rate.parse::<f64>().unwrap());
        assert_eq!(time.hour(), hour);
    }
    let unchanged = time;
    for command in ["/time", "time query", "time help"] {
        assert!(!time.command(command).unwrap().changed);
        assert_eq!(time, unchanged);
    }
}

#[test]
fn malformed_debug_commands_leave_phase_and_speed_unchanged() {
    let mut time = WorldTime::default();
    let original = time;
    for command in [
        "",
        "/speed 60",
        "/time speed",
        "/time set",
        "/time query 4",
        "/time help extra",
        "/time set noon extra",
        "/time speed 2 extra",
        "/time set 24:00",
        "/time set 23:60",
        "/time set -1:30",
        "/time set 12:30:00",
        "/time set nan",
        "/time set inf",
        "/time set 24",
        "/time set -1",
        "/time set nowhere",
        "/time speed -1",
        "/time speed 121",
        "/time speed NaN",
        "/time speed inf",
    ] {
        assert!(time.command(command).is_err(), "{command}");
        assert_eq!(time, original, "{command}");
    }
}
