//! A glyph as SVG, built from the same painting, mark and badge as
//! [`glyph_view`](super::glyph_view), for the drag icon.

use std::fmt::Write;

use crownui::{
    ext::DrawList,
    prelude::{CornerRadii, Point, Rect, Size},
};

use super::{
    BADGE_BOTTOM, BADGE_PADDING, BADGE_RADIUS, BADGE_TEXT_SIZE, Glyph, GlyphArt, MARK_FRAME,
    MARK_STROKE, paint_glyph,
    palette::{BADGE_TEXT, FOLDER_INK, badge_color},
};
use crate::{
    model::badge::Badge,
    util::{
        drag_icon::{ART_ORIGIN, ART_SIZE, ICON},
        draw_svg::{draw_list_svg, hex, rounded_rect_path},
        svg_raster::rasterize,
    },
    views::metrics::TILE_GLYPH_SCALE,
};

/// Inter Bold's average capital advance, as a share of the font size.
const BADGE_ADVANCE: f32 = 0.7;
/// Inter's line height, as a share of the font size.
const LINE_HEIGHT: f32 = 1.21;

/// `glyph` at the grid's tile size, centered in the drag icon's art box, on
/// a transparent `ICON` square at `scale`.
pub fn render_drag_glyph(glyph: Glyph, scale: f32) -> Option<Vec<u8>> {
    let size = GlyphArt::of(glyph).size;
    let left = ART_ORIGIN.0 + (ART_SIZE.width - size.width * TILE_GLYPH_SCALE) / 2.0;
    let top = ART_ORIGIN.1 + (ART_SIZE.height - size.height * TILE_GLYPH_SCALE) / 2.0;
    let body = format!(
        r#"<g transform="translate({left} {top}) scale({TILE_GLYPH_SCALE})">{}</g>"#,
        glyph_svg(glyph)
    );
    rasterize(&body, Size::new(ICON, ICON), scale)
}

/// The glyph at its mockup size from the origin.
fn glyph_svg(glyph: Glyph) -> String {
    let art = GlyphArt::of(glyph);
    let mut list = DrawList::new();
    paint_glyph(glyph, &mut list);
    let mut svg = draw_list_svg(&list);
    if let Some(body) = art.mark {
        let (ink, alpha) = hex(FOLDER_INK);
        let _ = write!(
            svg,
            r#"<svg x="{}" y="{}" width="{}" height="{}" viewBox="0 0 24 24" fill="none" stroke="{ink}" stroke-opacity="{alpha}" stroke-width="{MARK_STROKE}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#,
            MARK_FRAME.origin.x, MARK_FRAME.origin.y, MARK_FRAME.size.width, MARK_FRAME.size.height
        );
    }
    if let Some(badge) = art.badge {
        badge_svg(&mut svg, badge, art.size);
    }
    svg
}

/// The page's format pill, laid out as `page_badge` lays it out.
fn badge_svg(svg: &mut String, badge: Badge, page: Size) {
    let text_width = badge.label.chars().count() as f32 * BADGE_TEXT_SIZE * BADGE_ADVANCE;
    let pill = Size::new(
        text_width + 2.0 * BADGE_PADDING,
        BADGE_TEXT_SIZE * LINE_HEIGHT,
    );
    let left = (page.width - pill.width) / 2.0;
    let top = page.height - BADGE_BOTTOM - pill.height;
    let outline = rounded_rect_path(
        Rect::new(Point::new(left, top), pill),
        CornerRadii::uniform(BADGE_RADIUS),
    );
    let (fill, _) = hex(badge_color(badge.color));
    let (ink, _) = hex(BADGE_TEXT);
    let _ = write!(
        svg,
        r#"<path d="{outline}" fill="{fill}"/><text x="{}" y="{}" font-family="Inter Variable, Inter" font-weight="700" font-size="{BADGE_TEXT_SIZE}" fill="{ink}" text-anchor="middle" dominant-baseline="central">{}</text>"#,
        page.width / 2.0,
        top + pill.height / 2.0,
        badge.label
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{badge::badge_for, special_dir::FolderGlyph},
        views::glyph::palette::FOLDER_FRONT,
    };

    fn pixel(rgba: &[u8], x: f32, y: f32) -> [u8; 4] {
        let start = (y as usize * ICON as usize + x as usize) * 4;
        [
            rgba[start],
            rgba[start + 1],
            rgba[start + 2],
            rgba[start + 3],
        ]
    }

    fn near(color: [u8; 4], expected: [u8; 4]) -> bool {
        color[..3]
            .iter()
            .zip(&expected[..3])
            .all(|(got, want)| got.abs_diff(*want) < 40)
    }

    /// The point `(x, y)` of the glyph's mockup coordinates, in the icon.
    fn at(glyph: Glyph, x: f32, y: f32) -> (f32, f32) {
        let size = GlyphArt::of(glyph).size;
        (
            ART_ORIGIN.0
                + (ART_SIZE.width - size.width * TILE_GLYPH_SCALE) / 2.0
                + x * TILE_GLYPH_SCALE,
            ART_ORIGIN.1
                + (ART_SIZE.height - size.height * TILE_GLYPH_SCALE) / 2.0
                + y * TILE_GLYPH_SCALE,
        )
    }

    #[test]
    fn a_folder_is_painted_at_tile_size_on_a_clear_square() {
        let folder = Glyph::Folder(None);
        let Some(rgba) = render_drag_glyph(folder, 1.0) else {
            panic!("the glyph renders");
        };
        assert_eq!(rgba.len(), (ICON * ICON) as usize * 4);
        assert_eq!(pixel(&rgba, 0.0, 0.0)[3], 0, "outside the glyph is clear");
        let (x, y) = at(folder, 10.0, 20.0);
        let front = pixel(&rgba, x, y);
        assert!(front[3] > 240, "{front:?}");
        assert!(near(front, FOLDER_FRONT.0.to_rgba8()), "{front:?}");
    }

    #[test]
    fn special_folders_carry_their_mark_and_pages_their_badge() {
        let plain = render_drag_glyph(Glyph::Folder(None), 1.0);
        let marked = render_drag_glyph(Glyph::Folder(Some(FolderGlyph::Music)), 1.0);
        assert!(plain.is_some() && marked.is_some() && plain != marked);
        let page = |badge| render_drag_glyph(Glyph::Page(badge), 2.0);
        let (bare, badged) = (page(None), page(badge_for("pdf")));
        assert!(bare.is_some() && badged.is_some() && bare != badged);
    }
}
