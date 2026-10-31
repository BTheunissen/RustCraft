//! Block and chunk coordinate conventions.

/// Number of blocks along each axis of a chunk.
pub const CHUNK_SIZE: i32 = 16;

/// Splits a world block coordinate into its chunk coordinate and local offset.
///
/// The local offset is always in `0..CHUNK_SIZE`. Negative world coordinates
/// use floor division so that blocks immediately below zero belong to chunk -1.
///
/// ```
/// use rustcraft::world::split_block_coordinate;
///
/// assert_eq!(split_block_coordinate(16), (1, 0));
/// assert_eq!(split_block_coordinate(-1), (-1, 15));
/// ```
pub fn split_block_coordinate(coordinate: i32) -> (i32, i32) {
    (
        coordinate.div_euclid(CHUNK_SIZE),
        coordinate.rem_euclid(CHUNK_SIZE),
    )
}

#[cfg(test)]
mod tests {
    use super::{CHUNK_SIZE, split_block_coordinate};

    #[test]
    fn splits_non_negative_coordinates_at_chunk_boundaries() {
        for (coordinate, expected) in [
            (0, (0, 0)),
            (1, (0, 1)),
            (15, (0, 15)),
            (16, (1, 0)),
            (17, (1, 1)),
            (31, (1, 15)),
            (32, (2, 0)),
            (33, (2, 1)),
        ] {
            assert_eq!(
                split_block_coordinate(coordinate),
                expected,
                "world coordinate: {coordinate}"
            );
        }
    }

    #[test]
    fn splits_negative_coordinates_at_chunk_boundaries() {
        for (coordinate, expected) in [
            (-1, (-1, 15)),
            (-15, (-1, 1)),
            (-16, (-1, 0)),
            (-17, (-2, 15)),
            (-31, (-2, 1)),
            (-32, (-2, 0)),
            (-33, (-3, 15)),
        ] {
            assert_eq!(
                split_block_coordinate(coordinate),
                expected,
                "world coordinate: {coordinate}"
            );
        }
    }

    #[test]
    fn supports_extreme_world_coordinates() {
        assert_eq!(split_block_coordinate(i32::MIN), (-134_217_728, 0));
        assert_eq!(split_block_coordinate(i32::MAX), (134_217_727, 15));
    }

    #[test]
    fn reconstructs_world_coordinates_with_bounded_local_offsets() {
        for coordinate in -4_096..=4_096 {
            let (chunk, local) = split_block_coordinate(coordinate);

            assert!(
                (0..CHUNK_SIZE).contains(&local),
                "world coordinate: {coordinate}, local offset: {local}"
            );
            assert_eq!(
                chunk * CHUNK_SIZE + local,
                coordinate,
                "world coordinate: {coordinate}"
            );
        }
    }
}
