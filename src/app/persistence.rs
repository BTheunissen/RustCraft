//! Local-slot availability and wall-clock autosave scheduling, separate from rendering.

use std::io;
use std::time::{Duration, Instant};

use crate::game::{Game, GameSnapshot};
use crate::hud::SaveNotice;
use crate::save::SaveSlot;

pub(super) const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

enum Storage {
    Disabled,
    Local(SaveSlot),
    Unavailable(String),
}

pub(super) struct Persistence {
    storage: Storage,
    occupied: bool,
    active: bool,
    next_save: Instant,
    pub(super) notice: Option<SaveNotice>,
}

impl Persistence {
    pub(super) fn local() -> Self {
        match SaveSlot::local() {
            Ok(slot) => {
                println!("Local world save: {}", slot.path().display());
                Self::with_slot(slot)
            }
            Err(error) => {
                eprintln!("Cannot locate the local save: {error}");
                Self {
                    storage: Storage::Unavailable(error.to_string()),
                    notice: Some(SaveNotice::Unavailable),
                    ..Self::disabled()
                }
            }
        }
    }

    pub(super) fn with_slot(slot: SaveSlot) -> Self {
        let mut persistence = Self {
            storage: Storage::Local(slot),
            ..Self::disabled()
        };
        if let Err(error) = persistence.check_occupied() {
            persistence.occupied = true;
            persistence.warn(SaveNotice::Unavailable, &error);
        }
        persistence
    }

    /// Smoke tests and unit-test menu previews must never read or overwrite player saves.
    pub(super) fn disabled() -> Self {
        Self {
            storage: Storage::Disabled,
            occupied: false,
            active: false,
            next_save: Instant::now() + AUTOSAVE_INTERVAL,
            notice: None,
        }
    }

    pub(super) fn can_continue(&self) -> bool {
        self.occupied && matches!(self.storage, Storage::Local(_))
    }

    /// Recheck at the new-world boundary: another launch may have created the slot.
    pub(super) fn check_occupied(&mut self) -> io::Result<bool> {
        self.occupied = match &self.storage {
            Storage::Disabled => false,
            Storage::Local(slot) => slot.occupied()?,
            Storage::Unavailable(error) => return Err(io::Error::other(error.clone())),
        };
        Ok(self.occupied)
    }

    pub(super) fn load(&mut self) -> io::Result<GameSnapshot> {
        let result = match &self.storage {
            Storage::Local(slot) => slot.load().and_then(|snapshot| {
                snapshot.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "no saved world to continue")
                })
            }),
            _ => Err(io::Error::other("local save is unavailable")),
        };
        if let Err(error) = &result {
            self.warn(
                if error.kind() == io::ErrorKind::Unsupported {
                    SaveNotice::Incompatible
                } else {
                    SaveNotice::LoadFailed
                },
                error,
            );
        }
        result
    }

    pub(super) fn begin_world(&mut self, now: Instant) {
        self.active = true;
        self.next_save = now + AUTOSAVE_INTERVAL;
        self.notice = None;
    }

    pub(super) fn end_world(&mut self) {
        self.active = false;
    }

    pub(super) fn due(&self, now: Instant) -> bool {
        self.wake_deadline().is_some_and(|deadline| now >= deadline)
    }

    /// The native loop can wait for this even with an occluded/zero-sized window.
    pub(super) fn wake_deadline(&self) -> Option<Instant> {
        (self.active && !matches!(self.storage, Storage::Disabled)).then_some(self.next_save)
    }

    pub(super) fn save_active(&mut self, game: &Game, now: Instant) -> io::Result<()> {
        if self.active {
            self.write(&game.snapshot(), now)?;
        }
        Ok(())
    }

    /// A new world commits before replacing the live scene; a failed write keeps
    /// the old slot and menu intact. Failed autosaves wait one interval to retry.
    pub(super) fn write(&mut self, snapshot: &GameSnapshot, now: Instant) -> io::Result<()> {
        self.next_save = now + AUTOSAVE_INTERVAL;
        let result = match &self.storage {
            Storage::Disabled => return Ok(()),
            Storage::Local(slot) => slot.write(snapshot),
            Storage::Unavailable(error) => Err(io::Error::other(error.clone())),
        };
        match &result {
            Ok(()) => {
                self.occupied = true;
                self.notice = Some(SaveNotice::Saved);
            }
            Err(error) => self.warn(SaveNotice::SaveFailed, error),
        }
        result
    }

    pub(super) fn warn(&mut self, notice: SaveNotice, error: &impl std::fmt::Display) {
        self.notice = Some(notice);
        eprintln!("Local world persistence: {error}");
    }
}

#[cfg(test)]
mod tests;
