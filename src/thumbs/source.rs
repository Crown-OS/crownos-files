use std::fs;

use tokio_util::sync::CancellationToken;

use crate::{
    thumbs::{
        cache::{SourceStamp, ThumbCache},
        decode::{Decoded, decode},
        error::{DecodeError, ThumbError},
        key::ThumbKey,
    },
    util::uri::glib_file_uri,
};

/// Where the thumbnail service gets pixels from. Called on a blocking thread.
pub trait ThumbnailSource: Send + Sync + 'static {
    fn produce(&self, key: &ThumbKey, cancel: &CancellationToken) -> Result<Decoded, ThumbError>;
}

/// The shared freedesktop cache in front of the decoders: a valid cached
/// PNG is loaded without touching the original, a fresh thumbnail is
/// written back, and a file that cannot be decoded is remembered as failed.
#[derive(Debug, Clone)]
pub struct CachedDecoder {
    cache: ThumbCache,
}

impl CachedDecoder {
    pub fn new(cache: ThumbCache) -> Self {
        Self { cache }
    }
}

impl ThumbnailSource for CachedDecoder {
    fn produce(&self, key: &ThumbKey, cancel: &CancellationToken) -> Result<Decoded, ThumbError> {
        let path = &*key.path;
        let metadata = fs::metadata(path).map_err(|source| ThumbError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let stamp = SourceStamp::of(&metadata);
        let uri = glib_file_uri(path);
        if let Some(cached) = self.cache.load(&uri, stamp, key.size) {
            return Ok(Decoded {
                frame: cached.frame,
                duration: cached.duration,
            });
        }
        if self.cache.has_failed(&uri, stamp) {
            return Err(ThumbError::PreviouslyFailed);
        }
        if cancel.is_cancelled() {
            return Err(ThumbError::Cancelled);
        }
        let decoded = match decode(path, key.size.edge()) {
            Ok(decoded) => decoded,
            Err(error @ (DecodeError::Unsupported | DecodeError::Io { .. })) => {
                return Err(error.into());
            }
            Err(error) => {
                self.cache.mark_failed(&uri, stamp).ok();
                return Err(error.into());
            }
        };
        if !self.cache.holds(path) {
            self.cache
                .store(&uri, stamp, key.size, &decoded.frame, decoded.duration)
                .ok();
        }
        Ok(decoded)
    }
}
