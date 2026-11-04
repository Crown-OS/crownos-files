use crownui::prelude::{CornerRadii, Point, Size, Styled, View, canvas};

use super::{
    paint::{drop_shadow, inner_ring, linear, rect, scaled},
    palette::{DISC, DISC_HOLE, DISC_RIM, DISC_RING, DROP_SHADOW},
};

const SIZE: Size = Size::new(56.0, 56.0);
const RING_RADIUS: f32 = 7.5;
const HOLE_RADIUS: f32 = 5.9;

/// A disk image: a silver disc with its hub.
pub fn disc(scale: f32) -> impl View {
    canvas(move |_, _, list| {
        scaled(list, scale, |list| {
            let bounds = rect(0.0, 0.0, SIZE.width, SIZE.height);
            let round = SIZE.width / 2.0;
            drop_shadow(list, bounds, round, DROP_SHADOW);
            list.fill(
                bounds,
                CornerRadii::uniform(round),
                linear(
                    Point::ZERO,
                    Point::new(SIZE.width, SIZE.height),
                    &[DISC[0], DISC[1], DISC[2], DISC[3], DISC[0]],
                ),
            );
            inner_ring(list, bounds, round, DISC_RIM);
            for (radius, color) in [(RING_RADIUS, DISC_RING), (HOLE_RADIUS, DISC_HOLE)] {
                list.fill(
                    rect(round - radius, round - radius, 2.0 * radius, 2.0 * radius),
                    CornerRadii::uniform(radius),
                    color,
                );
            }
        });
    })
    .w(SIZE.width * scale)
    .h(SIZE.height * scale)
}
