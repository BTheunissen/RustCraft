//! Cave cross sections, loops, and retained rock.

use super::*;

#[derive(Debug, Default)]
pub(super) struct Morphology {
    pub(super) sampled_columns: usize,
    pub(super) connected_columns: usize,
    pub(super) headroom_min: usize,
    pub(super) headroom_max: usize,
    pub(super) headroom_quantiles: [usize; 3],
    pub(super) tall_columns: usize,
    pub(super) width_min: usize,
    pub(super) width_max: usize,
    pub(super) width_quantiles: [usize; 3],
    pub(super) aspect_min: f32,
    pub(super) aspect_max: f32,
    pub(super) aspect_quantiles: [f32; 3],
    pub(super) split_columns: usize,
    pub(super) split_rows: usize,
    pub(super) narrow_cells: usize,
    pub(super) wide_cells: usize,
    pub(super) junction_cells: usize,
    pub(super) corridor_loops: usize,
    pub(super) features: Features,
}

impl Morphology {
    pub(super) fn inspect(
        feet: Vec3,
        connected: &HashSet<[i32; 3]>,
        block: impl Fn([i32; 3]) -> Block,
    ) -> Self {
        let origin = feet.to_array().map(|coordinate| coordinate.floor() as i32);
        let min = [origin[0] - GEOMETRY_RADIUS, origin[2] - GEOMETRY_RADIUS];
        let max = [origin[0] + GEOMETRY_RADIUS, origin[2] + GEOMETRY_RADIUS];
        let mut result = Self {
            headroom_min: usize::MAX,
            width_min: usize::MAX,
            aspect_min: f32::INFINITY,
            ..Self::default()
        };
        let mut heights = Vec::new();
        let mut widths = Vec::new();
        let mut aspects = Vec::new();
        for z in (min[1]..=max[1]).step_by(2) {
            for x in (min[0]..=max[0]).step_by(2) {
                result.sampled_columns += 1;
                let intervals =
                    air_intervals(MIN_WORLD_Y + 1..0, |y| block([x, y, z]) == Block::Air);
                let local: Vec<_> = intervals
                    .into_iter()
                    .filter(|&(first, last)| {
                        last > first && (first..last).any(|y| connected.contains(&[x, y, z]))
                    })
                    .collect();
                result.connected_columns += usize::from(!local.is_empty());
                result.split_columns += usize::from(local.len() > 1);
                for (first, last) in local {
                    let height = (last - first + 1) as usize;
                    let middle = (first + last) / 2;
                    let width = [0, 2]
                        .into_iter()
                        .map(|axis| air_run([x, middle, z], axis, GEOMETRY_RADIUS, &block))
                        .min()
                        .unwrap_or(0);
                    let aspect = height as f32 / width.max(1) as f32;
                    heights.push(height);
                    widths.push(width);
                    aspects.push(aspect);
                    result.headroom_min = result.headroom_min.min(height);
                    result.headroom_max = result.headroom_max.max(height);
                    result.tall_columns += usize::from(height >= 20);
                    result.width_min = result.width_min.min(width);
                    result.width_max = result.width_max.max(width);
                    result.aspect_min = result.aspect_min.min(aspect);
                    result.aspect_max = result.aspect_max.max(aspect);
                }
            }
        }
        let y = origin[1] + 1;
        let passable = |[x, z]: [i32; 2]| {
            connected.contains(&[x, y, z])
                && block([x, y, z]) == Block::Air
                && block([x, y + 1, z]) == Block::Air
        };
        let mut narrow = HashSet::new();
        for z in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                if !passable([x, z]) {
                    continue;
                }
                let width = [0, 2]
                    .into_iter()
                    .map(|axis| air_run([x, y, z], axis, GEOMETRY_RADIUS, &block))
                    .min()
                    .unwrap_or(0);
                result.narrow_cells += usize::from(width <= 10);
                if width <= 10 {
                    narrow.insert([x, z]);
                }
                result.wide_cells += usize::from(width >= 17);
                let branches = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                    .into_iter()
                    .filter(|[dx, dz]| (1..=6).all(|step| passable([x + dx * step, z + dz * step])))
                    .count();
                result.junction_cells += usize::from(width < 17 && branches >= 3);
            }
            let intervals = air_intervals(min[0]..max[0] + 1, |x| passable([x, z]));
            result.split_rows +=
                usize::from(intervals.iter().filter(|(a, b)| b - a >= 2).count() > 1);
        }
        result.corridor_loops =
            enclosed_rock_islands(min, max, passable, |point| narrow.contains(&point));
        for z in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                for y in MIN_WORLD_Y + 1..0 {
                    let point = [x, y, z];
                    let material = block(point);
                    if material == Block::Air {
                        continue;
                    }
                    let exposed = NEIGHBORS.into_iter().any(|offset| {
                        let next = std::array::from_fn(|axis| point[axis] + offset[axis]);
                        connected.contains(&next) && block(next) == Block::Air
                    });
                    if !exposed {
                        continue;
                    }
                    result.features.record(point, material, &block);
                }
            }
        }
        if result.headroom_min == usize::MAX {
            result.headroom_min = 0;
            result.width_min = 0;
            result.aspect_min = 0.0;
        } else {
            heights.sort_unstable();
            widths.sort_unstable();
            aspects.sort_by(f32::total_cmp);
            result.headroom_quantiles =
                [10, 50, 90].map(|percent| heights[(heights.len() - 1) * percent / 100]);
            result.width_quantiles =
                [10, 50, 90].map(|percent| widths[(widths.len() - 1) * percent / 100]);
            result.aspect_quantiles =
                [10, 50, 90].map(|percent| aspects[(aspects.len() - 1) * percent / 100]);
        }
        result
    }

    pub(super) fn report(&self) {
        println!(
            "  observed geometry ({}-block radius): {} connected/{} sampled columns; air height={}..{}, tall(>=20)={}, cross-width={}..{}, height/width={:.2}..{:.2}; split vertical columns={}, split horizontal rows={}",
            GEOMETRY_RADIUS,
            self.connected_columns,
            self.sampled_columns,
            self.headroom_min,
            self.headroom_max,
            self.tall_columns,
            self.width_min,
            self.width_max,
            self.aspect_min,
            self.aspect_max,
            self.split_columns,
            self.split_rows,
        );
        println!(
            "  section p10/p50/p90: height={:?}, width={:?}, height/width=[{:.2}, {:.2}, {:.2}]",
            self.headroom_quantiles,
            self.width_quantiles,
            self.aspect_quantiles[0],
            self.aspect_quantiles[1],
            self.aspect_quantiles[2],
        );
        println!(
            "  horizontal slice: narrow(<=10)={}, wide(>=17)={}, junction samples={}, rock islands with narrow-corridor loops={}",
            self.narrow_cells, self.wide_cells, self.junction_cells, self.corridor_loops,
        );
        self.features.report("connected");
    }
}

pub(super) fn air_intervals(
    range: std::ops::Range<i32>,
    mut air: impl FnMut(i32) -> bool,
) -> Vec<(i32, i32)> {
    let mut result = Vec::new();
    let mut first = None;
    for coordinate in range.clone() {
        if air(coordinate) {
            first.get_or_insert(coordinate);
        } else if let Some(start) = first.take() {
            result.push((start, coordinate - 1));
        }
    }
    if let Some(start) = first {
        result.push((start, range.end - 1));
    }
    result
}

pub(super) fn air_run(
    point: [i32; 3],
    axis: usize,
    limit: i32,
    block: impl Fn([i32; 3]) -> Block,
) -> usize {
    if block(point) != Block::Air {
        return 0;
    }
    1 + [-1, 1]
        .into_iter()
        .map(|direction| {
            (1..=limit)
                .take_while(|&distance| {
                    let mut next = point;
                    next[axis] += distance * direction;
                    block(next) == Block::Air
                })
                .count()
        })
        .sum::<usize>()
}

pub(super) fn enclosed_rock_islands(
    min: [i32; 2],
    max: [i32; 2],
    air: impl Fn([i32; 2]) -> bool,
    narrow: impl Fn([i32; 2]) -> bool,
) -> usize {
    let mut seen = HashSet::new();
    let mut islands = 0;
    for z in min[1]..=max[1] {
        for x in min[0]..=max[0] {
            let root = [x, z];
            if air(root) || !seen.insert(root) {
                continue;
            }
            let mut queue = VecDeque::from([root]);
            let mut cells = 0;
            let mut boundary = false;
            let mut rim = HashSet::new();
            while let Some(point) = queue.pop_front() {
                cells += 1;
                boundary |= (0..2).any(|axis| point[axis] == min[axis] || point[axis] == max[axis]);
                for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    let next = [point[0] + dx, point[1] + dz];
                    if !(0..2).all(|axis| (min[axis]..=max[axis]).contains(&next[axis])) {
                        continue;
                    }
                    if air(next) {
                        rim.insert(next);
                    } else if seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            islands += usize::from(
                !boundary
                    && cells >= 9
                    && rim.len() >= 12
                    && rim.iter().filter(|&&point| narrow(point)).count() * 2 >= rim.len(),
            );
        }
    }
    islands
}
