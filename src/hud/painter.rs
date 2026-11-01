//! Clipped rectangles and bitmap text geometry.

use super::{Color, HudVertex, MAX_TEXT_CHARS, glyph};

pub(super) struct Painter {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) vertices: Vec<HudVertex>,
}

impl Painter {
    pub(super) fn rect(&mut self, rect: [f32; 4], color: Color) {
        let [x, y, width, height] = rect;
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let left = x.max(0.0);
        let top = y.max(0.0);
        let right = (x + width).min(self.width);
        let bottom = (y + height).min(self.height);
        if left >= right || top >= bottom {
            return;
        }

        let left = left / self.width * 2.0 - 1.0;
        let right = right / self.width * 2.0 - 1.0;
        let top = 1.0 - top / self.height * 2.0;
        let bottom = 1.0 - bottom / self.height * 2.0;
        for position in [
            [left, top],
            [left, bottom],
            [right, bottom],
            [left, top],
            [right, bottom],
            [right, top],
        ] {
            self.vertices.push(HudVertex { position, color });
        }
    }

    pub(super) fn frame(&mut self, rect: [f32; 4], thickness: f32, color: Color) {
        let [x, y, width, height] = rect;
        self.rect([x, y, width, thickness], color);
        self.rect([x, y + height - thickness, width, thickness], color);
        self.rect([x, y, thickness, height], color);
        self.rect([x + width - thickness, y, thickness, height], color);
    }

    pub(super) fn text(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Color) {
        for (index, character) in text.chars().take(MAX_TEXT_CHARS).enumerate() {
            for (row, bits) in glyph(character).into_iter().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        self.rect(
                            [
                                x + (index as f32 * 6.0 + column as f32) * scale,
                                y + row as f32 * scale,
                                scale,
                                scale,
                            ],
                            color,
                        );
                    }
                }
            }
        }
    }

    pub(super) fn centered_text(
        &mut self,
        center: f32,
        y: f32,
        text: &str,
        scale: f32,
        color: Color,
    ) {
        self.text(
            center - text_width(text, scale) * 0.5,
            y,
            text,
            scale,
            color,
        );
    }
}

pub(super) fn text_width(text: &str, scale: f32) -> f32 {
    let count = text.chars().take(MAX_TEXT_CHARS).count();
    if count == 0 {
        0.0
    } else {
        (count as f32 * 6.0 - 1.0) * scale
    }
}

pub(super) fn truncate_text(text: &str, limit: usize) -> String {
    let limit = limit.min(MAX_TEXT_CHARS);
    let mut characters: Vec<_> = text.chars().take(limit + 1).collect();
    if characters.len() > limit {
        characters.truncate(limit.saturating_sub(3));
        characters.extend(std::iter::repeat_n('.', limit.min(3)));
    }
    characters.into_iter().collect()
}

pub(super) fn fit_text(text: &str, width: f32, scale: f32) -> String {
    let limit = ((width + scale) / (6.0 * scale)).floor().max(0.0) as usize;
    truncate_text(text, limit)
}
