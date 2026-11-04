//! Thumbnails: which files are asked for, and the placeholder giving way to
//! the uploaded picture.

use std::{path::Path, time::Duration};

use crowntest::{Role, role, text};

use super::window::{Window, drain, window};
use crate::{
    app::thumbnails::{ThumbCommand, finish_thumbnails},
    thumbs::{ThumbKey, Thumbnail, ThumbnailEvent},
};

fn requested(window: &mut Window) -> Vec<ThumbKey> {
    drain(&mut window.workers.thumbs)
        .into_iter()
        .filter_map(|command| match command {
            ThumbCommand::Request(key) => Some(key),
            ThumbCommand::CancelAll => None,
        })
        .collect()
}

fn names(keys: &[ThumbKey]) -> Vec<&str> {
    keys.iter()
        .filter_map(|key| key.path.file_name()?.to_str())
        .collect()
}

#[test]
fn the_images_and_videos_in_view_are_asked_for_bottom_up() {
    let mut window = window();
    let keys = requested(&mut window);
    let asked = names(&keys);
    assert_eq!(
        asked[asked.len() - 3..],
        [
            "launch-film.webm",
            "crownbook-hero.png",
            "aurora-wallpaper.avif"
        ],
        "the top-left one is asked for last, so it is served first"
    );
    assert!(
        asked.iter().all(|name| [
            "launch-film.webm",
            "crownbook-hero.png",
            "aurora-wallpaper.avif"
        ]
        .contains(name)),
        "only images and videos"
    );
    window.key("Ctrl+2");
    assert!(
        requested(&mut window).is_empty(),
        "switching views asks nothing again"
    );
}

#[test]
fn a_decoded_thumbnail_replaces_its_placeholder_and_a_video_shows_its_length() {
    let mut window = window();
    let keys = requested(&mut window);
    let images = |window: &mut Window| window.app.count(role(Role::Image));
    let before = images(&mut window);
    for key in keys {
        window.answer(|cx, store| {
            let ready = [key]
                .into_iter()
                .map(|key| {
                    let duration = key
                        .path
                        .ends_with(Path::new("launch-film.webm"))
                        .then_some(Duration::from_secs(84));
                    ThumbnailEvent::Ready(Thumbnail {
                        key,
                        width: 4,
                        height: 3,
                        rgba: vec![200; 48],
                        duration,
                    })
                })
                .collect();
            finish_thumbnails(cx, store, ready);
        });
    }
    assert_eq!(images(&mut window), before + 3);
    assert!(window.shows(text("1:24")));
    window.click("aurora-wallpaper.avif");
    assert_eq!(
        images(&mut window),
        before + 4,
        "the details panel shows it too"
    );
}
