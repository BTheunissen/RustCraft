//! Exclusively owned temporary directories; tests never access the player's slot.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::SaveSlot;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

pub(crate) struct TestDirectory {
    pub(crate) path: PathBuf,
}

impl TestDirectory {
    pub(crate) fn new() -> Self {
        loop {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rustcraft-save-tests-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create isolated save fixture: {error}"),
            }
        }
    }

    pub(crate) fn slot(&self) -> SaveSlot {
        SaveSlot::new(self.path.join("world.save"))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
