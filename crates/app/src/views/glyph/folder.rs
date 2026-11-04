use crownui::{
    ext::{DrawList, Path},
    kit::{icon_colored, icons},
    prelude::{CornerRadii, Point, Size, Styled, View, canvas, zstack},
};

use super::{
    paint::{fill_path, rect, scaled, vertical},
    palette::{FOLDER_BACK, FOLDER_FRONT, FOLDER_INK, FOLDER_RIM, FOLDER_SHEET},
};
use crate::model::special_dir::FolderGlyph;

const SIZE: Size = Size::new(66.0, 54.0);
const MARK_SIZE: f32 = 18.0;
const MARK_TOP: f32 = 25.0;
const MARK_STROKE: f32 = 2.0;
const SMALLEST_MARKED_SCALE: f32 = 0.5;

/// A folder: a dark back with its tab, a white sheet and a lighter front,
/// optionally marked with the special directory's Lucide glyph.
pub fn folder(mark: Option<FolderGlyph>, scale: f32) -> impl View {
    let art = canvas(move |_, _, list| scaled(list, scale, paint))
        .w(SIZE.width * scale)
        .h(SIZE.height * scale);
    let mark = mark.filter(|_| scale >= SMALLEST_MARKED_SCALE).map(|mark| {
        icon_colored(lucide(mark), MARK_STROKE, |_| FOLDER_INK)
            .size(MARK_SIZE * scale)
            .absolute()
            .left((SIZE.width - MARK_SIZE) / 2.0 * scale)
            .top(MARK_TOP * scale)
    });
    zstack((art, mark))
}

fn lucide(mark: FolderGlyph) -> &'static str {
    match mark {
        FolderGlyph::Image => icons::IMAGE,
        FolderGlyph::Music => icons::MUSIC,
        FolderGlyph::Video => icons::VIDEO,
        FolderGlyph::Download => icons::DOWNLOAD,
        FolderGlyph::Share => icons::SHARE,
    }
}

fn paint(list: &mut DrawList) {
    let back = Path::new()
        .move_to(Point::new(4.0, 8.5))
        .quad_to(Point::new(4.0, 4.5), Point::new(8.0, 4.5))
        .line_to(Point::new(23.0, 4.5))
        .quad_to(Point::new(24.8, 4.5), Point::new(25.9, 5.7))
        .line_to(Point::new(29.3, 9.3))
        .quad_to(Point::new(30.4, 10.5), Point::new(32.2, 10.5))
        .line_to(Point::new(58.0, 10.5))
        .quad_to(Point::new(62.0, 10.5), Point::new(62.0, 14.5))
        .line_to(Point::new(62.0, 46.0))
        .quad_to(Point::new(62.0, 50.0), Point::new(58.0, 50.0))
        .line_to(Point::new(8.0, 50.0))
        .quad_to(Point::new(4.0, 50.0), Point::new(4.0, 46.0))
        .close();
    fill_path(list, back, vertical(4.5, 50.0, FOLDER_BACK));
    list.fill(
        rect(8.0, 13.0, 50.0, 22.0),
        CornerRadii::uniform(2.0),
        FOLDER_SHEET,
    );
    list.fill(
        rect(4.0, 17.0, 58.0, 33.0),
        CornerRadii::uniform(4.0),
        vertical(17.0, 50.0, FOLDER_FRONT),
    );
    list.fill(rect(8.0, 17.0, 50.0, 1.2), CornerRadii::ZERO, FOLDER_RIM);
}
