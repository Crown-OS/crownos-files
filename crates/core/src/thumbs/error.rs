use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot encode the thumbnail: {0}")]
    Encode(#[from] png::EncodingError),
    #[error("cannot decode the thumbnail: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("the thumbnail is malformed")]
    Malformed,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("no thumbnailer handles this file")]
    Unsupported,
    #[error("the file has nothing to show")]
    Empty,
    #[error("image: {0}")]
    Image(#[from] image::ImageError),
    #[error("jpeg: {0}")]
    Jpeg(#[from] turbojpeg::Error),
    #[error("svg: {0}")]
    Svg(#[from] resvg::usvg::Error),
    #[error("resize: {0}")]
    Resize(#[from] fast_image_resize::ResizeError),
    #[error("pdf: {0}")]
    Pdf(String),
    #[error("media: {0}")]
    Media(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ThumbError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("an earlier attempt to thumbnail this file failed")]
    PreviouslyFailed,
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error("the decode worker stopped: {0}")]
    Worker(#[from] tokio::task::JoinError),
}
