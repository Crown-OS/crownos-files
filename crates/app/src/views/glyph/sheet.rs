use crownui::{ext::DrawList, prelude::CornerRadii};

use super::{
    paint::rect,
    palette::{SHEET_RULE, badge_color},
};
use crate::model::badge::BadgeColor;

const GRID_LEFT: f32 = 6.0;
const GRID_TOP: f32 = 13.0;
const GRID_WIDTH: f32 = 34.0;
const GRID_HEIGHT: f32 = 28.0;
const ROW_PITCH: f32 = 5.0;
const COLUMN_PITCH: f32 = 11.0;

/// A spreadsheet: ruled cells under a header band in the badge color.
pub fn paint_sheet(list: &mut DrawList, accent: BadgeColor) {
    let header = badge_color(accent);
    let mut top = GRID_TOP;
    while top < GRID_TOP + GRID_HEIGHT {
        list.fill(
            rect(GRID_LEFT, top, GRID_WIDTH, 1.0),
            CornerRadii::ZERO,
            SHEET_RULE,
        );
        top += ROW_PITCH;
    }
    let mut left = GRID_LEFT;
    while left < GRID_LEFT + GRID_WIDTH {
        list.fill(
            rect(left, GRID_TOP, 1.0, GRID_HEIGHT),
            CornerRadii::ZERO,
            SHEET_RULE,
        );
        left += COLUMN_PITCH;
    }
    list.fill(
        rect(GRID_LEFT, GRID_TOP, GRID_WIDTH * 0.3, ROW_PITCH),
        CornerRadii::ZERO,
        header,
    );
}
