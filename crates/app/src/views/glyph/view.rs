use crownui::{
    ext::DrawList,
    kit::icon_colored,
    prelude::{AnyView, Size, Styled, View, canvas, zstack},
};

use super::{
    archive::{ARCHIVE_SIZE, paint_archive},
    audio_wave::paint_wave,
    choice::Glyph,
    code_page::paint_code,
    disc::{DISC_SIZE, paint_disc},
    folder::{FOLDER_SIZE, MARK_FRAME, MARK_STROKE, lucide, paint_folder},
    page::{PAGE_SIZE, Paper, page_badge, paint_paper, paint_swatches, paint_text_lines},
    paint::scaled,
    palette::FOLDER_INK,
    photo::{PHOTO_SIZE, paint_photo},
    sheet::paint_sheet,
    video::paint_video,
};
use crate::model::badge::Badge;

/// Below this scale (list rows) marks and badges would not be legible.
const SMALLEST_DETAILED_SCALE: f32 = 0.5;

/// What a glyph is made of beyond its painting, at its mockup size.
pub struct GlyphArt {
    pub size: Size,
    /// A Lucide body drawn in `FOLDER_INK` inside `MARK_FRAME`.
    pub mark: Option<&'static str>,
    pub badge: Option<Badge>,
}

impl GlyphArt {
    pub fn of(glyph: Glyph) -> Self {
        let plain = |size| Self {
            size,
            mark: None,
            badge: None,
        };
        let page = |badge| Self {
            size: PAGE_SIZE,
            mark: None,
            badge,
        };
        match glyph {
            Glyph::Folder(mark) => Self {
                size: FOLDER_SIZE,
                mark: mark.map(lucide),
                badge: None,
            },
            Glyph::Photo | Glyph::Video => plain(PHOTO_SIZE),
            Glyph::Disc => plain(DISC_SIZE),
            Glyph::Archive => plain(ARCHIVE_SIZE),
            Glyph::Page(badge)
            | Glyph::Vector(badge)
            | Glyph::Code(badge)
            | Glyph::Sheet(badge, _)
            | Glyph::Audio(badge, _) => page(badge),
        }
    }
}

/// Paints `glyph` at its mockup size from the origin.
pub fn paint_glyph(glyph: Glyph, list: &mut DrawList) {
    match glyph {
        Glyph::Folder(_) => paint_folder(list),
        Glyph::Photo => paint_photo(list),
        Glyph::Video => paint_video(list),
        Glyph::Disc => paint_disc(list),
        Glyph::Archive => paint_archive(list),
        Glyph::Page(_) => {
            paint_paper(list, Paper::Light);
            paint_text_lines(list);
        }
        Glyph::Vector(_) => {
            paint_paper(list, Paper::Light);
            paint_swatches(list);
        }
        Glyph::Code(_) => {
            paint_paper(list, Paper::Dark);
            paint_code(list);
        }
        Glyph::Sheet(_, accent) => {
            paint_paper(list, Paper::Light);
            paint_sheet(list, accent);
        }
        Glyph::Audio(_, accent) => {
            paint_paper(list, Paper::Light);
            paint_wave(list, accent);
        }
    }
}

/// The glyph's view at `scale` times its mockup size.
pub fn glyph_view(glyph: Glyph, scale: f32) -> AnyView {
    let art = GlyphArt::of(glyph);
    let detailed = scale >= SMALLEST_DETAILED_SCALE;
    let painting = canvas(move |_, _, list| scaled(list, scale, |list| paint_glyph(glyph, list)))
        .w(art.size.width * scale)
        .h(art.size.height * scale);
    if !detailed || (art.mark.is_none() && art.badge.is_none()) {
        return painting.boxed();
    }
    let mark = art.mark.map(|body| {
        let frame = MARK_FRAME;
        icon_colored(body, MARK_STROKE, |_| FOLDER_INK)
            .size(frame.size.width * scale)
            .absolute()
            .left(frame.origin.x * scale)
            .top(frame.origin.y * scale)
    });
    let badge = art.badge.map(|badge| page_badge(badge, scale));
    zstack((painting, mark, badge)).boxed()
}
