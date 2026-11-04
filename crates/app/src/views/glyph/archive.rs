use crownui::{
    ext::DrawList,
    prelude::{CornerRadii, Size},
};

use super::{
    paint::{drop_shadow, rect, vertical},
    palette::{ARCHIVE, ARCHIVE_HIGHLIGHT, DROP_SHADOW, ZIPPER, ZIPPER_PULL},
};

pub const ARCHIVE_SIZE: Size = Size::new(50.0, 56.0);
const SIZE: Size = ARCHIVE_SIZE;
const RADIUS: f32 = 6.0;
const ZIPPER_WIDTH: f32 = 6.0;
const ZIPPER_END: f32 = SIZE.height - 18.0;
const TOOTH: f32 = 3.0;
const PULL: Size = Size::new(10.0, 12.0);
const PULL_BOTTOM: f32 = 8.0;

/// An archive: a box with a zipper down its middle.
pub fn paint_archive(list: &mut DrawList) {
    let bounds = rect(0.0, 0.0, SIZE.width, SIZE.height);
    drop_shadow(list, bounds, RADIUS, DROP_SHADOW);
    list.fill(
        bounds,
        CornerRadii::uniform(RADIUS),
        vertical(0.0, SIZE.height, ARCHIVE),
    );
    list.fill(
        rect(RADIUS, 0.0, SIZE.width - 2.0 * RADIUS, 1.0),
        CornerRadii::ZERO,
        ARCHIVE_HIGHLIGHT,
    );
    let left = (SIZE.width - ZIPPER_WIDTH) / 2.0;
    let mut top = 0.0;
    let mut light = true;
    while top < ZIPPER_END {
        let color = if light { ZIPPER.0 } else { ZIPPER.1 };
        let height = TOOTH.min(ZIPPER_END - top);
        list.fill(
            rect(left, top, ZIPPER_WIDTH, height),
            CornerRadii::ZERO,
            color,
        );
        top += TOOTH;
        light = !light;
    }
    list.fill(
        rect(
            (SIZE.width - PULL.width) / 2.0,
            SIZE.height - PULL_BOTTOM - PULL.height,
            PULL.width,
            PULL.height,
        ),
        CornerRadii::uniform(3.0),
        ZIPPER_PULL,
    );
}
