use super::*;
use crate::save::test_support::TestDirectory;

pub(in crate::app) fn isolated(path: PathBuf) -> Preferences {
    Preferences::with_path(path)
}

#[test]
fn round_trip_is_separate_from_world_progress_and_missing_preferences_are_defaults() {
    let directory = TestDirectory::new();
    let world = directory.path.join("world.save");
    std::fs::write(&world, b"real world sentinel - do not touch").unwrap();
    let path = directory.path.join("audio.prefs");
    let mut preferences = Preferences::with_path(path.clone());
    assert_eq!(preferences.load(), AudioSettings::default());
    assert!(!path.exists());
    let settings = AudioSettings {
        muted: true,
        master: 0.25,
        actions: 0.65,
        ambience: 0.15,
    };
    preferences.save(settings);
    assert!(!preferences.warned);
    assert_eq!(preferences.load(), settings);
    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        PREFERENCE_BYTES as u64
    );
    assert_eq!(
        std::fs::read(&world).unwrap(),
        b"real world sentinel - do not touch"
    );
    assert_eq!(std::fs::read_dir(&directory.path).unwrap().count(), 2);
}

#[test]
fn malformed_oversized_future_and_nonfinite_settings_fall_back_without_rewriting() {
    let directory = TestDirectory::new();
    let path = directory.path.join("audio.prefs");
    let valid = encode(AudioSettings::default());
    let mut future = valid;
    future[7] = b'2';
    let mut boolean = valid;
    boolean[8] = 2;
    for bytes in [
        Vec::new(),
        valid[..10].to_vec(),
        vec![0; 4096],
        future.to_vec(),
        boolean.to_vec(),
        encode(AudioSettings {
            master: f32::NAN,
            ..Default::default()
        })
        .to_vec(),
        encode(AudioSettings {
            actions: 1.01,
            ..Default::default()
        })
        .to_vec(),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        let mut preferences = Preferences::with_path(path.clone());
        assert_eq!(preferences.load(), AudioSettings::default());
        assert!(preferences.unavailable());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn failed_atomic_replacement_leaves_world_and_foreign_staging_intact() {
    let directory = TestDirectory::new();
    let world = directory.path.join("world.save");
    std::fs::write(&world, b"unchanged world").unwrap();
    let path = directory.path.join("audio.prefs");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("owned-by-user"), b"preserve").unwrap();
    std::fs::write(
        directory.path.join("audio.prefs.foreign.tmp"),
        b"other writer",
    )
    .unwrap();
    let mut preferences = Preferences::with_path(path.clone());
    preferences.save(AudioSettings::default());
    assert!(preferences.unavailable());
    preferences.save(AudioSettings {
        muted: true,
        ..Default::default()
    });
    assert_eq!(std::fs::read(&world).unwrap(), b"unchanged world");
    assert_eq!(
        std::fs::read(path.join("owned-by-user")).unwrap(),
        b"preserve"
    );
    assert_eq!(
        std::fs::read(directory.path.join("audio.prefs.foreign.tmp")).unwrap(),
        b"other writer"
    );
    assert_eq!(std::fs::read_dir(&directory.path).unwrap().count(), 3);
}

#[test]
fn disabled_or_invalid_parent_storage_has_no_disk_side_effects() {
    let directory = TestDirectory::new();
    let blocker = directory.path.join("blocked");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let mut preferences = Preferences::with_path(blocker.join("audio.prefs"));
    preferences.save(AudioSettings::default());
    assert!(preferences.warned);
    assert_eq!(std::fs::read(&blocker).unwrap(), b"not a directory");
    let mut disabled = Preferences::disabled();
    disabled.save(AudioSettings {
        muted: true,
        ..Default::default()
    });
    assert_eq!(disabled.load(), AudioSettings::default());
    assert!(!disabled.warned);
    assert!(replace(Path::new("audio.prefs"), b"invalid").is_err());
}

#[test]
fn a_foreign_staging_filename_collision_is_never_deleted_or_reused() {
    let directory = TestDirectory::new();
    let path = directory.path.join("audio.prefs");
    let start = NEXT_TEMPORARY.load(Ordering::Relaxed);
    let foreign: Vec<_> = (start..start + 128)
        .map(|sequence| {
            directory
                .path
                .join(format!("audio.prefs.{}.{sequence}.tmp", std::process::id()))
        })
        .collect();
    for path in &foreign {
        std::fs::write(path, b"foreign partial writer").unwrap();
    }
    let mut preferences = Preferences::with_path(path);
    preferences.save(AudioSettings::default());
    assert!(!preferences.unavailable());
    for path in foreign {
        assert_eq!(std::fs::read(path).unwrap(), b"foreign partial writer");
    }
    assert_eq!(preferences.load(), AudioSettings::default());
}
