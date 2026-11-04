//! The picture that follows the pointer while files are dragged: a rounded
//! card of the first file's thumbnail (or its kind color) and, for several
//! files, an accent badge with the count.

use crownui::prelude::{Color, DragIcon, Point};

const RGBA: usize = 4;
const ICON: f32 = 64.0;
const CARD_ORIGIN: (f32, f32) = (4.0, 12.0);
const CARD: f32 = 48.0;
const CARD_RADIUS: f32 = 9.0;
const CARD_OPACITY: f32 = 0.92;
const BADGE_CENTER: (f32, f32) = (51.0, 13.0);
const BADGE_RADIUS: f32 = 11.0;
const GLYPH_COLUMNS: usize = 3;
const GLYPH_ROWS: usize = 5;
const GLYPH_CELL: f32 = 2.0;
const MOST_COUNTED: usize = 99;
/// 3×5 bitmaps, row-major from the top-left bit: 0–9, then "+".
const GLYPHS: [u16; 11] = [
    0b111_101_101_101_111,
    0b010_110_010_010_111,
    0b111_001_111_100_111,
    0b111_001_111_001_111,
    0b101_101_111_001_001,
    0b111_100_111_001_111,
    0b111_100_111_101_111,
    0b111_001_001_001_001,
    0b111_101_111_101_111,
    0b111_101_111_001_111,
    0b000_010_111_010_000,
];
const PLUS: usize = 10;

/// A thumbnail shrunk to a few dozen pixels, kept for drag icons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swatch {
    width: u32,
    height: u32,
    rgba: Box<[u8]>,
}

impl Swatch {
    /// `rgba` (`width × height`) sampled down to fit an `edge` square.
    pub fn shrink(width: u32, height: u32, rgba: &[u8], edge: u32) -> Option<Self> {
        let longest = width.max(height).max(1);
        let fit = |side: u32| (side * edge.min(longest) / longest).max(1);
        let (fitted_width, fitted_height) = (fit(width), fit(height));
        let mut shrunk = Vec::with_capacity(fitted_width as usize * fitted_height as usize * RGBA);
        for y in 0..fitted_height {
            let row = (y * height / fitted_height) * width;
            for x in 0..fitted_width {
                let start = (row + x * width / fitted_width) as usize * RGBA;
                shrunk.extend_from_slice(rgba.get(start..start + RGBA)?);
            }
        }
        Some(Self {
            width: fitted_width,
            height: fitted_height,
            rgba: shrunk.into_boxed_slice(),
        })
    }

    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }

    /// The pixel under `(x, y)` of a `side` square this swatch covers, centred.
    fn covering(&self, x: f32, y: f32, side: f32) -> [u8; 4] {
        let scale = (side / self.width as f32).max(side / self.height as f32);
        let source = |at: f32, extent: u32| {
            let offset = (side - extent as f32 * scale) / 2.0;
            (((at - offset) / scale) as u32).min(extent - 1)
        };
        let start = (source(y, self.height) * self.width + source(x, self.width)) as usize * RGBA;
        let mut pixel = [0; RGBA];
        pixel.copy_from_slice(&self.rgba[start..start + RGBA]);
        pixel
    }
}

/// The colors of the icon, from the theme and the file's kind.
#[derive(Debug, Clone, Copy)]
pub struct DragIconColors {
    pub card: Color,
    pub badge: Color,
    pub badge_text: Color,
}

pub fn drag_icon(
    swatch: Option<&Swatch>,
    count: usize,
    colors: DragIconColors,
    scale: f32,
) -> DragIcon {
    let side = (ICON * scale).round().max(1.0) as u32;
    let mut canvas = Canvas {
        side,
        rgba: vec![0; side as usize * side as usize * RGBA],
    };
    let card_fill = colors.card.to_rgba8();
    let (left, top, card) = (CARD_ORIGIN.0 * scale, CARD_ORIGIN.1 * scale, CARD * scale);
    canvas.paint(|x, y| {
        let coverage = rounded_square(x - left, y - top, card, CARD_RADIUS * scale);
        let fill = swatch.map_or(card_fill, |swatch| swatch.covering(x - left, y - top, card));
        (coverage * CARD_OPACITY > 0.0).then_some((fill, coverage * CARD_OPACITY))
    });
    if count > 1 {
        paint_badge(&mut canvas, count, colors, scale);
    }
    DragIcon {
        width: side,
        height: side,
        rgba: canvas.rgba,
        logical_width: ICON as u32,
        logical_height: ICON as u32,
        hotspot: Point::new(CARD_ORIGIN.0 + CARD / 2.0, CARD_ORIGIN.1 + CARD / 2.0),
    }
}

fn paint_badge(canvas: &mut Canvas, count: usize, colors: DragIconColors, scale: f32) {
    let (center_x, center_y) = (BADGE_CENTER.0 * scale, BADGE_CENTER.1 * scale);
    let radius = BADGE_RADIUS * scale;
    let badge = colors.badge.to_rgba8();
    canvas.paint(|x, y| {
        let distance = (x - center_x).hypot(y - center_y);
        let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
        (coverage > 0.0).then_some((badge, coverage))
    });
    let glyphs = count_glyphs(count);
    let cell = (GLYPH_CELL * scale).round().max(1.0);
    let advance = (GLYPH_COLUMNS + 1) as f32 * cell;
    let width = glyphs.len() as f32 * advance - cell;
    let (left, top) = (
        (center_x - width / 2.0).round(),
        (center_y - GLYPH_ROWS as f32 * cell / 2.0).round(),
    );
    let ink = colors.badge_text.to_rgba8();
    canvas.paint(|x, y| {
        let (column, row) = (((x - left) / cell).floor(), ((y - top) / cell).floor());
        if column < 0.0 || row < 0.0 || row >= GLYPH_ROWS as f32 {
            return None;
        }
        let (glyph, column) = (
            column as usize / (GLYPH_COLUMNS + 1),
            column as usize % (GLYPH_COLUMNS + 1),
        );
        if column >= GLYPH_COLUMNS {
            return None;
        }
        let bits = *GLYPHS.get(*glyphs.get(glyph)?)?;
        let bit = GLYPH_COLUMNS * GLYPH_ROWS - 1 - (row as usize * GLYPH_COLUMNS + column);
        (bits >> bit & 1 == 1).then_some((ink, 1.0))
    });
}

fn count_glyphs(count: usize) -> Vec<usize> {
    let shown = count.min(MOST_COUNTED);
    let mut glyphs: Vec<usize> = shown
        .to_string()
        .bytes()
        .map(|digit| usize::from(digit - b'0'))
        .collect();
    if count > MOST_COUNTED {
        glyphs.push(PLUS);
    }
    glyphs
}

/// How much of the pixel at `(x, y)` a `side` square with `radius` corners covers.
fn rounded_square(x: f32, y: f32, side: f32, radius: f32) -> f32 {
    let half = side / 2.0;
    let (dx, dy) = (
        ((x + 0.5 - half).abs() - half + radius).max(0.0),
        ((y + 0.5 - half).abs() - half + radius).max(0.0),
    );
    let outside = dx.hypot(dy) - radius;
    (0.5 - outside).clamp(0.0, 1.0)
}

struct Canvas {
    side: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    /// Blends what `shade` returns for each pixel over the canvas.
    fn paint(&mut self, mut shade: impl FnMut(f32, f32) -> Option<([u8; 4], f32)>) {
        let side = self.side as usize;
        for (index, pixel) in self.rgba.chunks_exact_mut(RGBA).enumerate() {
            let (x, y) = ((index % side) as f32, (index / side) as f32);
            if let Some((color, coverage)) = shade(x, y) {
                blend(pixel, color, coverage);
            }
        }
    }
}

fn blend(pixel: &mut [u8], color: [u8; 4], coverage: f32) {
    let source = f32::from(color[3]) / 255.0 * coverage;
    let below = f32::from(pixel[3]) / 255.0;
    let alpha = source + below * (1.0 - source);
    if alpha <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let mixed = (f32::from(color[channel]) * source
            + f32::from(pixel[channel]) * below * (1.0 - source))
            / alpha;
        pixel[channel] = mixed.round() as u8;
    }
    pixel[3] = (alpha * 255.0).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crownui::prelude::{BLACK, WHITE, rgb};

    const COLORS: DragIconColors = DragIconColors {
        card: rgb(0x3b82f6),
        badge: rgb(0x8f6dfb),
        badge_text: WHITE,
    };

    fn pixel(icon: &DragIcon, x: f32, y: f32) -> [u8; 4] {
        let start = (y as usize * icon.width as usize + x as usize) * RGBA;
        icon.rgba[start..start + RGBA].try_into().unwrap()
    }

    #[test]
    fn one_file_is_a_card_without_a_badge() {
        let icon = drag_icon(None, 1, COLORS, 1.0);
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(pixel(&icon, 28.0, 36.0)[..3], [0x3b, 0x82, 0xf6]);
        assert_eq!(pixel(&icon, 0.0, 0.0)[3], 0, "outside the card is clear");
        assert_eq!(pixel(&icon, BADGE_CENTER.0, BADGE_CENTER.1 - 8.0)[3], 0);
    }

    #[test]
    fn several_files_get_a_counted_badge_at_the_display_scale() {
        let icon = drag_icon(None, 3, COLORS, 2.0);
        assert_eq!((icon.width, icon.logical_width), (128, 64));
        let edge = pixel(&icon, BADGE_CENTER.0 * 2.0, (BADGE_CENTER.1 - 9.0) * 2.0);
        assert_eq!(edge[..3], [0x8f, 0x6d, 0xfb]);
        let inked = icon
            .rgba
            .chunks_exact(RGBA)
            .filter(|pixel| pixel[..3] == [255, 255, 255] && pixel[3] == 255)
            .count();
        assert!(inked > 0, "the count is drawn");
    }

    #[test]
    fn counts_above_99_read_99_plus() {
        assert_eq!(count_glyphs(7), [7]);
        assert_eq!(count_glyphs(42), [4, 2]);
        assert_eq!(count_glyphs(500), [9, 9, PLUS]);
    }

    #[test]
    fn a_swatch_shrinks_by_sampling_and_covers_the_card() {
        let black = BLACK.to_rgba8();
        let white = WHITE.to_rgba8();
        let rgba: Vec<u8> = [white, black, black, white].into_iter().flatten().collect();
        let swatch = Swatch::shrink(2, 2, &rgba, 1).unwrap();
        assert_eq!((swatch.width, swatch.height, swatch.bytes()), (1, 1, 4));
        let full = Swatch::shrink(2, 2, &rgba, 40).unwrap();
        assert_eq!(full.covering(0.0, 0.0, 10.0), white);
        assert_eq!(full.covering(9.0, 0.0, 10.0), black);
    }
}
