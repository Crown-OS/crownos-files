use crownui::prelude::{CornerRadii, View};

use super::{
    page::{Paper, page_with},
    paint::rect,
    palette::CODE_LINES,
};
use crate::model::badge::Badge;

const LINE_LEFT: f32 = 6.0;
const LINE_SPAN: f32 = 34.0;
const LINE_TOP: f32 = 14.0;
const LINE_PITCH: f32 = 5.0;

/// Source code: a dark page of colored lines.
pub fn code_page(badge: Option<Badge>, scale: f32) -> impl View {
    page_with(Paper::Dark, badge, scale, |list| {
        for (row, (color, length)) in CODE_LINES.into_iter().enumerate() {
            list.fill(
                rect(
                    LINE_LEFT,
                    LINE_TOP + row as f32 * LINE_PITCH,
                    LINE_SPAN * length,
                    2.0,
                ),
                CornerRadii::uniform(1.0),
                color,
            );
        }
    })
}
