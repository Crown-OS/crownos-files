//! The picture that follows the pointer while files are dragged: the first
//! file as its grid tile draws it (its thumbnail, or its glyph rendered on
//! the GPU) and, for several files, an accent badge with the count.

use crownui::prelude::{Color, DragIcon, Point, Size};

const RGBA: usize = 4;
pub const ICON: f32 = 96.0;
/// Where the art sits in the icon: the glyph is centered in this box.
pub const ART_ORIGIN: (f32, f32) = (4.0, 16.0);
pub const ART_SIZE: Size = Size::new(80.0, 76.0);
/// A thumbnail at the grid's photo size and corner radius.
const THUMB_SIZE: Size = Size::new(80.0, 52.0);
const THUMB_RADIUS: f32 = 6.0;
const CARD_SIDE: f32 = 56.0;
const CARD_RADIUS: f32 = 9.0;
const ART_OPACITY: f32 = 0.92;
const BADGE_CENTER: (f32, f32) = (83.0, 14.0);
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

    /// The pixel under `(x, y)` of a `frame` this swatch covers, centred.
    fn covering(&self, x: f32, y: f32, frame: Size) -> [u8; 4] {
        let scale = (frame.width / self.width as f32).max(frame.height / self.height as f32);
        let source = |at: f32, extent: u32, side: f32| {
            let offset = (side - extent as f32 * scale) / 2.0;
            (((at - offset).max(0.0) / scale) as u32).min(extent - 1)
        };
        let (row, column) = (
            source(y, self.height, frame.height),
            source(x, self.width, frame.width),
        );
        let start = (row * self.width + column) as usize * RGBA;
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

/// What stands for the dragged files.
pub enum DragArt<'a> {
    /// The first file's thumbnail.
    Thumbnail(&'a Swatch),
    /// The first file's glyph, already painted on the icon's square.
    Rendered(Vec<u8>),
    /// A card in the file's kind color, when the glyph could not be rendered.
    Card,
}

pub fn drag_icon(art: DragArt, count: usize, colors: DragIconColors, scale: f32) -> DragIcon {
    let side = (ICON * scale).round().max(1.0) as u32;
    let length = side as usize * side as usize * RGBA;
    let mut canvas = match art {
        DragArt::Rendered(mut rgba) if rgba.len() == length => {
            rgba.chunks_exact_mut(RGBA)
                .for_each(|pixel| pixel[3] = (f32::from(pixel[3]) * ART_OPACITY) as u8);
            Canvas { side, rgba }
        }
        DragArt::Thumbnail(swatch) => {
            let mut canvas = Canvas::blank(side);
            paint_centered(
                &mut canvas,
                THUMB_SIZE,
                THUMB_RADIUS,
                scale,
                |x, y, frame| swatch.covering(x, y, frame),
            );
            canvas
        }
        DragArt::Rendered(_) | DragArt::Card => {
            let mut canvas = Canvas::blank(side);
            let fill = colors.card.to_rgba8();
            let card = Size::new(CARD_SIDE, CARD_SIDE);
            paint_centered(&mut canvas, card, CARD_RADIUS, scale, |_, _, _| fill);
            canvas
        }
    };
    if count > 1 {
        paint_badge(&mut canvas, count, colors, scale);
    }
    DragIcon {
        width: side,
        height: side,
        rgba: canvas.rgba,
        logical_width: ICON as u32,
        logical_height: ICON as u32,
        hotspot: Point::new(
            ART_ORIGIN.0 + ART_SIZE.width / 2.0,
            ART_ORIGIN.1 + ART_SIZE.height / 2.0,
        ),
    }
}

/// A rounded `size` box centered in the art area, each pixel from `fill`
/// given its position inside the box and the box's device size.
fn paint_centered(
    canvas: &mut Canvas,
    size: Size,
    radius: f32,
    scale: f32,
    fill: impl Fn(f32, f32, Size) -> [u8; 4],
) {
    let frame = Size::new(size.width * scale, size.height * scale);
    let left = (ART_ORIGIN.0 + (ART_SIZE.width - size.width) / 2.0) * scale;
    let top = (ART_ORIGIN.1 + (ART_SIZE.height - size.height) / 2.0) * scale;
    canvas.paint(|x, y| {
        let (x, y) = (x - left, y - top);
        let coverage = rounded_rect(x, y, frame, radius * scale) * ART_OPACITY;
        (coverage > 0.0).then(|| (fill(x, y, frame), coverage))
    });
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

/// How much of the pixel at `(x, y)` a `frame` box with `radius` corners covers.
fn rounded_rect(x: f32, y: f32, frame: Size, radius: f32) -> f32 {
    let (half_width, half_height) = (frame.width / 2.0, frame.height / 2.0);
    let (dx, dy) = (
        ((x + 0.5 - half_width).abs() - half_width + radius).max(0.0),
        ((y + 0.5 - half_height).abs() - half_height + radius).max(0.0),
    );
    let outside = dx.hypot(dy) - radius;
    (0.5 - outside).clamp(0.0, 1.0)
}

struct Canvas {
    side: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    fn blank(side: u32) -> Self {
        Self {
            side,
            rgba: vec![0; side as usize * side as usize * RGBA],
        }
    }

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
        let icon = drag_icon(DragArt::Card, 1, COLORS, 1.0);
        assert_eq!((icon.width, icon.height), (96, 96));
        assert_eq!(pixel(&icon, 44.0, 54.0)[..3], [0x3b, 0x82, 0xf6]);
        assert_eq!(pixel(&icon, 0.0, 0.0)[3], 0, "outside the card is clear");
        assert_eq!(pixel(&icon, BADGE_CENTER.0, BADGE_CENTER.1 - 8.0)[3], 0);
    }

    #[test]
    fn several_files_get_a_counted_badge_at_the_display_scale() {
        let icon = drag_icon(DragArt::Card, 3, COLORS, 2.0);
        assert_eq!((icon.width, icon.logical_width), (192, 96));
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
        let square = Size::new(10.0, 10.0);
        assert_eq!(full.covering(0.0, 0.0, square), white);
        assert_eq!(full.covering(9.0, 0.0, square), black);
    }

    #[test]
    fn a_rendered_glyph_is_kept_and_a_mismatched_one_falls_back_to_the_card() {
        let side = ICON as usize;
        let rendered = vec![255; side * side * RGBA];
        let icon = drag_icon(DragArt::Rendered(rendered), 1, COLORS, 1.0);
        assert_eq!(pixel(&icon, 0.0, 0.0)[..3], [255, 255, 255]);
        let icon = drag_icon(DragArt::Rendered(vec![255; 4]), 1, COLORS, 1.0);
        assert_eq!(pixel(&icon, 0.0, 0.0)[3], 0);
        assert_eq!(pixel(&icon, 44.0, 54.0)[..3], [0x3b, 0x82, 0xf6]);
    }
}
