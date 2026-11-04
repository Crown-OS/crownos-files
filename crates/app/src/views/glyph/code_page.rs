use crownui::{ext::DrawList, prelude::CornerRadii};

use super::{paint::rect, palette::CODE_LINES};

const LINE_LEFT: f32 = 6.0;
const LINE_SPAN: f32 = 34.0;
const LINE_TOP: f32 = 14.0;
const LINE_PITCH: f32 = 5.0;

/// Source code: colored lines on the dark page.
pub fn paint_code(list: &mut DrawList) {
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
}
