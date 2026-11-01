//! Nearest-first residency and coalesced invalidation, independent of worker timing.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::world::ChunkCoord;

use super::MAX_IN_FLIGHT;

#[derive(Clone, Debug)]
pub(super) struct RequestStamp {
    pub(super) token: u64,
    halo_revision: u64,
    active: Arc<AtomicBool>,
}

impl RequestStamp {
    pub(super) fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.active.store(false, Ordering::Relaxed);
    }
}

pub(super) struct Residency {
    center: ChunkCoord,
    radius: i32,
    pub(super) desired: BTreeSet<ChunkCoord>,
    pub(super) resident: HashSet<ChunkCoord>,
    pub(super) in_flight: HashMap<ChunkCoord, RequestStamp>,
    dirty: HashSet<ChunkCoord>,
    next_token: u64,
}

impl Residency {
    pub(super) fn new(center: ChunkCoord, radius: i32) -> Self {
        let mut state = Self {
            center,
            radius,
            desired: BTreeSet::new(),
            resident: HashSet::new(),
            in_flight: HashMap::new(),
            dirty: HashSet::new(),
            next_token: 0,
        };
        state.recenter(center);
        state
    }

    pub(super) fn recenter(&mut self, center: ChunkCoord) -> Vec<ChunkCoord> {
        if self.center == center && !self.desired.is_empty() {
            return Vec::new();
        }
        self.center = center;
        self.desired = (-self.radius..=self.radius)
            .flat_map(|z| {
                (-self.radius..=self.radius).filter_map(move |x| {
                    Some(ChunkCoord::new(
                        center.x.checked_add(x)?,
                        center.z.checked_add(z)?,
                    ))
                })
            })
            .collect();
        let mut evicted = Vec::new();
        self.resident.retain(|coord| {
            let keep = self.desired.contains(coord);
            if !keep {
                evicted.push(*coord);
            }
            keep
        });
        self.dirty.retain(|coord| self.desired.contains(coord));
        for (coord, stamp) in &self.in_flight {
            if !self.desired.contains(coord) {
                stamp.cancel();
            }
        }
        evicted
    }

    pub(super) fn invalidate(&mut self, affected: &BTreeSet<ChunkCoord>) {
        self.dirty.extend(affected.intersection(&self.desired));
        for coord in affected {
            if let Some(stamp) = self.in_flight.get(coord) {
                // Keep its slot until completion, even when queued work is skipped.
                stamp.cancel();
            }
        }
    }

    pub(super) fn next_requests(&self) -> Vec<ChunkCoord> {
        let mut missing: Vec<_> = self
            .desired
            .iter()
            .filter(|coord| {
                (!self.resident.contains(coord) || self.dirty.contains(coord))
                    && !self.in_flight.contains_key(coord)
            })
            .copied()
            .collect();
        missing.sort_by_key(|coord| {
            let x = i64::from(coord.x) - i64::from(self.center.x);
            let z = i64::from(coord.z) - i64::from(self.center.z);
            (x * x + z * z, coord.z, coord.x)
        });
        missing.truncate(MAX_IN_FLIGHT.saturating_sub(self.in_flight.len()));
        missing
    }

    pub(super) fn request_stamp(&mut self, halo_revision: u64) -> RequestStamp {
        self.next_token = self.next_token.wrapping_add(1);
        RequestStamp {
            token: self.next_token,
            halo_revision,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(super) fn accept(
        &mut self,
        coord: ChunkCoord,
        token: u64,
        halo_revision: u64,
        complete: bool,
    ) -> bool {
        let Some(stamp) = self.in_flight.get(&coord) else {
            return false;
        };
        if stamp.token != token {
            return false;
        }
        let current = complete
            && stamp.is_active()
            && stamp.halo_revision == halo_revision
            && self.desired.contains(&coord);
        self.in_flight.remove(&coord);
        if current {
            self.dirty.remove(&coord);
            self.resident.insert(coord);
        }
        current
    }

    pub(super) fn pending_count(&self) -> usize {
        self.desired
            .iter()
            .filter(|coord| !self.resident.contains(coord) || self.dirty.contains(coord))
            .count()
    }
}
