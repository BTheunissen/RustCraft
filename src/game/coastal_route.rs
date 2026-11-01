//! Bounded diagnostic walking surveys; never invoked by interactive gameplay.

use glam::Vec3;

use crate::world::Terrain;

mod path;
mod replay;
mod survey;
#[cfg(test)]
mod tests;

use survey::Survey;

/// A supported cardinal walking route; feet positions are diagnostics, not HUD markers.
#[derive(Clone, Debug)]
pub struct CoastalRoute {
    /// Centered feet, including the supported height, in forward traversal order.
    pub feet: Vec<[f32; 3]>,
    /// Sum of actual three-dimensional step lengths, not straight-line distance.
    pub length: f32,
    /// Reversible one-block height changes; uphill traversal uses ordinary jumping.
    pub rises: usize,
}

/// Three reversible legs from the ordinary spawn to the view, cove, and arch shore.
#[derive(Clone, Debug)]
pub struct CoastalRoutes {
    /// Ordinary fresh spawn to the cliff viewpoint.
    pub approach: CoastalRoute,
    /// Cliff viewpoint to the dry cove, including its natural ramp.
    pub descent: CoastalRoute,
    /// Cove to the dry arch shore view.
    pub beach: CoastalRoute,
    /// Transient CPU chunks visited by sampling/replay, never saved or rendered here.
    pub surveyed_chunks: usize,
    /// A* expansions across the three bounded legs.
    pub expanded_nodes: usize,
    /// Actual controller updates for every leg forward and reverse.
    pub replay_steps: usize,
}

/// Surveys actual voxels and replays the complete route with walking/jumping controls.
/// Fails rather than repairing terrain or using flight, edits, swimming, or respawn.
/// Work is limited to a 640×640 horizontal region, 1,681 chunks, and 100,000 nodes.
pub fn survey_coastal_routes(terrain: Terrain) -> Result<CoastalRoutes, String> {
    let site = terrain.coastal_landmark().ok_or("no clear early coast")?;
    let spawn = terrain.spawn_position();
    let survey = Survey::new(terrain, &[spawn, site.viewpoint, site.cove, site.arch.view])?;
    let (approach, first) = path::find(&survey, spawn, site.viewpoint)?;
    let (descent, second) = path::find(&survey, site.viewpoint, site.cove)?;
    let (beach, third) = path::find(&survey, site.cove, site.arch.view)?;
    let expanded_nodes = first + second + third;
    if expanded_nodes > path::MAX_NODES {
        return Err("coastal survey exceeded total node budget".into());
    }
    let mut replay_steps = 0;
    for route in [&approach, &descent, &beach] {
        replay_steps += replay::verify(&survey, route, false)?;
        replay_steps += replay::verify(&survey, route, true)?;
    }
    Ok(CoastalRoutes {
        approach,
        descent,
        beach,
        surveyed_chunks: survey.chunk_count(),
        expanded_nodes,
        replay_steps,
    })
}

fn route(points: Vec<[f32; 3]>) -> CoastalRoute {
    let length = points
        .windows(2)
        .map(|pair| Vec3::from_array(pair[0]).distance(Vec3::from_array(pair[1])))
        .sum();
    let rises = points
        .windows(2)
        .filter(|pair| pair[0][1] != pair[1][1])
        .count();
    CoastalRoute {
        feet: points,
        length,
        rises,
    }
}
