use std::path::Path;

use crate::{mime::MimeDatabase, model::mime_id::MimeId};

const JPEG_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "jfif"];
const RASTER_EXTENSIONS: &[&str] = &["png", "gif", "webp", "bmp", "tif", "tiff", "ico"];
const SVG_EXTENSIONS: &[&str] = &["svg", "svgz"];
#[cfg(feature = "thumbs-pdf")]
const PDF_EXTENSIONS: &[&str] = &["pdf"];
#[cfg(feature = "thumbs-video")]
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "ogv", "wmv", "flv", "mpg", "mpeg", "3gp", "ts",
];
#[cfg(feature = "thumbs-video")]
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "ogg", "oga", "opus", "aac", "wma"];
const RASTER_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/tiff",
    "image/vnd.microsoft.icon",
];

/// Which decoder makes the thumbnail of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbFormat {
    Jpeg,
    Raster,
    Svg,
    #[cfg(feature = "thumbs-pdf")]
    Pdf,
    #[cfg(feature = "thumbs-video")]
    Video,
    #[cfg(feature = "thumbs-video")]
    Audio,
}

impl ThumbFormat {
    /// By extension, then by the type the name implies, and by content
    /// only when the name says nothing.
    pub fn of(path: &Path) -> Option<Self> {
        if let Some(format) = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
        {
            return Some(format);
        }
        let db = MimeDatabase::system();
        let by_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| db.mime_of_name(name))
            .filter(|&mime| mime != MimeId::UNKNOWN);
        let mime = match by_name {
            Some(mime) => mime,
            None => db.sniff(path).ok()?,
        };
        Self::from_mime(mime.name()?)
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        let listed = |extensions: &[&str]| {
            extensions
                .iter()
                .any(|listed| listed.eq_ignore_ascii_case(extension))
        };
        if listed(JPEG_EXTENSIONS) {
            return Some(Self::Jpeg);
        }
        if listed(RASTER_EXTENSIONS) {
            return Some(Self::Raster);
        }
        if listed(SVG_EXTENSIONS) {
            return Some(Self::Svg);
        }
        #[cfg(feature = "thumbs-pdf")]
        if listed(PDF_EXTENSIONS) {
            return Some(Self::Pdf);
        }
        #[cfg(feature = "thumbs-video")]
        if listed(VIDEO_EXTENSIONS) {
            return Some(Self::Video);
        }
        #[cfg(feature = "thumbs-video")]
        if listed(AUDIO_EXTENSIONS) {
            return Some(Self::Audio);
        }
        None
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "image/jpeg" => Some(Self::Jpeg),
            "image/svg+xml" | "image/svg+xml-compressed" => Some(Self::Svg),
            #[cfg(feature = "thumbs-pdf")]
            "application/pdf" => Some(Self::Pdf),
            raster if RASTER_MIME_TYPES.contains(&raster) => Some(Self::Raster),
            #[cfg(feature = "thumbs-video")]
            video if video.starts_with("video/") => Some(Self::Video),
            #[cfg(feature = "thumbs-video")]
            audio if audio.starts_with("audio/") => Some(Self::Audio),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_mime_types_pick_a_decoder() {
        assert_eq!(ThumbFormat::from_extension("JPG"), Some(ThumbFormat::Jpeg));
        assert_eq!(
            ThumbFormat::from_extension("webp"),
            Some(ThumbFormat::Raster)
        );
        assert_eq!(ThumbFormat::from_extension("svgz"), Some(ThumbFormat::Svg));
        assert_eq!(ThumbFormat::from_extension("txt"), None);
        assert_eq!(
            ThumbFormat::from_mime("image/png"),
            Some(ThumbFormat::Raster)
        );
        assert_eq!(ThumbFormat::from_mime("text/plain"), None);
    }
}
