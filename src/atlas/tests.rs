use std::collections::HashSet;

use super::{Atlas, TILE_COUNT, TILE_SIZE, generate_atlas};

fn pixel(atlas: &Atlas, tile: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * atlas.width + tile * TILE_SIZE + x) * 4) as usize;
    atlas.pixels[index..index + 4].try_into().unwrap()
}

fn tile_pixels(atlas: &Atlas, tile: u32) -> Vec<[u8; 4]> {
    (0..TILE_SIZE)
        .flat_map(|y| (0..TILE_SIZE).map(move |x| pixel(atlas, tile, x, y)))
        .collect()
}

fn tile_checksum(atlas: &Atlas, tile: u32) -> u64 {
    tile_pixels(atlas, tile)
        .into_iter()
        .flatten()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

#[test]
fn packs_processed_building_tiles_into_one_rgba_row() {
    let atlas = generate_atlas();
    assert_eq!((atlas.width, atlas.height), (448, 16));
    assert_eq!(atlas.pixels.len(), 448 * 16 * 4);
    assert_eq!(atlas.width, TILE_SIZE * TILE_COUNT);
    let (pixels, remainder) = atlas.pixels.as_chunks::<4>();
    assert!(remainder.is_empty());
    assert!(pixels.iter().all(|pixel| pixel[3] == 255));
}

#[test]
fn generation_is_repeatable() {
    assert_eq!(generate_atlas(), generate_atlas());
}

#[test]
fn all_twenty_two_previous_tiles_are_byte_for_byte_unchanged() {
    let atlas = generate_atlas();
    for (tile, expected) in [
        0x39fe_cba3_65bb_c50d,
        0x6a0f_cf5f_966e_015f,
        0x5c3c_3c3d_1419_667b,
        0xf68e_341e_ac35_3591,
        0xde5d_3077_d633_995e,
        0xa99c_38ef_c2ae_30a2,
        0x268b_4bf1_63fe_d1bc,
        0x5aa3_8a20_8b97_2427,
        0x58bd_9303_e679_448b,
        0x691a_33e1_6cb3_ac99,
        0xcd43_f60c_0c64_5fe3,
        0xe576_621b_cc62_6b7d,
        0x331f_088d_1438_c103,
        0xcab9_5628_8452_8361,
        0xe24d_9e3e_6e28_d6c5,
        0x752c_70a7_1512_c820,
        0x8676_f6e3_503d_5f89,
        0x0132_ac35_9bd9_dd24,
        0xa9f4_f37b_8ee1_f796,
        0x8d85_237c_5ff9_9840,
        0x858b_cda7_08ce_15ae,
        0xe84b_63e0_b022_8a0f,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            tile_checksum(&atlas, tile as u32),
            expected,
            "existing tile {tile} changed"
        );
    }
}

#[test]
fn gravel_and_clay_keep_their_original_art_checksums() {
    let atlas = generate_atlas();
    assert_eq!(tile_checksum(&atlas, 22), 0x7e72_5e22_31f7_bd6a);
    assert_eq!(tile_checksum(&atlas, 23), 0xd97e_a68d_8e74_55eb);
}

#[test]
fn processed_materials_have_original_regular_seams_and_differ_from_raw_sources() {
    let atlas = generate_atlas();
    assert_ne!(tile_pixels(&atlas, 24), tile_pixels(&atlas, 6));
    assert_ne!(tile_pixels(&atlas, 25), tile_pixels(&atlas, 18));
    for x in 0..16 {
        let seam = pixel(&atlas, 24, x, 4);
        let plank = pixel(&atlas, 24, x, 5);
        assert!(seam[0] < plank[0] || (x + 5) % 16 <= 1);
        let joint = pixel(&atlas, 25, x, 8);
        let dressed = pixel(&atlas, 25, x, 10);
        assert!(joint[0] < dressed[0] || (x + 8).is_multiple_of(16));
    }
}

#[test]
fn every_material_has_distinct_non_flat_art() {
    let atlas = generate_atlas();
    let tiles: Vec<_> = (0..TILE_COUNT)
        .map(|tile| tile_pixels(&atlas, tile))
        .collect();
    for (index, tile) in tiles.iter().enumerate() {
        assert!(tile.iter().copied().collect::<HashSet<_>>().len() >= 12);
        for other in &tiles[index + 1..] {
            assert_ne!(tile, other, "tile {index} duplicates another material");
        }
    }
}

#[test]
fn grass_side_has_a_green_upper_fringe_over_matching_dirt() {
    let atlas = generate_atlas();
    let mut fringe_depths = HashSet::new();
    for x in 0..TILE_SIZE {
        let top = pixel(&atlas, 1, x, 0);
        assert!(top[1] > top[0] && top[1] > top[2]);
        let mut fringe_depth = 0;
        for y in 0..TILE_SIZE {
            let side = pixel(&atlas, 1, x, y);
            if side[1] > side[0] {
                fringe_depth += 1;
            } else {
                assert_eq!(side, pixel(&atlas, 2, x, y));
                assert!(side[0] > side[1] && side[1] > side[2]);
            }
        }
        assert!((3..=5).contains(&fringe_depth));
        fringe_depths.insert(fringe_depth);
    }
    assert!(fringe_depths.len() > 1);
}

#[test]
fn snow_sand_leaves_and_water_keep_their_material_palettes() {
    let atlas = generate_atlas();
    for y in 0..TILE_SIZE {
        for x in 0..TILE_SIZE {
            let sand = pixel(&atlas, 4, x, y);
            assert!(sand[0] > sand[1] && sand[1] > sand[2]);
            let snow = pixel(&atlas, 5, x, y);
            assert!(snow[..3].iter().all(|channel| *channel >= 190));
            let leaves = pixel(&atlas, 8, x, y);
            assert!(leaves[1] > leaves[0] && leaves[1] > leaves[2]);
            let water = pixel(&atlas, 9, x, y);
            assert!(water[2] > water[1] && water[1] > water[0]);
        }
    }
}

#[test]
fn bedrock_and_deep_stone_are_dark_distinct_slate_variants() {
    let atlas = generate_atlas();
    let mut bedrock_red = 0;
    let mut deep_stone_red = 0;
    for y in 0..TILE_SIZE {
        for x in 0..TILE_SIZE {
            let bedrock = pixel(&atlas, 10, x, y);
            assert!(bedrock[..3].iter().all(|channel| *channel < 70));
            assert!(bedrock[2] > bedrock[1] && bedrock[1] > bedrock[0]);
            let deep_stone = pixel(&atlas, 11, x, y);
            assert!(deep_stone[2] > deep_stone[1] && deep_stone[1] > deep_stone[0]);
            assert!(deep_stone[2] < 110);
            bedrock_red += u32::from(bedrock[0]);
            deep_stone_red += u32::from(deep_stone[0]);
        }
    }
    assert!(deep_stone_red > bedrock_red + TILE_SIZE * TILE_SIZE * 16);
}

#[test]
fn ores_form_clustered_veins_in_matching_deep_stone() {
    let atlas = generate_atlas();
    let host = tile_pixels(&atlas, 11);
    let mut masks = Vec::new();
    for tile in 12..=14 {
        let pixels = tile_pixels(&atlas, tile);
        let vein: Vec<_> = pixels.iter().zip(&host).map(|(a, b)| a != b).collect();
        let vein_pixels = vein.iter().filter(|vein| **vein).count();
        assert!((32..128).contains(&vein_pixels), "ore tile {tile}");
        let mut connected = 0;
        for y in 0..TILE_SIZE as usize {
            for x in 0..TILE_SIZE as usize {
                let index = y * TILE_SIZE as usize + x;
                if !vein[index] {
                    continue;
                }
                connected += usize::from(
                    (x > 0 && vein[index - 1])
                        || (x + 1 < TILE_SIZE as usize && vein[index + 1])
                        || (y > 0 && vein[index - TILE_SIZE as usize])
                        || (y + 1 < TILE_SIZE as usize && vein[index + TILE_SIZE as usize]),
                );
                let [r, g, b, _] = pixels[index];
                if tile == 12 {
                    assert!(r < 40 && g < 40 && b < 50);
                } else {
                    assert!(r > g && g > b);
                    if tile == 14 {
                        assert!(u16::from(g) > u16::from(b) * 2);
                    } else {
                        assert!(b >= 70);
                    }
                }
            }
        }
        assert!(connected * 4 >= vein_pixels * 3, "ore tile {tile}");
        masks.push(vein);
    }
    assert_ne!(masks[0], masks[1]);
    assert_ne!(masks[1], masks[2]);
}

#[test]
fn crystal_facets_mix_cyan_violet_and_bright_glints() {
    let atlas = generate_atlas();
    let pixels = tile_pixels(&atlas, 15);
    let cyan = pixels
        .iter()
        .filter(|p| i32::from(p[1]) - i32::from(p[0]) > 30 && p[2] > p[1])
        .count();
    let violet = pixels
        .iter()
        .filter(|p| i32::from(p[0]) - i32::from(p[1]) > 20 && p[2] > p[0])
        .count();
    let glints = pixels.iter().filter(|p| p[1] > 230 && p[2] > 240).count();
    let rock = pixels
        .iter()
        .filter(|p| p[..3].iter().all(|c| *c < 70))
        .count();
    assert!(cyan >= 24 && violet >= 16 && glints >= 8 && rock >= 128);
}

#[test]
fn moss_is_muted_green_and_lava_has_hot_cracks_in_dark_crust() {
    let atlas = generate_atlas();
    for moss in tile_pixels(&atlas, 16) {
        assert!(moss[1] > moss[0] && moss[1] > moss[2]);
        assert!(moss[1] < 140);
    }
    let lava = tile_pixels(&atlas, 17);
    assert!(lava.iter().all(|p| p[0] > p[1] && p[1] > p[2]));
    assert!(lava.iter().filter(|p| p[0] > 240 && p[1] > 180).count() >= 32);
    assert!(
        lava.iter()
            .filter(|p| p[0] > 230 && (90..140).contains(&p[1]))
            .count()
            >= 32
    );
    assert!(lava.iter().filter(|p| p[0] < 100 && p[1] < 45).count() >= 64);
}

#[test]
fn geology_tiles_have_warm_and_cool_palettes_with_visible_relief() {
    let atlas = generate_atlas();
    for tile in 18..=21 {
        let pixels = tile_pixels(&atlas, tile);
        for &[r, g, b, _] in &pixels {
            if tile == 19 {
                assert!(b > g && g > r);
                assert!(g - r >= 14);
            } else {
                assert!(r > g && g > b, "geology tile {tile}");
                if tile == 20 {
                    assert!(b >= 20 && r < 110);
                }
            }
        }
        let brightness: Vec<_> = pixels
            .iter()
            .map(|p| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]))
            .collect();
        assert!(
            brightness.iter().max().unwrap() - brightness.iter().min().unwrap() >= 150,
            "geology tile {tile} has too little relief"
        );
    }
}

#[test]
fn geology_palettes_are_distinct_from_existing_lookalikes() {
    let atlas = generate_atlas();
    let mean = |tile| {
        let pixels = tile_pixels(&atlas, tile);
        [0, 1, 2].map(|channel| {
            pixels
                .iter()
                .map(|pixel| u32::from(pixel[channel]))
                .sum::<u32>()
                / pixels.len() as u32
        })
    };
    let limestone = mean(18);
    let slate = mean(19);
    let basalt = mean(20);
    let calcite = mean(21);
    let sand = mean(4);
    let snow = mean(5);
    let bedrock = mean(10);
    let deep_stone = mean(11);

    assert!(limestone[0] >= 200 && limestone[2] > sand[2] + 20);
    assert!(slate[1] - slate[0] > deep_stone[1] - deep_stone[0] + 8);
    assert!(basalt[0] > bedrock[0] + 25 && basalt[0] > basalt[2] + 10);
    assert!(calcite[0] > limestone[0] + 8 && calcite[0] >= 220);
    assert!(calcite[2] + 25 < snow[2] && calcite[0] > calcite[2] + 20);
}

#[test]
fn bedding_and_fissility_run_across_columnar_and_flowstone_streaks() {
    let atlas = generate_atlas();
    let brightness = |tile, x, y| {
        let [r, g, b, _] = pixel(&atlas, tile, x, y);
        u32::from(r) + u32::from(g) + u32::from(b)
    };
    for tile in 18..=21 {
        let mut horizontal = 0;
        let mut vertical = 0;
        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let value = brightness(tile, x, y);
                if x + 1 < TILE_SIZE {
                    horizontal += value.abs_diff(brightness(tile, x + 1, y));
                }
                if y + 1 < TILE_SIZE {
                    vertical += value.abs_diff(brightness(tile, x, y + 1));
                }
            }
        }
        if tile <= 19 {
            assert!(
                vertical * 2 > horizontal * 3,
                "tile {tile} needs horizontal bedding: horizontal={horizontal}, vertical={vertical}"
            );
        } else {
            assert!(
                horizontal * 2 > vertical * 3,
                "tile {tile} needs vertical streaks: horizontal={horizontal}, vertical={vertical}"
            );
        }
    }
}

#[test]
fn bed_material_palettes_are_muted_and_distinct_from_dirt_slate_and_sand() {
    let atlas = generate_atlas();
    let mean = |tile| {
        let pixels = tile_pixels(&atlas, tile);
        [0, 1, 2].map(|channel| {
            pixels
                .iter()
                .map(|pixel| u32::from(pixel[channel]))
                .sum::<u32>()
                / pixels.len() as u32
        })
    };
    for [r, g, b, _] in tile_pixels(&atlas, 22) {
        assert!(r > g && g > b);
        assert!((8..=24).contains(&(r - b)));
        assert!(b >= 85 && r < 180);
    }
    for [r, g, b, _] in tile_pixels(&atlas, 23) {
        assert!(r > g && g > b);
        assert!(r - b <= 6);
        assert!(b >= 105 && r <= 145);
    }
    let gravel = mean(22);
    let clay = mean(23);
    let dirt = mean(2);
    let slate = mean(19);
    let sand = mean(4);
    assert!(gravel[0] - gravel[2] >= clay[0] - clay[2] + 8);
    for bed in [gravel, clay] {
        assert!(dirt[0] - dirt[2] > bed[0] - bed[2] + 40);
        assert!(bed[1] > slate[1] + 40);
        assert!(sand[0] > bed[0] + 60);
    }
}

#[test]
fn gravel_has_rounded_multi_pixel_pebbles_of_different_sizes() {
    let atlas = generate_atlas();
    let chroma = |x, y| {
        let [r, g, b, _] = pixel(&atlas, 22, x, y);
        (r - g, g - b)
    };
    let rounded = chroma(2, 2);
    let row_width = |y| (0..=4).filter(|&x| chroma(x, y) == rounded).count();
    assert_eq!(row_width(0), 1);
    assert_eq!(row_width(2), 5);
    assert_eq!(row_width(4), 1);
    let large = chroma(8, 1);
    let small = chroma(10, 6);
    assert_eq!((5..=11).filter(|&x| chroma(x, 1) == large).count(), 7);
    assert_eq!((9..=11).filter(|&x| chroma(x, 6) == small).count(), 3);

    let mut pebble_pixels = 0;
    let mut connected_pixels = 0;
    for y in 0..TILE_SIZE {
        for x in 0..TILE_SIZE {
            if pixel(&atlas, 22, x, y)[0] < 110 {
                continue;
            }
            pebble_pixels += 1;
            connected_pixels += usize::from(
                [
                    ((x + TILE_SIZE - 1) % TILE_SIZE, y),
                    ((x + 1) % TILE_SIZE, y),
                    (x, (y + TILE_SIZE - 1) % TILE_SIZE),
                    (x, (y + 1) % TILE_SIZE),
                ]
                .into_iter()
                .any(|(nx, ny)| pixel(&atlas, 22, nx, ny)[0] >= 110),
            );
        }
    }
    assert!((128..=208).contains(&pebble_pixels));
    assert_eq!(connected_pixels, pebble_pixels);
}

#[test]
fn clay_has_fine_horizontal_sediment_seams_without_rock_relief() {
    let atlas = generate_atlas();
    let brightness = |x, y| {
        let [r, g, b, _] = pixel(&atlas, 23, x, y);
        u32::from(r) + u32::from(g) + u32::from(b)
    };
    let values: Vec<_> = (0..TILE_SIZE)
        .flat_map(|y| (0..TILE_SIZE).map(move |x| brightness(x, y)))
        .collect();
    let relief = values.iter().max().unwrap() - values.iter().min().unwrap();
    assert!((45..=100).contains(&relief));

    let mut horizontal = 0;
    let mut vertical = 0;
    for y in 0..TILE_SIZE {
        for x in 0..TILE_SIZE {
            let value = brightness(x, y);
            if x + 1 < TILE_SIZE {
                horizontal += value.abs_diff(brightness(x + 1, y));
            }
            if y + 1 < TILE_SIZE {
                vertical += value.abs_diff(brightness(x, y + 1));
            }
        }
    }
    assert!(vertical > horizontal * 2);
}
