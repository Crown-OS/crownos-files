//! The file area's geometry, shared by the lazy views, the rubber band and
//! the keyboard's page size.

use crownconfig::ViewMode;
use crownui::{ext::ItemLayout, prelude::Size};

pub const TILE: Size = Size::new(90.0, 112.0);
pub const TILE_GAP: (f32, f32) = (6.0, 2.0);
pub const GRID_PADDING: f32 = 12.0;
pub const THUMB_WELL: Size = Size::new(76.0, 62.0);
pub const ROW_HEIGHT: f32 = 30.0;
pub const LIST_PADDING: f32 = 6.0;
pub const LIST_ICON: f32 = 22.0;
pub const DETAILS_WIDTH: f32 = 212.0;
pub const STATUS_HEIGHT: f32 = 32.0;

pub fn layout_for(mode: ViewMode) -> ItemLayout {
    match mode {
        ViewMode::Grid => ItemLayout::cells(TILE, TILE_GAP).with_padding(GRID_PADDING),
        ViewMode::List => ItemLayout::rows(ROW_HEIGHT).with_padding(LIST_PADDING),
    }
}
