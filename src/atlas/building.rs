//! Original processed-building textures, separate from generated bark/geology.

use super::{hash, noise, shade};

pub(super) fn planks(x: u32, y: u32) -> [u8; 3] {
    let row = y / 4;
    let seam = y.is_multiple_of(4) || (x + row * 5).is_multiple_of(16);
    let base = if seam { [114, 73, 36] } else { [185, 132, 73] };
    let grain = noise(x, y, 301, 5)
        + noise(x / 4, row, 302, 7)
        + if (x + y / 2).is_multiple_of(7) { -5 } else { 0 };
    let nail = y % 4 == 1 && (x + row * 5) % 16 == 1;
    shade(base, grain - if nail { 30 } else { 0 })
}

pub(super) fn dressed_limestone(x: u32, y: u32) -> [u8; 3] {
    let row = y / 8;
    let edge = y.is_multiple_of(8) || (x + row * 8).is_multiple_of(16);
    let base = if edge {
        [164, 154, 130]
    } else {
        [225, 216, 190]
    };
    let bevel = if y % 8 == 1 || (x + row * 8) % 16 == 1 {
        9
    } else {
        0
    };
    let tool_mark = if hash(x / 2, y / 2, 311).is_multiple_of(13) {
        -5
    } else {
        0
    };
    shade(base, bevel + tool_mark + noise(x, y, 312, 3))
}

pub(super) fn storage_crate(x: u32, y: u32) -> [u8; 3] {
    let frame = x <= 2 || x >= 13 || y <= 2 || y >= 13;
    let brace = x.abs_diff(y) <= 1 || x.abs_diff(15 - y) <= 1;
    if frame || brace {
        shade([113, 72, 33], noise(x, y, 321, 7))
    } else {
        planks(x, y)
    }
}

pub(super) fn lantern(x: u32, y: u32) -> [u8; 3] {
    let frame = x <= 2 || x >= 13 || y <= 2 || y >= 13 || x == 7 || x == 8;
    let base = if frame { [79, 67, 45] } else { [251, 188, 63] };
    shade(
        base,
        noise(x, y, 331, 4) - if !frame && y > 10 { 15 } else { 0 },
    )
}
