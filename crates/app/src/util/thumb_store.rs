//! The uploaded thumbnails, least recently wanted first, kept under a byte
//! budget of GPU memory.

use std::{path::Path, sync::Arc, time::Duration};

use crownui::{ext::ImageId, prelude::Size};

use crate::{
    model::timestamp::Timestamp,
    util::{drag_icon::Swatch, hash::FastIndexMap},
};

const RGBA_BYTES: usize = 4;

/// The file a thumbnail is of, as it was when listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbSource {
    pub path: Arc<Path>,
    pub modified: Timestamp,
}

/// What a tile needs to draw a thumbnail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThumbImage {
    pub image: ImageId,
    pub size: Size,
    pub duration: Option<Duration>,
}

#[derive(Debug)]
struct Cached {
    modified: Timestamp,
    thumb: ThumbImage,
    swatch: Option<Swatch>,
    bytes: usize,
}

#[derive(Debug)]
pub struct ThumbStore {
    cached: FastIndexMap<Arc<Path>, Cached>,
    bytes: usize,
    budget: usize,
}

impl ThumbStore {
    pub fn new(budget: usize) -> Self {
        Self {
            cached: FastIndexMap::default(),
            bytes: 0,
            budget,
        }
    }

    /// The thumbnail of `path` as it was at `modified`.
    pub fn get(&self, path: &Path, modified: Timestamp) -> Option<ThumbImage> {
        self.cached
            .get(path)
            .filter(|cached| cached.modified == modified)
            .map(|cached| cached.thumb)
    }

    pub fn swatch(&self, path: &Path) -> Option<&Swatch> {
        self.cached.get(path)?.swatch.as_ref()
    }

    /// Marks the thumbnail of `path` as just wanted; `false` when there is
    /// none for `modified`, so it must be requested.
    pub fn touch(&mut self, path: &Path, modified: Timestamp) -> bool {
        let Some(index) = self.cached.get_index_of(path) else {
            return false;
        };
        if self.cached[index].modified != modified {
            return false;
        }
        let newest = self.cached.len() - 1;
        self.cached.move_index(index, newest);
        true
    }

    /// Keeps `thumb` of `path` and returns the images to release: the one it
    /// replaces and the oldest ones over the budget.
    pub fn insert(
        &mut self,
        path: Arc<Path>,
        modified: Timestamp,
        thumb: ThumbImage,
        swatch: Option<Swatch>,
    ) -> Vec<ImageId> {
        let bytes = thumb.size.width as usize * thumb.size.height as usize * RGBA_BYTES
            + swatch.as_ref().map_or(0, Swatch::bytes);
        let mut released: Vec<ImageId> = self
            .cached
            .shift_remove(&path)
            .map(|old| self.forget(old))
            .into_iter()
            .collect();
        self.bytes += bytes;
        self.cached.insert(
            path,
            Cached {
                modified,
                thumb,
                swatch,
                bytes,
            },
        );
        while self.bytes > self.budget && self.cached.len() > 1 {
            if let Some((_, oldest)) = self.cached.shift_remove_index(0) {
                released.push(self.forget(oldest));
            }
        }
        released
    }

    fn forget(&mut self, cached: Cached) -> ImageId {
        self.bytes -= cached.bytes;
        cached.thumb.image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumb(id: u64) -> ThumbImage {
        ThumbImage {
            image: ImageId(id),
            size: Size::new(10.0, 10.0),
            duration: None,
        }
    }

    fn path(name: &str) -> Arc<Path> {
        Path::new(name).into()
    }

    #[test]
    fn the_least_recently_wanted_go_first_over_budget() {
        let mut store = ThumbStore::new(2 * 400);
        let at = Timestamp::from_parts(1, 0);
        assert!(store.insert(path("/a"), at, thumb(1), None).is_empty());
        assert!(store.insert(path("/b"), at, thumb(2), None).is_empty());
        assert!(store.touch(Path::new("/a"), at));
        assert_eq!(store.insert(path("/c"), at, thumb(3), None), [ImageId(2)]);
        assert_eq!(store.get(Path::new("/a"), at), Some(thumb(1)));
        assert_eq!(store.get(Path::new("/b"), at), None);
    }

    #[test]
    fn a_changed_file_needs_a_new_thumbnail_and_releases_the_old() {
        let mut store = ThumbStore::new(4_000);
        let (old, new) = (Timestamp::from_parts(1, 0), Timestamp::from_parts(2, 0));
        store.insert(path("/a"), old, thumb(1), None);
        assert_eq!(store.get(Path::new("/a"), new), None);
        assert!(!store.touch(Path::new("/a"), new));
        assert_eq!(store.insert(path("/a"), new, thumb(2), None), [ImageId(1)]);
        assert_eq!(store.get(Path::new("/a"), new), Some(thumb(2)));
    }
}
