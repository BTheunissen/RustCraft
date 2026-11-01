//! Bounded settings stored separately from progress; failures never prevent world saves.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::AudioSettings;

const MAGIC: &[u8; 8] = b"RCAUDIO1";
const PREFERENCE_BYTES: usize = 21;
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(1);

pub(in crate::app) struct Preferences {
    path: Option<PathBuf>,
    warned: bool,
}

impl Preferences {
    pub(super) fn disabled() -> Self {
        Self {
            path: None,
            warned: false,
        }
    }

    pub(super) fn local() -> Self {
        match crate::save::SaveSlot::local() {
            Ok(slot) => Self::with_path(slot.path().with_file_name("audio.prefs")),
            Err(error) => {
                let mut preferences = Self::disabled();
                preferences.warn(&error);
                preferences
            }
        }
    }

    fn with_path(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            warned: false,
        }
    }

    pub(in crate::app) fn load(&mut self) -> AudioSettings {
        let result = self
            .path
            .as_ref()
            .map_or(Ok(AudioSettings::default()), |path| read(path));
        match result {
            Ok(settings) => settings,
            Err(error) => {
                self.warn(&error);
                AudioSettings::default()
            }
        }
    }

    pub(super) fn save(&mut self, settings: AudioSettings) {
        if let Some(path) = &self.path
            && let Err(error) = replace(path, &encode(settings.normalized()))
        {
            self.warn(&error);
        }
    }

    pub(super) const fn unavailable(&self) -> bool {
        self.warned
    }

    fn warn(&mut self, error: &io::Error) {
        if !self.warned {
            eprintln!("Audio preferences unavailable; in-memory controls still work: {error}");
            self.warned = true;
        }
    }
}

fn read(path: &Path) -> io::Result<AudioSettings> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(AudioSettings::default());
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != PREFERENCE_BYTES as u64 {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(PREFERENCE_BYTES + 1);
    file.take(PREFERENCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

fn encode(settings: AudioSettings) -> [u8; PREFERENCE_BYTES] {
    let mut bytes = [0; PREFERENCE_BYTES];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8] = u8::from(settings.muted);
    for (destination, gain) in bytes[9..].as_chunks_mut::<4>().0.iter_mut().zip([
        settings.master,
        settings.actions,
        settings.ambience,
    ]) {
        destination.copy_from_slice(&gain.to_le_bytes());
    }
    bytes
}

fn decode(bytes: &[u8]) -> io::Result<AudioSettings> {
    if bytes.len() != PREFERENCE_BYTES || &bytes[..8] != MAGIC || bytes[8] > 1 {
        return Err(invalid());
    }
    let mut gains = [0.0; 3];
    for (gain, bytes) in gains.iter_mut().zip(bytes[9..].as_chunks::<4>().0.iter()) {
        *gain = f32::from_le_bytes(*bytes);
        if !gain.is_finite() || !(0.0..=1.0).contains(gain) {
            return Err(invalid());
        }
    }
    Ok(AudioSettings {
        muted: bytes[8] == 1,
        master: gains[0],
        actions: gains[1],
        ambience: gains[2],
    })
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or unsupported audio preferences",
    )
}

fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "audio preferences need a parent directory",
            )
        })?;
    std::fs::create_dir_all(directory)?;
    #[cfg(unix)]
    let directory_file = File::open(directory)?;
    let (temporary, mut file) = loop {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let temporary =
            path.with_file_name(format!("audio.prefs.{}.{sequence}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (Temporary(temporary), file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    };
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary.0, path)?;
    #[cfg(unix)]
    directory_file.sync_all()?;
    Ok(())
}

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
pub(in crate::app) mod tests;
