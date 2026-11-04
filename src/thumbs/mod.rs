//! Thumbnails for the visible pages, through the shared freedesktop cache.
//! Pixels come out as RGBA8 buffers for the UI to upload.

pub mod cache;
pub mod decode;
pub mod error;
pub mod frame;
pub mod key;
mod png;
pub mod prefetch;
pub mod queue;
pub mod size;
pub mod source;

pub use cache::{SourceStamp, ThumbCache};
pub use decode::{Decoded, ThumbFormat, decode};
pub use error::{CacheError, DecodeError, ThumbError};
pub use frame::Frame;
pub use key::{ThumbKey, Thumbnail, ThumbnailEvent};
pub use prefetch::prefetch_range;
pub use queue::{DECODE_WORKERS, ThumbnailService};
pub use size::ThumbSize;
pub use source::{CachedDecoder, ThumbnailSource};
