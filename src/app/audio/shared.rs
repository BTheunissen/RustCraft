//! A bounded mailbox plus atomic lifecycle priority; neither thread waits for the other.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use super::mixer::MixFrame;
use super::queue::EventQueue;

pub(super) const ACTIVE: u64 = 1;
pub(super) const MUTED: u64 = 2;
pub(super) const STOPPED: u64 = 4;
pub(super) const SCENE_CLEAR: u64 = 8;
pub(super) const FLAG_BITS: u32 = 4;

pub(super) struct Mailbox {
    pub(super) control: u64,
    pub(super) frame: MixFrame,
    pub(super) updated: Instant,
    pub(super) events: EventQueue,
}

pub(super) struct Shared {
    pub(super) control: AtomicU64,
    pub(super) failed: AtomicBool,
    pub(super) ready: AtomicBool,
    pub(super) callbacks: AtomicU64,
    pub(super) mailbox: Mutex<Mailbox>,
}

impl Shared {
    pub(super) fn new() -> Self {
        Self {
            control: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            callbacks: AtomicU64::new(0),
            mailbox: Mutex::new(Mailbox {
                control: 0,
                frame: MixFrame::default(),
                updated: Instant::now(),
                events: EventQueue::default(),
            }),
        }
    }

    pub(super) fn stopped(&self) -> bool {
        self.control.load(Ordering::Acquire) & STOPPED != 0
    }
}
