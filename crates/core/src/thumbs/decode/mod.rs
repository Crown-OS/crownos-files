//! Blocking decoders that produce a thumbnail no longer than a given edge.

mod format;
mod jpeg;
mod orient;
mod raster;
mod resize;
mod svg;

#[cfg(feature = "thumbs-video")]
mod audio;
#[cfg(feature = "thumbs-video")]
mod media;
#[cfg(feature = "thumbs-pdf")]
mod pdf;
#[cfg(feature = "thumbs-video")]
mod video;

use std::{path::Path, time::Duration};

pub use format::ThumbFormat;
pub use raster::decode_bytes as decode_image_bytes;
pub use resize::fit_within;

use crate::thumbs::{error::DecodeError, frame::Frame};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub frame: Frame,
    /// The running time of a video or song, for the duration badge.
    pub duration: Option<Duration>,
}

impl From<Frame> for Decoded {
    fn from(frame: Frame) -> Self {
        Self {
            frame,
            duration: None,
        }
    }
}

/// A thumbnail of `path` whose longest edge is at most `edge`. Blocking:
/// run it on a blocking thread.
pub fn decode(path: &Path, edge: u32) -> Result<Decoded, DecodeError> {
    let format = ThumbFormat::of(path).ok_or(DecodeError::Unsupported)?;
    decode_as(path, format, edge)
}

pub fn decode_as(path: &Path, format: ThumbFormat, edge: u32) -> Result<Decoded, DecodeError> {
    match format {
        ThumbFormat::Jpeg => jpeg::decode_file(path, edge).map(Decoded::from),
        ThumbFormat::Raster => raster::decode_file(path, edge).map(Decoded::from),
        ThumbFormat::Svg => svg::decode_file(path, edge).map(Decoded::from),
        #[cfg(feature = "thumbs-pdf")]
        ThumbFormat::Pdf => pdf::decode_file(path, edge).map(Decoded::from),
        #[cfg(feature = "thumbs-video")]
        ThumbFormat::Video => video::decode_file(path, edge),
        #[cfg(feature = "thumbs-video")]
        ThumbFormat::Audio => audio::decode_file(path, edge),
    }
}

fn read_error(path: &Path) -> impl FnOnce(std::io::Error) -> DecodeError + '_ {
    move |source| DecodeError::Io {
        path: path.to_path_buf(),
        source,
    }
}
