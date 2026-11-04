use crownui::{
    ext::DrawList,
    kit::INTER_FAMILY,
    prelude::{CornerRadii, Point, Size, Styled, View, canvas, hstack, text, zstack},
};

use super::{
    paint::{fill_path, page_outline, rect, scaled, vertical},
    palette::{
        BADGE_TEXT, CODE_EDGE, CODE_FOLD, CODE_PAGE, PAGE, PAGE_EDGE, PAGE_FOLD, PAGE_LINE,
        SWATCHES, badge_color,
    },
};
use crate::model::badge::Badge;

pub const PAGE_SIZE: Size = Size::new(46.0, 60.0);
const FOLD: Size = Size::new(PAGE_SIZE.width * 0.32, PAGE_SIZE.height * 0.24);
const RADIUS: f32 = 4.0;
const SMALLEST_BADGED_SCALE: f32 = 0.5;

/// Light paper or the dark sheet code is printed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    Light,
    Dark,
}

/// A page with a folded corner, `body` painted on it and `badge` naming its format.
pub fn page_with(
    paper: Paper,
    badge: Option<Badge>,
    scale: f32,
    body: impl Fn(&mut DrawList) + 'static,
) -> impl View {
    let art = canvas(move |_, _, list| {
        scaled(list, scale, |list| {
            paint_paper(list, paper);
            body(list);
        });
    })
    .w(PAGE_SIZE.width * scale)
    .h(PAGE_SIZE.height * scale);
    let badge = badge
        .filter(|_| scale >= SMALLEST_BADGED_SCALE)
        .map(|badge| page_badge(badge, scale));
    zstack((art, badge))
}

/// A text page: lines of type.
pub fn page(badge: Option<Badge>, scale: f32) -> impl View {
    page_with(Paper::Light, badge, scale, |list| {
        for top in [14.0, 20.0, 26.0, 32.0, 38.0] {
            list.fill(rect(7.0, top, 32.0, 2.0), CornerRadii::ZERO, PAGE_LINE);
        }
    })
}

/// A vector image: four color swatches.
pub fn swatch_page(badge: Option<Badge>, scale: f32) -> impl View {
    page_with(Paper::Light, badge, scale, |list| {
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
    })
}

fn paint_paper(list: &mut DrawList, paper: Paper) {
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

fn page_badge(badge: Badge, scale: f32) -> impl View {
    hstack(
        text(badge.label)
            .text_size(7.5 * scale)
            .font_bold()
            .font_family(INTER_FAMILY)
            .text_color(BADGE_TEXT)
            .bg(badge_color(badge.color))
            .rounded(3.0 * scale)
            .px(4.0 * scale),
    )
    .absolute()
    .left(0.0)
    .right(0.0)
    .bottom(6.0 * scale)
    .justify_center()
}
