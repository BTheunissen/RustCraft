use super::{ChunkStreamer, MAX_IN_FLIGHT, Residency};
use crate::world::{ChunkCoord, Terrain};

mod edits;

fn queue(state: &mut Residency, coord: ChunkCoord, halo_revision: u64) -> u64 {
    let stamp = state.request_stamp(halo_revision);
    let token = stamp.token;
    state.in_flight.insert(coord, stamp);
    token
}

#[test]
fn requests_nearest_first_and_bounds_the_queue() {
    let state = Residency::new(ChunkCoord::new(-3, 2), 4);
    let requests = state.next_requests();
    assert_eq!(requests.len(), MAX_IN_FLIGHT);
    assert_eq!(requests[0], ChunkCoord::new(-3, 2));
    assert!(
        requests[1..5]
            .iter()
            .all(|coord| { (coord.x + 3).abs() + (coord.z - 2).abs() == 1 })
    );
}

#[test]
fn excludes_resident_and_in_flight_chunks() {
    let center = ChunkCoord::new(0, 0);
    let mut state = Residency::new(center, 2);
    queue(&mut state, center, 0);
    let neighbor = ChunkCoord::new(1, 0);
    state.resident.insert(neighbor);
    let requests = state.next_requests();
    assert_eq!(requests.len(), MAX_IN_FLIGHT - 1);
    assert!(!requests.contains(&center));
    assert!(!requests.contains(&neighbor));
}

#[test]
fn accepts_current_results_once_and_discards_stale_results() {
    let center = ChunkCoord::new(-1, -1);
    let mut state = Residency::new(center, 1);
    let token = queue(&mut state, center, 0);
    assert!(state.accept(center, token, 0, true));
    assert!(!state.accept(center, token, 0, true));
    assert!(state.in_flight.is_empty());
    let stale = ChunkCoord::new(100, 100);
    let token = queue(&mut state, stale, 0);
    assert!(!state.accept(stale, token, 0, true));
    assert!(state.in_flight.is_empty());
    assert_eq!(state.resident.len(), 1);
}

#[test]
fn recenter_evicts_only_outside_the_new_area() {
    let mut state = Residency::new(ChunkCoord::new(0, 0), 1);
    state.resident.extend(state.desired.iter().copied());
    let evicted = state.recenter(ChunkCoord::new(1, 0));
    assert_eq!(evicted.len(), 3);
    assert!(evicted.iter().all(|coord| coord.x == -1));
    assert_eq!(state.resident.len(), 6);
    assert_eq!(state.desired.len(), 9);
    assert!(state.recenter(ChunkCoord::new(1, 0)).is_empty());
}

#[test]
fn representable_chunk_edges_do_not_wrap_residency_or_priorities() {
    for center in [
        ChunkCoord::new(i32::MAX, i32::MAX),
        ChunkCoord::new(i32::MIN, i32::MIN),
    ] {
        let state = Residency::new(center, 1);
        assert_eq!(state.desired.len(), 4);
        assert_eq!(state.next_requests()[0], center);
    }
}

#[test]
fn rapid_travel_does_not_expand_in_flight_work() {
    let mut state = Residency::new(ChunkCoord::new(0, 0), 4);
    for coord in state.next_requests() {
        queue(&mut state, coord, 0);
    }
    state.recenter(ChunkCoord::new(100, -100));
    assert!(state.next_requests().is_empty());
    let (&old, stamp) = state.in_flight.iter().next().unwrap();
    let token = stamp.token;
    assert!(!state.accept(old, token, 0, true));
    assert_eq!(state.next_requests().len(), 1);
    assert_eq!(state.resident.len(), 0);
}

#[test]
fn worker_generates_real_meshes_and_shuts_down_with_queued_work() {
    let center = ChunkCoord::new(-1, 0);
    let mut streamer = ChunkStreamer::new(Terrain::default(), center, 2).unwrap();
    assert_eq!(streamer.pending_count(), 25);
    assert!(streamer.update_center(center).is_empty());
    let result = streamer
        .results
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let chunk = result.prepared.unwrap();
    assert_eq!(chunk.coord, center);
    assert_eq!(chunk.chunk.coord(), center);
    assert_eq!(
        chunk.chunk.block(0, crate::world::MIN_WORLD_Y, 0),
        crate::world::Block::Bedrock
    );
    assert!(!chunk.mesh.opaque.indices.is_empty());
    assert!(
        streamer
            .residency
            .accept(chunk.coord, result.token, 0, true)
    );
    assert_eq!(streamer.loaded_count(), 1);
    assert_eq!(streamer.pending_count(), 24);
    assert!(!streamer.worker_failed());
    drop(streamer);
}

#[test]
fn polling_uploads_each_desired_chunk_once_and_recenter_evicts_it() {
    let center = ChunkCoord::new(0, 0);
    let mut streamer = ChunkStreamer::new(Terrain::default(), center, 1).unwrap();
    streamer.update_center(center);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut ready = Vec::new();
    while streamer.pending_count() > 0 {
        ready.extend(streamer.poll_ready(2));
        assert!(!streamer.worker_failed());
        assert!(std::time::Instant::now() < deadline, "worker stalled");
        std::thread::yield_now();
    }
    assert_eq!(ready.len(), 9);
    assert_eq!(streamer.loaded_count(), 9);
    let coordinates: std::collections::HashSet<_> = ready.iter().map(|chunk| chunk.coord).collect();
    assert_eq!(coordinates.len(), 9);
    assert!(streamer.poll_ready(2).is_empty());
    assert_eq!(streamer.update_center(ChunkCoord::new(10, -10)).len(), 9);
    assert_eq!(streamer.loaded_count(), 0);
    assert_eq!(streamer.pending_count(), 9);
}
