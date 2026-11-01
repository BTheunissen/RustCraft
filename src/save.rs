//! One local progress slot with bounded, versioned data and atomic replacement.
//!
//! Generated terrain and resident chunks are never written. An interrupted write
//! leaves either the previous complete slot or the new complete slot, not a prefix.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::game::GameSnapshot;

mod atomic;
mod format;
mod path;

/// A single local save file. Custom paths support isolated tests and CPU tools.
#[derive(Clone, Debug)]
pub struct SaveSlot {
    path: PathBuf,
}

impl SaveSlot {
    /// Selects RustCraft's per-user application-data directory, not the launch folder.
    /// Merely opening the menu never creates a directory or changes an existing save.
    pub fn local() -> io::Result<Self> {
        Ok(Self::new(path::local_directory()?.join("world.save")))
    }

    /// Uses exactly this slot path; temporary writes stay in the same directory.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Location of the durable slot, useful when backing up local progress.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Any existing entry requires replacement confirmation, even an unreadable or
    /// incompatible save. Never mistake an invalid world for an empty slot.
    pub fn occupied(&self) -> io::Result<bool> {
        match std::fs::symlink_metadata(&self.path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Loads only the committed slot. Abandoned temporary writes are ignored.
    /// Missing slots are normal; corrupt, oversized, or future saves are errors.
    pub fn load(&self) -> io::Result<Option<GameSnapshot>> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(format::invalid("local save is not a regular file"));
        }
        if metadata.len() > format::MAX_SAVE_BYTES as u64 {
            return Err(format::invalid("local save is too large"));
        }
        let mut bytes = Vec::new();
        file.take(format::MAX_SAVE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        format::decode(&bytes).map(Some)
    }

    /// Validates before touching disk, flushes a separate file, then atomically
    /// renames it over the slot and flushes directory metadata on Unix platforms.
    pub fn write(&self, snapshot: &GameSnapshot) -> io::Result<()> {
        let bytes = format::encode(snapshot)?;
        atomic::replace(&self.path, |file| file.write_all(&bytes))
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
