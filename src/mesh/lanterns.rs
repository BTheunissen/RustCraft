//! Prepared only when sparse nearby lantern edits exist; all terrain work is worker-side.

use crate::lighting::lantern::{LANTERN_REACH, LanternField};
use crate::world::{Block, CHUNK_SIZE, MIN_WORLD_Y, Terrain, WORLD_DEPTH, WorldEdits};

pub(super) fn prepare(
    origin: [i32; 2],
    terrain: &Terrain,
    edits: Option<&WorldEdits>,
) -> Option<LanternField> {
    let edits = edits?;
    let sources = edits.lantern_sources(origin);
    if sources.is_empty() {
        return None;
    }
    let first = [
        origin[0].checked_sub(LANTERN_REACH)?,
        origin[1].checked_sub(LANTERN_REACH)?,
    ];
    let width = CHUNK_SIZE + LANTERN_REACH * 2;
    let columns: Option<Vec<_>> = (0..width)
        .flat_map(|z| (0..width).map(move |x| [x, z]))
        .map(|[x, z]| Some([first[0].checked_add(x)?, first[1].checked_add(z)?]))
        .collect();
    let columns = columns?;
    let mut occupancy = vec![Block::Air; (width * width * WORLD_DEPTH) as usize];
    for (index, (&position, mut blocks)) in columns
        .iter()
        .zip(terrain.column_block_batch(&columns))
        .enumerate()
    {
        edits.apply_to_column(position, &mut blocks);
        let x = index % width as usize;
        let z = index / width as usize;
        for (y, block) in blocks.into_iter().enumerate() {
            occupancy[(z * WORLD_DEPTH as usize + y) * width as usize + x] = block;
        }
    }
    LanternField::build(
        [first[0], MIN_WORLD_Y, first[1]],
        [width, WORLD_DEPTH, width],
        &occupancy,
    )
    .ok()
}

#[cfg(test)]
mod tests;
