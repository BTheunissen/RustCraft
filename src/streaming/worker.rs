//! Bounded CPU preparation using immutable session snapshots.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};

use crate::mesh::mesh_chunk_with_edits;
use crate::world::{Chunk, ChunkCoord, Terrain, WorldEdits};

use super::PreparedChunk;
use super::residency::RequestStamp;

pub(super) struct WorkRequest {
    pub(super) coord: ChunkCoord,
    pub(super) stamp: RequestStamp,
    pub(super) edits: Arc<WorldEdits>,
}

pub(super) struct WorkerResult {
    pub(super) coord: ChunkCoord,
    pub(super) token: u64,
    pub(super) prepared: Option<PreparedChunk>,
}

pub(super) fn run_worker(
    terrain: Terrain,
    work: Receiver<WorkRequest>,
    finished: SyncSender<WorkerResult>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        let Ok(request) = work.recv() else {
            break;
        };
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let prepared = prepare(&terrain, &request, stop);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        // Cancellations complete too, releasing their bounded in-flight slot.
        if finished
            .send(WorkerResult {
                coord: request.coord,
                token: request.stamp.token,
                prepared,
            })
            .is_err()
        {
            break;
        }
    }
}

fn prepare(terrain: &Terrain, request: &WorkRequest, stop: &AtomicBool) -> Option<PreparedChunk> {
    if !request.stamp.is_active() || stop.load(Ordering::Relaxed) {
        return None;
    }
    let mut chunk = Chunk::generate(terrain, request.coord);
    request.edits.apply_to_chunk(&mut chunk);
    if !request.stamp.is_active() || stop.load(Ordering::Relaxed) {
        return None;
    }
    let mesh = mesh_chunk_with_edits(&chunk, terrain, &request.edits);
    if !request.stamp.is_active() || stop.load(Ordering::Relaxed) {
        return None;
    }
    Some(PreparedChunk {
        coord: request.coord,
        chunk,
        mesh,
        edit_revision: request.edits.revision(),
        request_token: request.stamp.token,
    })
}
