//! Selecting by dragging a rectangle over the empty part of the file area.
//! Hit-testing uses the grid's cell math, not the mounted views, so entries
//! scrolled out of view are covered too.

use crownui::{
    ext::ItemLayout,
    kit::use_theme,
    prelude::{CornerRadii, Cx, Point, Rect, Runtime, Size, Styled, Vector, View, canvas},
};

use crate::{app::selection::select_positions, state::Store, views::metrics::layout_for};

const FILL_ALPHA: f32 = 0.14;
const EDGE_ALPHA: f32 = 0.7;
const RADIUS: f32 = 4.0;

/// The rectangle with corners `from` and `to`.
pub fn spanning(from: Point, to: Point) -> Rect {
    let origin = Point::new(from.x.min(to.x), from.y.min(to.y));
    Rect::new(
        origin,
        Size::new((from.x - to.x).abs(), (from.y - to.y).abs()),
    )
}

/// The positions of the `count` items `layout` places in `width` that `band` touches.
pub fn covered(layout: &ItemLayout, width: f32, count: usize, band: Rect) -> Vec<usize> {
    let columns = layout.columns(width);
    let column_width = layout.column_width(width, columns);
    let rows = ItemLayout::row_count(count, columns);
    let first_row = ((band.min_y() - layout.padding) / layout.stride())
        .floor()
        .max(0.0) as usize;
    let mut positions = Vec::new();
    for row in first_row..rows {
        let top = layout.row_top(row);
        if top > band.max_y() {
            break;
        }
        if top + layout.height < band.min_y() {
            continue;
        }
        for column in 0..columns {
            let left = layout.padding + column as f32 * (column_width + layout.gap.0);
            let position = row * columns + column;
            if position < count && left <= band.max_x() && left + column_width >= band.min_x() {
                positions.push(position);
            }
        }
    }
    positions
}

/// The item under `point`, in scrolled-content coordinates.
pub fn item_at(layout: &ItemLayout, width: f32, count: usize, point: Point) -> Option<usize> {
    covered(layout, width, count, Rect::new(point, Size::ZERO))
        .first()
        .copied()
}

/// Where a pointer at `position` in the viewport is in the scrolled content.
fn content_point(runtime: &mut Runtime, store: Store, position: Point) -> Point {
    let offset = store.scroll.offset().get(runtime);
    Point::new(position.x + offset.x, position.y + offset.y)
}

fn geometry(runtime: &mut Runtime, store: Store) -> (ItemLayout, f32, usize) {
    (
        layout_for(store.view_mode.get(runtime)),
        store.scroll.size().get(runtime).width,
        store.count.get(runtime),
    )
}

/// The entry under a pointer at `position` in the viewport.
pub fn entry_at(runtime: &mut Runtime, store: Store, position: Point) -> Option<usize> {
    let point = content_point(runtime, store, position);
    let (layout, width, count) = geometry(runtime, store);
    item_at(&layout, width, count, point)
}

/// Starts a band at `position`, which is over no entry (dragging entries drags files).
pub fn start_band(runtime: &mut Runtime, store: Store, position: Point) {
    let point = content_point(runtime, store, position);
    store.band.set(runtime, Some(Rect::new(point, Size::ZERO)));
}

/// Stretches the band to `position` and selects what it covers.
pub fn extend_band(runtime: &mut Runtime, store: Store, position: Point, anchor: Point) {
    let point = content_point(runtime, store, position);
    let band = spanning(anchor, point);
    store.band.set(runtime, Some(band));
    let (layout, width, count) = geometry(runtime, store);
    select_positions(runtime, store, covered(&layout, width, count, band));
}

pub fn finish_band(runtime: &mut Runtime, store: Store) {
    if store.band.with(runtime, Option::is_some) {
        store.band.set(runtime, None);
    }
}

/// The translucent accent rectangle over the file area while a band is dragged.
pub fn rubber_band(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    canvas(move |runtime, _, list| {
        let Some(band) = store.band.get(runtime) else {
            return;
        };
        let offset = store.scroll.offset().get(runtime);
        let accent = theme.with(runtime, |theme| theme.accent.start);
        let shown = band.translate(Vector::new(-offset.x, -offset.y));
        let radii = CornerRadii::uniform(RADIUS);
        list.fill(shown, radii, accent.with_alpha_factor(FILL_ALPHA));
        list.stroke(shown, radii, 1.0, accent.with_alpha_factor(EDGE_ALPHA));
    })
    .pointer_events_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::metrics::{GRID_PADDING, TILE, TILE_GAP};

    fn grid() -> ItemLayout {
        ItemLayout::cells(TILE, TILE_GAP).with_padding(GRID_PADDING)
    }

    #[test]
    fn a_band_covers_the_cells_it_touches_in_every_row() {
        let width = 2.0 * GRID_PADDING + 4.0 * TILE.width + 3.0 * TILE_GAP.0;
        let band = spanning(Point::new(20.0, 20.0), Point::new(110.0, 130.0));
        assert_eq!(covered(&grid(), width, 16, band), [0, 1, 4, 5]);
        assert!(
            covered(&grid(), width, 2, band)
                .iter()
                .all(|&position| position < 2)
        );
    }

    #[test]
    fn the_padding_and_gaps_hold_no_item() {
        let width = 2.0 * GRID_PADDING + 4.0 * TILE.width + 3.0 * TILE_GAP.0;
        assert_eq!(item_at(&grid(), width, 16, Point::new(4.0, 4.0)), None);
        assert_eq!(item_at(&grid(), width, 16, Point::new(50.0, 50.0)), Some(0));
        let second_row = GRID_PADDING + TILE.height + TILE_GAP.1 + 1.0;
        assert_eq!(
            item_at(&grid(), width, 16, Point::new(50.0, second_row)),
            Some(4)
        );
    }
}
