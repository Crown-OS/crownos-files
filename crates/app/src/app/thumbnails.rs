//! Thumbnails and content types for the page in view: requested newest
//! first, uploaded to the GPU as they arrive and evicted by byte budget.

use std::{ops::Range, path::Path, sync::Arc};

use compact_str::CompactString;
use crownui::prelude::{Cx, Runtime, Signal, Size, Store as Cell};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    app::{tasks::Task, virtual_folder::VirtualFolder},
    model::{entry::Entry, kind::FileKind, mime_id::MimeId},
    state::Store,
    thumbs::{
        ThumbCache, ThumbKey, ThumbSize, Thumbnail, ThumbnailEvent, ThumbnailService,
        prefetch_range,
    },
    util::{
        drag_icon::Swatch,
        hash::FastHashSet,
        thumb_store::{ThumbImage, ThumbSource, ThumbStore},
        xdg_dirs::XdgDirs,
    },
    views::{glyph::Glyph, metrics::THUMB_WELL},
};

/// GPU memory the uploaded thumbnails may hold.
pub const THUMB_BUDGET: usize = 64 << 20;
const SWATCH_EDGE: u32 = 48;

#[derive(Debug)]
pub enum ThumbCommand {
    Request(ThumbKey),
    CancelAll,
}

/// What was last asked for, so scrolling within a row asks nothing again.
#[derive(Debug, Default)]
struct Asked {
    dir: Option<Arc<Path>>,
    visible: Range<usize>,
    listing: u64,
    size: Option<ThumbSize>,
    sniffed: FastHashSet<CompactString>,
}

#[derive(Clone, Copy)]
pub struct Thumbnails {
    pub images: Cell<ThumbStore>,
    pub revision: Signal<u64>,
    pub service: Cell<Option<UnboundedSender<ThumbCommand>>>,
    asked: Cell<Asked>,
}

impl Thumbnails {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            images: cx.store(ThumbStore::new(THUMB_BUDGET)),
            revision: cx.signal(0),
            service: cx.store(None),
            asked: cx.store(Asked::default()),
        }
    }

    fn send(self, runtime: &Runtime, command: ThumbCommand) {
        self.service.with(runtime, |service| {
            if let Some(service) = service {
                let _ = service.send(command);
            }
        });
    }
}

/// Where the thumbnail of `entry` in `dir` comes from, for images and videos.
pub fn thumb_source(entry: &Entry, dir: &Path, glyph: Glyph) -> Option<ThumbSource> {
    glyph.has_thumbnail().then(|| ThumbSource {
        path: entry.path_in(dir).into(),
        modified: entry.stat.modified,
    })
}

/// Serves thumbnail requests from the shared cache until the window closes.
pub async fn run_thumbnailer(
    events: UnboundedSender<ThumbnailEvent>,
    mut commands: UnboundedReceiver<ThumbCommand>,
) {
    let Some(xdg) = XdgDirs::from_env() else {
        return;
    };
    let (service, mut produced) = ThumbnailService::spawn(ThumbCache::from_xdg(&xdg));
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(ThumbCommand::Request(key)) => service.request(key),
                Some(ThumbCommand::CancelAll) => service.cancel_all(),
                None => break,
            },
            Some(event) = produced.recv() => {
                if events.send(event).is_err() {
                    break;
                }
            }
        }
    }
}

/// Keeps thumbnails and sniffed types coming for the page in view.
pub fn follow_visible(cx: &mut Cx, store: Store) {
    let scale = cx.scale_factor();
    cx.effect(move |runtime| {
        store.revision.get(runtime);
        let visible = store.scroll.visible_range(runtime);
        let edge = (THUMB_WELL.width * scale.get(runtime)).ceil() as u32;
        request_page(runtime, store, visible, ThumbSize::covering(edge));
    });
}

/// Asks for the thumbnails of the prefetch range around `visible` (the
/// margin first, then the visible rows bottom-up, as the newest is served
/// first) and sniffs the visible entries whose names give no type.
pub fn request_page(runtime: &mut Runtime, store: Store, visible: Range<usize>, size: ThumbSize) {
    let (dir, listing, count) = store.folder.with(runtime, |folder| {
        (folder.dir().clone(), folder.revision(), folder.len())
    });
    let thumbs = store.thumbs;
    let unchanged = thumbs.asked.update(runtime, |asked| {
        let same_dir = asked.dir.as_ref() == Some(&dir);
        let unchanged = same_dir
            && asked.visible == visible
            && asked.listing == listing
            && asked.size == Some(size);
        if !same_dir {
            asked.sniffed.clear();
        }
        asked.dir = Some(dir.clone());
        asked.visible = visible.clone();
        asked.listing = listing;
        asked.size = Some(size);
        (unchanged, same_dir)
    });
    match unchanged {
        (true, _) => return,
        (false, false) => thumbs.send(runtime, ThumbCommand::CancelAll),
        (false, true) => {}
    }
    let page = prefetch_range(count, visible.clone(), visible.len());
    let order = page
        .filter(|position| !visible.contains(position))
        .chain(visible.clone().rev());
    let (keys, unknown) = store.folder.with(runtime, |folder| {
        let mut keys = Vec::new();
        let mut unknown = Vec::new();
        for position in order {
            let Some(entry) = folder.get(position) else {
                continue;
            };
            if let Some(source) = thumb_source(entry, &dir, Glyph::for_entry(entry, None)) {
                keys.push(ThumbKey {
                    path: source.path,
                    modified: source.modified,
                    size,
                });
            }
            if visible.contains(&position) && needs_sniffing(entry) {
                unknown.push((entry.name.clone(), entry.path_in(&dir)));
            }
        }
        (keys, unknown)
    });
    let fresh: Vec<ThumbKey> = thumbs.images.update(runtime, |images| {
        keys.into_iter()
            .filter(|key| !images.touch(&key.path, key.modified))
            .collect()
    });
    for key in fresh {
        thumbs.send(runtime, ThumbCommand::Request(key));
    }
    sniff_unknown(runtime, store, dir, unknown);
}

fn needs_sniffing(entry: &Entry) -> bool {
    !entry.is_dir()
        && !entry.stat.is_dummy()
        && entry.kind == FileKind::Other
        && entry.mime == MimeId::UNKNOWN
}

fn sniff_unknown(
    runtime: &mut Runtime,
    store: Store,
    dir: Arc<Path>,
    mut unknown: Vec<(CompactString, std::path::PathBuf)>,
) {
    if VirtualFolder::of(&dir).is_some() {
        return;
    }
    store.thumbs.asked.update(runtime, |asked| {
        unknown.retain(|(name, _)| asked.sniffed.insert(name.clone()));
    });
    if !unknown.is_empty() {
        store.run(
            runtime,
            Task::Sniff {
                dir,
                entries: unknown,
            },
        );
    }
}

/// Uploads finished thumbnails, releasing what they replace or push out.
pub fn finish_thumbnails(cx: &mut Cx, store: Store, events: Vec<ThumbnailEvent>) {
    let mut uploaded = false;
    for event in events {
        let ThumbnailEvent::Ready(thumbnail) = event else {
            continue;
        };
        let Thumbnail {
            key,
            width,
            height,
            rgba,
            duration,
        } = thumbnail;
        let swatch = Swatch::shrink(width, height, &rgba, SWATCH_EDGE);
        let image = cx.register_image(width, height, rgba);
        let thumb = ThumbImage {
            image,
            size: Size::new(width as f32, height as f32),
            duration,
        };
        let released = store.thumbs.images.update(cx, |images| {
            images.insert(key.path, key.modified, thumb, swatch)
        });
        for image in released {
            cx.release_image(image);
        }
        uploaded = true;
    }
    if uploaded {
        store.thumbs.revision.update(cx, |revision| *revision += 1);
    }
}
