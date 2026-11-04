use std::{path::Path, sync::Arc, time::Duration};

use crate::{
    model::timestamp::Timestamp,
    thumbs::{decode::Decoded, error::ThumbError, size::ThumbSize},
};

/// What a thumbnail is of: a newer request for the same path with another
/// time or size replaces the older one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub path: Arc<Path>,
    pub modified: Timestamp,
    pub size: ThumbSize,
}

/// A finished thumbnail: straight-alpha RGBA8 rows, packed, ready for
/// `cx.register_image`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub key: ThumbKey,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub duration: Option<Duration>,
}

impl Thumbnail {
    pub fn new(key: ThumbKey, decoded: Decoded) -> Self {
        Self {
            key,
            width: decoded.frame.width,
            height: decoded.frame.height,
            rgba: decoded.frame.rgba,
            duration: decoded.duration,
        }
    }
}

#[derive(Debug)]
pub enum ThumbnailEvent {
    Ready(Thumbnail),
    /// No thumbnail can be made; the UI keeps the file's icon.
    Failed {
        key: ThumbKey,
        error: ThumbError,
    },
}
