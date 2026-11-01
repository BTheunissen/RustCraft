use super::*;
use crate::world::Block;

#[test]
fn accepted_seed_routes_replay_every_leg_both_directions_without_flight_water_or_edits() {
    for seed in [Terrain::default().seed(), 42, 0, 1, 7, 2026] {
        let routes = survey_coastal_routes(Terrain::new(seed))
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert!(routes.approach.length > 100.0 && routes.approach.length < 1_000.0);
        assert!(routes.descent.length > 10.0 && routes.beach.length > 10.0);
        assert!(routes.surveyed_chunks <= 1_681 && routes.expanded_nodes <= 100_000);
        assert!(routes.replay_steps > 1_000);
        println!(
            "seed={seed}: controller forward/reverse verified, leg lengths={:.1}/{:.1}/{:.1}, rises={}/{}/{}, {} chunks, {} nodes, {} controller updates",
            routes.approach.length,
            routes.descent.length,
            routes.beach.length,
            routes.approach.rises,
            routes.descent.rises,
            routes.beach.rises,
            routes.surveyed_chunks,
            routes.expanded_nodes,
            routes.replay_steps
        );
    }
}

#[test]
fn survey_rejects_invalid_unbounded_and_unsupported_endpoints() {
    let terrain = Terrain::default();
    assert!(Survey::new(terrain, &[]).is_err());
    assert!(Survey::new(terrain, &[[f32::NAN, 0.0, 0.0]]).is_err());
    assert!(Survey::new(terrain, &[[0.0, 20.0, 0.0], [1_000.0, 20.0, 0.0]]).is_err());
    let spawn = terrain.spawn_position();
    let survey = Survey::new(terrain, &[spawn]).unwrap();
    let mut unsupported = spawn;
    unsupported[1] += 10.0;
    assert!(path::find(&survey, unsupported, spawn).is_err());
    assert!(replay::verify(&survey, &route(Vec::new()), false).is_err());
    assert_eq!(survey.block([i32::MAX, 0, i32::MIN]), Block::Stone);
}

#[test]
fn route_edges_require_full_body_clearance_and_reversible_one_block_steps() {
    let terrain = Terrain::default();
    let spawn = terrain.spawn_position();
    let survey = Survey::new(terrain, &[spawn]).unwrap();
    let first = Vec3::from_array(spawn);
    assert!(survey.clear(first, crate::player::PLAYER_HEIGHT));
    assert!(!survey.clear(first - Vec3::Y, crate::player::PLAYER_HEIGHT));
    assert!(!survey.edge(first, first + Vec3::Y * 2.0 + Vec3::X));
    assert!(!survey.edge(first, first - Vec3::Y * 2.0 + Vec3::X));
}

#[test]
fn additional_sampled_seeds_have_reversible_body_clear_controller_verified_coasts() {
    for seed in [2, 3, 8, 17, 99, 12345, u64::MAX] {
        let routes = survey_coastal_routes(Terrain::new(seed))
            .unwrap_or_else(|error| panic!("sampled seed {seed}: {error}"));
        println!(
            "sampled seed={seed}: route lengths {:.1}/{:.1}/{:.1}, chunks={}, nodes={}, controller updates={}",
            routes.approach.length,
            routes.descent.length,
            routes.beach.length,
            routes.surveyed_chunks,
            routes.expanded_nodes,
            routes.replay_steps
        );
    }
}

#[test]
fn actual_routes_have_support_full_width_headroom_and_strict_replay_failures() {
    let terrain = Terrain::default();
    let site = terrain.coastal_landmark().unwrap();
    let routes = survey_coastal_routes(terrain).unwrap();
    let survey = Survey::new(
        terrain,
        &[
            terrain.spawn_position(),
            site.viewpoint,
            site.cove,
            site.arch.view,
        ],
    )
    .unwrap();
    for route in [&routes.approach, &routes.descent, &routes.beach] {
        for point in &route.feet {
            let feet = Vec3::from_array(*point);
            let [x, y, z] = feet.floor().as_ivec3().to_array();
            assert!(survey.block([x, y - 1, z]).is_solid());
            assert!(survey.clear(feet, crate::player::PLAYER_HEIGHT));
            assert!(survey.water([x, y, z]).is_none());
        }
        for pair in route.feet.windows(2) {
            assert!(survey.edge(Vec3::from_array(pair[0]), Vec3::from_array(pair[1])));
            assert!(survey.edge(Vec3::from_array(pair[1]), Vec3::from_array(pair[0])));
        }
    }
    assert!(!survey.clear(Vec3::NAN, 1.8));
    assert!(!survey.clear(Vec3::from_array(site.cove), f32::NAN));
    let mut invalid = routes.descent.clone();
    invalid.feet[1][1] += 10.0;
    assert!(replay::verify(&survey, &invalid, false).is_err());
}
