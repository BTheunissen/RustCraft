//! A* over reversible, fully cleared cardinal surface steps.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use super::survey::Survey;
use super::{CoastalRoute, route};

pub(super) const MAX_NODES: usize = 100_000;
const MAX_POINTS: usize = 4_096;

pub(super) fn find(
    survey: &Survey,
    first: [f32; 3],
    last: [f32; 3],
) -> Result<(CoastalRoute, usize), String> {
    let start = [first[0].floor() as i32, first[2].floor() as i32];
    let goal = [last[0].floor() as i32, last[2].floor() as i32];
    let Some(start_feet) = survey.feet(start) else {
        return Err(format!("unsupported route start {first:?}"));
    };
    let Some(goal_feet) = survey.feet(goal) else {
        return Err(format!("unsupported route goal {last:?}"));
    };
    if (start_feet.y - first[1]).abs() > 0.01 || (goal_feet.y - last[1]).abs() > 0.01 {
        return Err("route endpoint does not match actual ground".into());
    }
    let heuristic = |[x, z]: [i32; 2]| x.abs_diff(goal[0]) + z.abs_diff(goal[1]);
    let mut open = BinaryHeap::from([Reverse((heuristic(start), 0_u32, start))]);
    let mut best = BTreeMap::from([(start, 0_u32)]);
    let mut parents = BTreeMap::new();
    let mut expanded = 0;
    while let Some(Reverse((_, cost, point))) = open.pop() {
        if best.get(&point) != Some(&cost) {
            continue;
        }
        expanded += 1;
        if expanded > MAX_NODES {
            return Err("coastal route exceeded node budget".into());
        }
        if point == goal {
            let mut points = vec![goal_feet.to_array()];
            let mut point = goal;
            while point != start {
                point = parents[&point];
                points.push(
                    survey
                        .feet(point)
                        .ok_or("route support disappeared")?
                        .to_array(),
                );
                if points.len() > MAX_POINTS {
                    return Err("coastal route exceeded point budget".into());
                }
            }
            points.reverse();
            return Ok((route(points), expanded));
        }
        let feet = survey.feet(point).ok_or("route support disappeared")?;
        for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
            let (Some(x), Some(z)) = (point[0].checked_add(dx), point[1].checked_add(dz)) else {
                continue;
            };
            let next = [x, z];
            let Some(target) = survey.feet(next) else {
                continue;
            };
            if !survey.edge(feet, target) {
                continue;
            }
            let cost = cost + 1 + u32::from(feet.y != target.y) * 2;
            if best.get(&next).is_none_or(|&old| cost < old) {
                best.insert(next, cost);
                parents.insert(next, point);
                open.push(Reverse((cost + heuristic(next), cost, next)));
            }
        }
    }
    Err(format!(
        "no reversible walking path from {first:?} to {last:?} ({expanded} nodes)"
    ))
}
