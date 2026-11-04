use crownui::{
    ext::{DrawList, Path},
    prelude::{Point, Styled, View, canvas},
};

use super::{
    paint::{fill_path, scaled},
    palette::PLAY,
    photo::{PHOTO_SIZE, paint_photo},
};

const PLAY_HALF: f32 = 7.0;

/// A video whose frame is not decoded yet: the photo placeholder with a play glyph.
pub fn paint_video(list: &mut DrawList) {
    paint_photo(list);
    paint_play(list);
}

/// The play glyph alone, laid over a video's decoded frame.
pub fn play_mark(scale: f32) -> impl View {
    canvas(move |_, _, list| scaled(list, scale, paint_play))
        .w(PHOTO_SIZE.width * scale)
        .h(PHOTO_SIZE.height * scale)
}

fn paint_play(list: &mut DrawList) {
    let center = Point::new(PHOTO_SIZE.width / 2.0, PHOTO_SIZE.height / 2.0);
    let play = Path::new()
        .move_to(Point::new(center.x - PLAY_HALF * 0.7, center.y - PLAY_HALF))
        .line_to(Point::new(center.x + PLAY_HALF, center.y))
        .line_to(Point::new(center.x - PLAY_HALF * 0.7, center.y + PLAY_HALF))
        .close();
    fill_path(list, play, PLAY);
}
