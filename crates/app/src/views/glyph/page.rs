use crownui::{
    ext::DrawList,
    kit::INTER_FAMILY,
    prelude::{CornerRadii, Point, Size, Styled, View, hstack, text},
};

use super::{
    paint::{fill_path, page_outline, rect, vertical},
    palette::{
        BADGE_TEXT, CODE_EDGE, CODE_FOLD, CODE_PAGE, PAGE, PAGE_EDGE, PAGE_FOLD, PAGE_LINE,
        SWATCHES, badge_color,
    },
};
use crate::model::badge::Badge;

pub const PAGE_SIZE: Size = Size::new(46.0, 60.0);
const FOLD: Size = Size::new(PAGE_SIZE.width * 0.32, PAGE_SIZE.height * 0.24);
const RADIUS: f32 = 4.0;
pub const BADGE_TEXT_SIZE: f32 = 7.5;
pub const BADGE_RADIUS: f32 = 3.0;
pub const BADGE_PADDING: f32 = 4.0;
pub const BADGE_BOTTOM: f32 = 6.0;

/// Light paper or the dark sheet code is printed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    Light,
    Dark,
}

/// A text page: lines of type.
pub fn paint_text_lines(list: &mut DrawList) {
    for top in [14.0, 20.0, 26.0, 32.0, 38.0] {
        list.fill(rect(7.0, top, 32.0, 2.0), CornerRadii::ZERO, PAGE_LINE);
    }
}

/// A vector image: four color swatches.
pub fn paint_swatches(list: &mut DrawList) {
    let cell = Size::new(13.0, 11.0);
    for (index, color) in SWATCHES.into_iter().enumerate() {
        let origin = Point::new(
            8.0 + (index % 2) as f32 * (cell.width + 4.0),
            14.0 + (index / 2) as f32 * (cell.height + 4.0),
        );
        list.fill(
            rect(origin.x, origin.y, cell.width, cell.height),
            CornerRadii::uniform(cell.height / 2.0),
            color,
        );
    }
}

/// A page with a folded corner, its body painted over it after.
pub fn paint_paper(list: &mut DrawList, paper: Paper) {
    let (fill, fold, edge) = match paper {
        Paper::Light => (PAGE, PAGE_FOLD, PAGE_EDGE),
        Paper::Dark => (CODE_PAGE, CODE_FOLD, CODE_EDGE),
    };
    let outline = page_outline(PAGE_SIZE.width, PAGE_SIZE.height, FOLD, RADIUS);
    fill_path(list, outline.clone(), vertical(0.0, PAGE_SIZE.height, fill));
    super::paint::stroke_path(list, outline, 1.0, edge);
    let fold_left = PAGE_SIZE.width - FOLD.width;
    let corner = crownui::ext::Path::new()
        .move_to(Point::new(fold_left, 0.0))
        .line_to(Point::new(PAGE_SIZE.width, FOLD.height))
        .line_to(Point::new(fold_left, FOLD.height))
        .close();
    fill_path(list, corner, fold);
}

/// The pill naming a page's format, centered near its foot.
pub fn page_badge(badge: Badge, scale: f32) -> impl View {
    hstack(
        text(badge.label)
            .text_size(BADGE_TEXT_SIZE * scale)
            .font_bold()
            .font_family(INTER_FAMILY)
            .text_color(BADGE_TEXT)
            .bg(badge_color(badge.color))
            .rounded(BADGE_RADIUS * scale)
            .px(BADGE_PADDING * scale),
    )
    .absolute()
    .left(0.0)
    .right(0.0)
    .bottom(BADGE_BOTTOM * scale)
    .justify_center()
}
