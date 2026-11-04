//! An entry's glyph, replaced by its thumbnail once that is uploaded; videos
//! keep the play glyph and gain a duration badge.

use crownui::prelude::{AnyView, Styled, View, cover_image, hstack, keyed, text, vstack, zstack};

use crate::{
    state::Store,
    util::{
        duration_label::duration_label,
        thumb_store::{ThumbImage, ThumbSource},
    },
    views::glyph::{
        Glyph, PHOTO_SIZE, glyph_view,
        palette::{DURATION_PILL, DURATION_TEXT},
        play_mark,
    },
};

const RADIUS: f32 = 5.0;
const BADGE_TEXT_SIZE: f32 = 9.0;
/// Below this scale (list rows) a duration badge would not be legible.
const BADGE_MIN_SCALE: f32 = 1.0;

pub fn entry_picture(
    store: Store,
    glyph: Glyph,
    source: Option<ThumbSource>,
    scale: f32,
) -> AnyView {
    let Some(source) = source else {
        return glyph_view(glyph, scale);
    };
    keyed(
        move |runtime| {
            store.thumbs.revision.get(runtime);
            store
                .thumbs
                .images
                .with(runtime, |images| images.get(&source.path, source.modified))
        },
        move |_, thumb: Option<ThumbImage>| match thumb {
            Some(thumb) => thumbnail(thumb, glyph, scale),
            None => glyph_view(glyph, scale),
        },
    )
    .flex_col()
    .w(PHOTO_SIZE.width * scale)
    .h(PHOTO_SIZE.height * scale)
    .boxed()
}

fn thumbnail(thumb: ThumbImage, glyph: Glyph, scale: f32) -> AnyView {
    let picture = cover_image(thumb.image, thumb.size, RADIUS * scale)
        .w(PHOTO_SIZE.width * scale)
        .h(PHOTO_SIZE.height * scale);
    if glyph != Glyph::Video {
        return picture.boxed();
    }
    let badge = thumb
        .duration
        .filter(|_| scale >= BADGE_MIN_SCALE)
        .map(|duration| {
            vstack(hstack(
                text(duration_label(duration))
                    .text_size(BADGE_TEXT_SIZE)
                    .font_semibold()
                    .text_color(DURATION_TEXT)
                    .px(4.0)
                    .rounded(4.0)
                    .bg(DURATION_PILL),
            ))
            .w_full()
            .h_full()
            .items_end()
            .justify_end()
            .p(3.0)
            .boxed()
        })
        .unwrap_or_else(|| vstack(()).boxed());
    zstack((picture, play_mark(scale), badge))
        .w(PHOTO_SIZE.width * scale)
        .h(PHOTO_SIZE.height * scale)
        .boxed()
}
