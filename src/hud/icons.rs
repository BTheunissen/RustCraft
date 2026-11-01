//! Cached, original atlas-derived pixel cubes, without GPU textures or assets.

use std::sync::OnceLock;

use crate::atlas::{Atlas, TILE_SIZE, generate_atlas};
use crate::game::Tool;
use crate::mesh::material_for_face;
use crate::world::Block;

use super::{Color, Painter};

const EDGE: usize = 20;
type Icon = [Option<Color>; EDGE * EDGE];
static ICONS: OnceLock<[Icon; Block::COUNT]> = OnceLock::new();

fn cached_icons() -> &'static [Icon; Block::COUNT] {
    ICONS.get_or_init(|| {
        let atlas = generate_atlas();
        Block::ALL.map(|block| {
            std::array::from_fn(|index| {
                if block == Block::Air {
                    return None;
                }
                let x = (index % EDGE) as f32 + 0.5;
                let y = (index / EDGE) as f32 + 0.5;
                let (normal, u, v, shade) = face_sample(x, y)?;
                let tile = material_for_face(block, normal);
                Some(texel(&atlas, tile, u, v, shade))
            })
        })
    })
}

fn face_sample(x: f32, y: f32) -> Option<(i32, f32, f32, f32)> {
    let dx = x - 10.0;
    let u = (y + dx * 0.5) / 10.0;
    let v = (y - dx * 0.5) / 10.0;
    if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
        return Some((1, u, v, 1.0));
    }
    let (u, v, shade) = if x < 10.0 {
        let u = x / 10.0;
        (u, (y - 5.0 - u * 5.0) / 9.0, 0.82)
    } else {
        let u = dx / 10.0;
        (u, (y - 10.0 + u * 5.0) / 9.0, 0.62)
    };
    ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then_some((0, u, v, shade))
}

fn texel(atlas: &Atlas, tile: u32, u: f32, v: f32, shade: f32) -> Color {
    let x = (u * TILE_SIZE as f32).floor() as u32;
    let y = (v * TILE_SIZE as f32).floor() as u32;
    let index = ((y * atlas.width + tile * TILE_SIZE + x) * 4) as usize;
    // HUD colors are linear; the world atlas stores sRGB bytes.
    let [r, g, b] = std::array::from_fn(|channel| {
        let value = f32::from(atlas.pixels[index + channel]) / 255.0;
        let linear = if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        };
        linear * shade
    });
    [r, g, b, 1.0]
}

pub(super) fn block_icon(painter: &mut Painter, block: Block, x: f32, y: f32, size: f32) {
    let pixel = size / EDGE as f32;
    for (index, color) in cached_icons()[block as usize].iter().enumerate() {
        if let Some(color) = color {
            painter.rect(
                [
                    x + (index % EDGE) as f32 * pixel,
                    y + (index / EDGE) as f32 * pixel,
                    pixel,
                    pixel,
                ],
                *color,
            );
        }
    }
}

pub(super) fn tool_icon(painter: &mut Painter, tool: Tool, x: f32, y: f32, size: f32) {
    let pixel = size / EDGE as f32;
    for row in 0..EDGE {
        for column in 0..EDGE {
            let handle = (8..=10).contains(&column) && (5..=18).contains(&row);
            let head = match tool {
                Tool::StonePickaxe => {
                    (3..=15).contains(&column) && (3..=5).contains(&row)
                        || ((column == 3 || column == 15) && row == 6)
                }
                Tool::StoneAxe => (10..=15).contains(&column) && (3..=8).contains(&row),
            };
            if handle || head {
                let shade = ((column + row) % 3) as f32 * 0.025;
                let color = if head {
                    [0.36 + shade, 0.41 + shade, 0.43 + shade, 1.0]
                } else {
                    [0.25 + shade, 0.13 + shade, 0.055 + shade, 1.0]
                };
                painter.rect(
                    [
                        x + column as f32 * pixel,
                        y + row as f32 * pixel,
                        pixel,
                        pixel,
                    ],
                    color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_cube_icons_are_cached_bounded_and_material_specific() {
        let icons = cached_icons();
        assert!(std::ptr::eq(icons, cached_icons()));
        assert!(icons[Block::Air as usize].iter().all(Option::is_none));
        for block in Block::ALL.into_iter().skip(1) {
            let icon = &icons[block as usize];
            assert!((240..320).contains(&icon.iter().flatten().count()));
            assert!(icon[0].is_none());
            assert!(icon.iter().flatten().all(|color| {
                color[3] == 1.0
                    && color
                        .iter()
                        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            }));
        }
        assert_ne!(icons[Block::Gravel as usize], icons[Block::Clay as usize]);
        assert_ne!(
            icons[Block::Stone as usize],
            icons[Block::DeepStone as usize]
        );
    }

    #[test]
    fn grass_and_wood_use_the_shared_top_and_side_tiles() {
        let atlas = generate_atlas();
        for (block, top, side) in [(Block::Grass, 0, 1), (Block::Wood, 7, 6)] {
            for (x, y, expected_tile) in [(10, 3, top), (4, 10, side), (16, 10, side)] {
                let (_, u, v, shade) = face_sample(x as f32 + 0.5, y as f32 + 0.5).unwrap();
                assert_eq!(
                    cached_icons()[block as usize][y * EDGE + x],
                    Some(texel(&atlas, expected_tile, u, v, shade)),
                );
            }
        }
    }

    #[test]
    fn icons_have_a_fixed_quad_budget_even_when_clipped() {
        for (x, y, size) in [(10.0, 10.0, 20.0), (-10.0, -8.0, 40.0), (10.0, 95.0, 160.0)] {
            let mut painter = Painter {
                width: 100.0,
                height: 100.0,
                vertices: Vec::new(),
            };
            block_icon(&mut painter, Block::Crystal, x, y, size);
            assert!(!painter.vertices.is_empty());
            assert!(painter.vertices.len() <= EDGE * EDGE * 6);
            assert!(painter.vertices.iter().all(|vertex| {
                vertex
                    .position
                    .iter()
                    .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
            }));
        }
    }
}
