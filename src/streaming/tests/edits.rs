use super::*;
use crate::world::{Block, Chunk, ResidentWorld, WORLD_HEIGHT};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn load_all(streamer: &mut ChunkStreamer) -> Vec<super::super::PreparedChunk> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut ready = Vec::new();
    while streamer.pending_count() > 0 {
        ready.extend(streamer.poll_ready(3));
        assert!(!streamer.worker_failed());
        assert!(Instant::now() < deadline, "worker stalled");
        std::thread::yield_now();
    }
    ready
}

#[test]
fn dirty_resident_chunks_are_replacements_not_duplicate_residency() {
    let center = ChunkCoord::new(-1, -1);
    let mut state = Residency::new(center, 1);
    state.resident.extend(state.desired.iter().copied());
    assert_eq!(state.pending_count(), 0);
    let affected = BTreeSet::from([center, ChunkCoord::new(0, 0)]);
    state.invalidate(&affected);
    state.invalidate(&affected);
    assert_eq!(state.pending_count(), 2);
    assert_eq!(state.resident.len(), 9);
    assert_eq!(state.next_requests(), vec![center, ChunkCoord::new(0, 0)]);
    let token = queue(&mut state, center, 7);
    assert!(state.accept(center, token, 7, true));
    assert_eq!(state.resident.len(), 9);
    assert_eq!(state.pending_count(), 1);
    assert!(!state.accept(center, token, 7, true));
}

#[test]
fn stale_tokens_do_not_remove_new_requests_and_revision_mismatches_release_old_work() {
    let center = ChunkCoord::new(-1, -1);
    let mut state = Residency::new(center, 1);
    let old = queue(&mut state, center, 1);
    assert!(!state.accept(center, old, 2, true));
    assert!(state.in_flight.is_empty());
    let new = queue(&mut state, center, 2);
    assert_ne!(old, new);
    assert!(!state.accept(center, old, 1, true));
    assert_eq!(state.in_flight[&center].token, new);
    assert!(state.accept(center, new, 2, true));
    assert_eq!(state.resident.len(), 1);
}

#[test]
fn repeated_edits_cancel_and_coalesce_without_expanding_in_flight_work() {
    let center = ChunkCoord::new(0, 0);
    let mut state = Residency::new(center, 2);
    for coord in state.next_requests() {
        queue(&mut state, coord, 1);
    }
    let affected = state.desired.clone();
    for _ in 0..64 {
        state.invalidate(&affected);
        assert_eq!(state.in_flight.len(), MAX_IN_FLIGHT);
        assert!(state.next_requests().is_empty());
    }
    assert!(state.in_flight.values().all(|stamp| !stamp.is_active()));
    let completed: Vec<_> = state
        .in_flight
        .iter()
        .map(|(&coord, stamp)| (coord, stamp.token))
        .collect();
    for (coord, token) in completed {
        assert!(!state.accept(coord, token, 64, true));
    }
    assert_eq!(state.next_requests().len(), MAX_IN_FLIGHT);
    assert_eq!(state.pending_count(), 25);
    assert!(state.resident.is_empty());
}

#[test]
fn recenter_drops_dirty_residency_and_never_accepts_canceled_requests_after_return() {
    let center = ChunkCoord::new(0, 0);
    let mut state = Residency::new(center, 1);
    state.resident.extend(state.desired.iter().copied());
    state.invalidate(&state.desired.clone());
    let token = queue(&mut state, center, 1);
    assert_eq!(state.recenter(ChunkCoord::new(100, -100)).len(), 9);
    assert_eq!(state.pending_count(), 9);
    state.recenter(center);
    assert!(!state.accept(center, token, 1, true));
    assert_eq!(state.pending_count(), 9);
    assert!(state.next_requests().contains(&center));
}

#[test]
fn unchanged_snapshots_are_constant_time_and_distant_edits_do_not_cancel_valid_work() {
    let terrain = Terrain::new(42);
    let center = ChunkCoord::new(-1, -1);
    let mut resident = ResidentWorld::default();
    let distant = ChunkCoord::new(10, 10);
    resident.insert(Chunk::generate(&terrain, distant));
    resident.edit_block([160, WORLD_HEIGHT - 3, 160], Block::Wood);
    let mut streamer = ChunkStreamer::new(terrain, center, 1).unwrap();
    streamer.update_center(center);
    let stamps: Vec<_> = streamer
        .residency
        .in_flight
        .iter()
        .map(|(&coord, stamp)| (coord, stamp.token))
        .collect();
    streamer.sync_edits(resident.edits());
    let snapshot = Arc::clone(&streamer.edits);
    streamer.sync_edits(resident.edits());
    assert!(Arc::ptr_eq(&snapshot, &streamer.edits));
    for (coord, token) in stamps {
        let stamp = &streamer.residency.in_flight[&coord];
        assert_eq!(stamp.token, token);
        assert!(stamp.is_active());
    }
    let ready = load_all(&mut streamer);
    assert_eq!(ready.len(), 9);
    assert_eq!(streamer.loaded_count(), 9);
}

#[test]
fn worker_rejects_prepared_old_snapshot_then_delivers_latest_coalesced_edit_and_halo() {
    let terrain = Terrain::default();
    let center = ChunkCoord::new(-1, -1);
    let position = [-1, WORLD_HEIGHT - 3, -1];
    let mut resident = ResidentWorld::default();
    resident.insert(Chunk::generate(&terrain, center));
    let mut streamer = ChunkStreamer::new(terrain, center, 1).unwrap();
    streamer.update_center(center);
    let old_result = streamer
        .results
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(old_result.coord, center);
    assert!(old_result.prepared.is_some());
    resident.edit_block(position, Block::Stone);
    streamer.sync_edits(resident.edits());
    let old_token = old_result.token;
    let (finished, results) = std::sync::mpsc::sync_channel(MAX_IN_FLIGHT);
    let real_results = std::mem::replace(&mut streamer.results, results);
    finished.send(old_result).unwrap();
    assert!(streamer.poll_ready(1).is_empty());
    streamer.results = real_results;
    resident.edit_block(position, Block::Wood);
    streamer.sync_edits(resident.edits());
    resident.edit_block(position, Block::Leaves);
    streamer.sync_edits(resident.edits());
    assert!(streamer.residency.in_flight.len() <= MAX_IN_FLIGHT);
    let latest_revision = resident.edits().revision();
    let ready = load_all(&mut streamer);
    assert_eq!(ready.len(), 9);
    assert!(
        ready
            .iter()
            .all(|chunk| chunk.edit_revision == latest_revision)
    );
    let edited = ready.iter().find(|chunk| chunk.coord == center).unwrap();
    assert_eq!(edited.chunk.block(15, position[1], 15), Block::Leaves);
    assert_ne!(edited.request_token, old_token);
    assert!(
        edited
            .mesh
            .opaque
            .vertices
            .iter()
            .any(|vertex| { vertex.position[1] >= position[1] as f32 && vertex.material == 8 })
    );
    assert_eq!(streamer.loaded_count(), 9);
    assert_eq!(streamer.pending_count(), 0);
}

#[test]
fn lantern_worker_fields_follow_source_removal_and_neighbor_halo_revisions() {
    let terrain = Terrain::new(42);
    let center = ChunkCoord::new(-1, -1);
    let mut world = ResidentWorld::default();
    world.insert(Chunk::generate(&terrain, center));
    for x in -5..=-1 {
        for z in -5..=-1 {
            for y in -32..=-28 {
                world.edit_block([x, y, z], if y == -32 { Block::Stone } else { Block::Air });
            }
        }
    }
    world.edit_block([-2, -30, -2], Block::Lantern);
    let mut streamer = ChunkStreamer::new(terrain, center, 1).unwrap();
    streamer.sync_edits(world.edits());
    streamer.update_center(center);
    let initial = load_all(&mut streamer);
    let lit = initial
        .iter()
        .find(|prepared| prepared.coord == center)
        .unwrap();
    assert!(
        lit.mesh
            .opaque
            .vertices
            .iter()
            .any(|vertex| vertex.local_light[0] > 0.0)
    );
    let old = lit.edit_revision;
    world.edit_block([-2, -30, -2], Block::Air);
    streamer.sync_edits(world.edits());
    let replacements = load_all(&mut streamer);
    let unlit = replacements
        .iter()
        .find(|prepared| prepared.coord == center)
        .unwrap();
    assert!(unlit.edit_revision > old);
    assert!(
        unlit
            .mesh
            .opaque
            .vertices
            .iter()
            .all(|vertex| vertex.local_light == [0.0; 3])
    );
    assert_eq!(streamer.loaded_count(), 9);
}

#[test]
fn worker_remeshes_resident_diagonal_halos_and_clear_restores_procedural_chunks() {
    let terrain = Terrain::new(42);
    let center = ChunkCoord::new(0, 0);
    let mut streamer = ChunkStreamer::new(terrain, center, 2).unwrap();
    streamer.update_center(center);
    let initial = load_all(&mut streamer);
    let mut resident = ResidentWorld::default();
    for chunk in initial {
        resident.insert(chunk.chunk);
    }
    let position = [-1, WORLD_HEIGHT - 3, -1];
    assert!(resident.edit_block(position, Block::Wood));
    streamer.sync_edits(resident.edits());
    assert_eq!(streamer.loaded_count(), 25);
    assert_eq!(streamer.pending_count(), 9);
    let replacements = load_all(&mut streamer);
    let expected: BTreeSet<_> = (-2..=0)
        .flat_map(|z| (-2..=0).map(move |x| ChunkCoord::new(x, z)))
        .collect();
    assert_eq!(
        replacements
            .iter()
            .map(|chunk| chunk.coord)
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(replacements.len(), 9);
    for chunk in replacements {
        resident.insert(chunk.chunk);
    }
    let edit_revision = resident.edits().revision();
    resident.clear();
    streamer.sync_edits(resident.edits());
    assert_eq!(streamer.pending_count(), 9);
    let restored = load_all(&mut streamer);
    assert_eq!(restored.len(), 9);
    assert!(
        restored
            .iter()
            .all(|chunk| chunk.edit_revision > edit_revision)
    );
    let chunk = restored
        .iter()
        .find(|chunk| chunk.coord == ChunkCoord::new(-1, -1))
        .unwrap();
    assert_eq!(chunk.chunk.block(15, position[1], 15), Block::Air);
    assert_eq!(streamer.loaded_count(), 25);
}

#[test]
fn shutdown_with_repeated_edit_replacements_keeps_work_bounded() {
    let terrain = Terrain::default();
    let center = ChunkCoord::new(0, 0);
    let mut resident = ResidentWorld::default();
    resident.insert(Chunk::generate(&terrain, center));
    let mut streamer = ChunkStreamer::new(terrain, center, 16).unwrap();
    streamer.update_center(center);
    for block in [Block::Stone, Block::Wood, Block::Leaves, Block::Air] {
        resident.edit_block([15, WORLD_HEIGHT - 2, 15], block);
        streamer.sync_edits(resident.edits());
    }
    assert!(streamer.residency.in_flight.len() <= MAX_IN_FLIGHT);
    assert_eq!(streamer.pending_count(), 33 * 33);
    let started = Instant::now();
    drop(streamer);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn canceled_completions_never_count_as_loaded_and_still_release_their_slot() {
    let center = ChunkCoord::new(0, 0);
    let mut state = Residency::new(center, 1);
    let token = queue(&mut state, center, 0);
    assert!(!state.accept(center, token, 0, false));
    assert!(state.resident.is_empty());
    assert!(state.in_flight.is_empty());
    assert_eq!(state.next_requests()[0], center);
}

#[test]
fn drop_drains_a_full_result_channel_before_joining_a_blocked_worker() {
    use crate::streaming::worker::WorkerResult;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    let center = ChunkCoord::new(0, 0);
    let (finished, results) = mpsc::sync_channel(1);
    finished
        .send(WorkerResult {
            coord: center,
            token: 1,
            prepared: None,
        })
        .unwrap();
    let (started, running) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        finished
            .send(WorkerResult {
                coord: center,
                token: 2,
                prepared: None,
            })
            .unwrap();
    });
    running.recv_timeout(Duration::from_secs(10)).unwrap();
    let streamer = ChunkStreamer {
        residency: Residency::new(center, 1),
        edits: Arc::default(),
        requests: None,
        results,
        stop: Arc::new(AtomicBool::new(false)),
        worker: Some(worker),
        disconnected: false,
    };
    let started = Instant::now();
    drop(streamer);
    assert!(started.elapsed() < Duration::from_secs(10));
}
