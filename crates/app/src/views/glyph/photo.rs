use crownui::{
    ext::{Brush, DrawList, Paint},
    prelude::{Color, CornerRadii, Point, Size, Styled, View, canvas},
};

use super::{
    paint::{drop_shadow, inner_ring, linear, rect, scaled},
    palette::{DROP_SHADOW, PHOTO, PHOTO_GLOWS, PHOTO_RIM},
};

pub const PHOTO_SIZE: Size = Size::new(68.0, 44.0);
const RADIUS: f32 = 5.0;
const GLOW_BLUR: f32 = 12.0;

/// An image whose thumbnail is not decoded yet: a dark print with two glows.
pub fn photo(scale: f32) -> impl View {
    canvas(move |_, _, list| scaled(list, scale, paint_photo))
        .w(PHOTO_SIZE.width * scale)
        .h(PHOTO_SIZE.height * scale)
}

pub(super) fn paint_photo(list: &mut DrawList) {
    let bounds = rect(0.0, 0.0, PHOTO_SIZE.width, PHOTO_SIZE.height);
    let radii = CornerRadii::uniform(RADIUS);
    drop_shadow(list, bounds, RADIUS, DROP_SHADOW);
    list.save();
    list.clip(bounds, radii);
    list.fill(
        bounds,
        radii,
        linear(
            Point::new(20.0, 0.0),
            Point::new(48.0, PHOTO_SIZE.height),
            &[PHOTO.0, PHOTO.1],
        ),
    );
    glow(list, rect(-14.0, 22.0, 52.0, 40.0), PHOTO_GLOWS[0]);
    glow(list, rect(40.0, -18.0, 48.0, 34.0), PHOTO_GLOWS[1]);
    list.restore();
    inner_ring(list, bounds, RADIUS, PHOTO_RIM);
}

fn glow(list: &mut DrawList, area: crownui::prelude::Rect, color: Color) {
    list.fill(
        area,
        CornerRadii::uniform(area.size.height / 2.0),
        Paint {
            brush: Brush::Solid(color),
            blur: Some(GLOW_BLUR),
        },
    );
}
