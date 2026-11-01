use super::*;

fn index([x, y, z]: [i32; 3], size: [i32; 3]) -> usize {
    ((z * size[1] + y) * size[0] + x) as usize
}

#[test]
fn warm_light_attenuates_to_zero_within_eight_steps_and_solid_walls_occlude() {
    let size = [18, 5, 5];
    let mut blocks = vec![Block::Air; 18 * 5 * 5];
    blocks[index([2, 2, 2], size)] = Block::Lantern;
    let open = LanternField::build([-16, -32, -17], size, &blocks).unwrap();
    let mut previous = 10.0;
    for x in -13..=-7 {
        let value = open.sample([x, -30, -15]);
        assert!(value[0] > value[1] && value[1] > value[2]);
        assert!(value[0] < previous);
        previous = value[0];
    }
    assert_eq!(open.sample([-6, -30, -15]), [0.0; 3]);
    for y in 0..5 {
        for z in 0..5 {
            blocks[index([4, y, z], size)] = Block::Stone;
        }
    }
    let closed = LanternField::build([-16, -32, -17], size, &blocks).unwrap();
    assert_eq!(closed.sample([-11, -30, -15]), [0.0; 3]);
    blocks[index([4, 2, 2], size)] = Block::Air;
    let opening = LanternField::build([-16, -32, -17], size, &blocks).unwrap();
    assert!(opening.sample([-11, -30, -15])[0] > 0.0);
}

#[test]
fn water_attenuates_overlap_uses_max_and_missing_sources_are_exact_zero() {
    let size = [16, 3, 3];
    let mut blocks = vec![Block::Air; 16 * 3 * 3];
    blocks[index([2, 1, 1], size)] = Block::Lantern;
    let first = LanternField::build([0; 3], size, &blocks).unwrap();
    blocks[index([4, 1, 1], size)] = Block::Water;
    let wet = LanternField::build([0; 3], size, &blocks).unwrap();
    assert!(wet.sample([4, 1, 1])[0] < first.sample([4, 1, 1])[0]);
    blocks[index([6, 1, 1], size)] = Block::Lantern;
    let overlap = LanternField::build([0; 3], size, &blocks).unwrap();
    assert!(overlap.sample([5, 1, 1])[0] > wet.sample([5, 1, 1])[0]);
    assert!(overlap.sample([5, 1, 1])[0] <= 1.8);
    blocks.fill(Block::Air);
    let removed = LanternField::build([0; 3], size, &blocks).unwrap();
    assert_eq!(removed.sample([5, 1, 1]), [0.0; 3]);
    assert_eq!(removed.sample([i32::MAX; 3]), [0.0; 3]);
    for bad_size in [[0, 3, 3], [33, 3, 3], [1, WORLD_DEPTH + 1, 1]] {
        assert!(LanternField::build([0; 3], bad_size, &[]).is_err());
    }
    assert!(LanternField::build([i32::MAX; 3], [1; 3], &[Block::Air]).is_err());
    assert!(LanternField::build([0; 3], [3; 3], &[Block::Air]).is_err());
}
