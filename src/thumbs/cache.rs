//! The shared freedesktop thumbnail cache:
//! `$XDG_CACHE_HOME/thumbnails/<size>/<md5 of the file URI>.png`, valid while
//! its `Thumb::MTime` (and `Thumb::Size`, when present) match the file.

use std::{
    fs::{DirBuilder, Metadata},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    time::Duration,
};

use md5::{Digest, Md5};

use crate::{
    thumbs::{
        error::CacheError,
        frame::Frame,
        png::{PngThumb, write_png},
        size::ThumbSize,
    },
    util::{atomic_write::write_atomically, hex::to_lower_hex, xdg_dirs::XdgDirs},
};

const THUMBNAILS_DIR: &str = "thumbnails";
const FAIL_DIR: &str = "fail";
const APP_FAIL_DIR: &str = "crownos-files";
const PNG_EXTENSION: &str = ".png";
const KEY_URI: &str = "Thumb::URI";
const KEY_MTIME: &str = "Thumb::MTime";
const KEY_SIZE: &str = "Thumb::Size";
const KEY_MOVIE_LENGTH: &str = "Thumb::Movie::Length";
const KEY_SOFTWARE: &str = "Software";
const SOFTWARE: &str = "CrownOS Files";
const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// What a thumbnail must have been made from to still be valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceStamp {
    pub mtime: i64,
    pub len: u64,
}

impl SourceStamp {
    pub fn of(metadata: &Metadata) -> Self {
        Self {
            mtime: metadata.mtime(),
            len: metadata.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedThumb {
    pub frame: Frame,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbCache {
    root: PathBuf,
}

impl ThumbCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_xdg(xdg: &XdgDirs) -> Self {
        Self::new(xdg.cache_home.join(THUMBNAILS_DIR))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether `path` lies inside the cache, whose files are never thumbnailed into it.
    pub fn holds(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    pub fn thumb_path(&self, uri: &str, size: ThumbSize) -> PathBuf {
        self.root.join(size.dir_name()).join(thumb_file_name(uri))
    }

    pub fn fail_path(&self, uri: &str) -> PathBuf {
        self.root
            .join(FAIL_DIR)
            .join(APP_FAIL_DIR)
            .join(thumb_file_name(uri))
    }

    /// The cached thumbnail of `uri` if it is still valid; a stale, missing
    /// or unreadable one is a miss.
    pub fn load(&self, uri: &str, stamp: SourceStamp, size: ThumbSize) -> Option<CachedThumb> {
        let thumb = PngThumb::open(&self.thumb_path(uri, size)).ok()?;
        if !describes(&thumb, uri, stamp) {
            return None;
        }
        let duration = thumb
            .text(KEY_MOVIE_LENGTH)
            .and_then(|seconds| seconds.parse().ok())
            .map(Duration::from_secs);
        Some(CachedThumb {
            frame: thumb.into_frame().ok()?,
            duration,
        })
    }

    pub fn store(
        &self,
        uri: &str,
        stamp: SourceStamp,
        size: ThumbSize,
        frame: &Frame,
        duration: Option<Duration>,
    ) -> Result<(), CacheError> {
        self.write(&self.thumb_path(uri, size), uri, stamp, frame, duration)
    }

    /// Records that `uri` cannot be thumbnailed until it changes.
    pub fn mark_failed(&self, uri: &str, stamp: SourceStamp) -> Result<(), CacheError> {
        let placeholder = Frame::new(1, 1, vec![0; 4]).ok_or(CacheError::Malformed)?;
        self.write(&self.fail_path(uri), uri, stamp, &placeholder, None)
    }

    pub fn has_failed(&self, uri: &str, stamp: SourceStamp) -> bool {
        PngThumb::open(&self.fail_path(uri)).is_ok_and(|thumb| describes(&thumb, uri, stamp))
    }

    fn write(
        &self,
        path: &Path,
        uri: &str,
        stamp: SourceStamp,
        frame: &Frame,
        duration: Option<Duration>,
    ) -> Result<(), CacheError> {
        let io_error = |source| CacheError::Io {
            path: path.to_path_buf(),
            source,
        };
        let mtime = stamp.mtime.to_string();
        let len = stamp.len.to_string();
        let length = duration.map(|duration| duration.as_secs().to_string());
        let mut chunks = vec![
            (KEY_URI, uri),
            (KEY_MTIME, mtime.as_str()),
            (KEY_SIZE, len.as_str()),
            (KEY_SOFTWARE, SOFTWARE),
        ];
        chunks.extend(length.as_deref().map(|length| (KEY_MOVIE_LENGTH, length)));
        let mut png = Vec::new();
        write_png(&mut png, frame, &chunks)?;
        if let Some(dir) = path.parent() {
            DirBuilder::new()
                .recursive(true)
                .mode(DIR_MODE)
                .create(dir)
                .map_err(io_error)?;
        }
        write_atomically(path, FILE_MODE, |out| out.write_all(&png)).map_err(io_error)
    }
}

/// `md5(uri)` in lowercase hex plus `.png`, the spec's cache file name.
pub fn thumb_file_name(uri: &str) -> String {
    let mut name = to_lower_hex(&Md5::digest(uri.as_bytes()));
    name.push_str(PNG_EXTENSION);
    name
}

fn describes(thumb: &PngThumb, uri: &str, stamp: SourceStamp) -> bool {
    let same_size = thumb
        .text(KEY_SIZE)
        .is_none_or(|len| len.parse() == Ok(stamp.len));
    thumb.text(KEY_URI) == Some(uri)
        && thumb.text(KEY_MTIME).and_then(|mtime| mtime.parse().ok()) == Some(stamp.mtime)
        && same_size
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    const STAMP: SourceStamp = SourceStamp {
        mtime: 1_700_000_000,
        len: 1234,
    };

    #[test]
    fn file_names_are_the_md5_of_the_uri() {
        assert_eq!(
            thumb_file_name("file:///home/jens/photos/me.png"),
            "c6ee772d9e49320e97ec29a7eb5b1697.png"
        );
        assert_eq!(thumb_file_name(""), "d41d8cd98f00b204e9800998ecf8427e.png");
    }

    #[test]
    fn paths_follow_the_size_directories() {
        let cache = ThumbCache::new("/c/thumbnails");
        let uri = "file:///home/jens/photos/me.png";
        assert_eq!(
            cache.thumb_path(uri, ThumbSize::Large),
            Path::new("/c/thumbnails/large/c6ee772d9e49320e97ec29a7eb5b1697.png")
        );
        assert_eq!(
            cache.fail_path(uri),
            Path::new("/c/thumbnails/fail/crownos-files/c6ee772d9e49320e97ec29a7eb5b1697.png")
        );
        assert!(cache.holds(Path::new("/c/thumbnails/normal/x.png")));
        assert!(!cache.holds(Path::new("/c/other.png")));
    }

    #[test]
    fn a_stored_thumbnail_is_valid_only_for_the_same_file_state() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::new(dir.path().join("thumbnails"));
        let uri = "file:///photos/a.mp4";
        let frame = Frame::new(1, 2, vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap();
        cache
            .store(
                uri,
                STAMP,
                ThumbSize::Normal,
                &frame,
                Some(Duration::from_secs(95)),
            )
            .unwrap();

        let hit = cache.load(uri, STAMP, ThumbSize::Normal).unwrap();
        assert_eq!(hit.frame, frame);
        assert_eq!(hit.duration, Some(Duration::from_secs(95)));

        let touched = SourceStamp {
            mtime: STAMP.mtime + 1,
            ..STAMP
        };
        let resized = SourceStamp { len: 99, ..STAMP };
        assert!(cache.load(uri, touched, ThumbSize::Normal).is_none());
        assert!(cache.load(uri, resized, ThumbSize::Normal).is_none());
        assert!(
            cache
                .load("file:///photos/b.mp4", STAMP, ThumbSize::Normal)
                .is_none()
        );
        assert!(cache.load(uri, STAMP, ThumbSize::Large).is_none());

        let stored = cache.thumb_path(uri, ThumbSize::Normal);
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&stored), FILE_MODE);
        assert_eq!(mode(stored.parent().unwrap()), DIR_MODE);
    }

    #[test]
    fn failures_are_remembered_until_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::new(dir.path());
        let uri = "file:///broken.png";
        assert!(!cache.has_failed(uri, STAMP));
        cache.mark_failed(uri, STAMP).unwrap();
        assert!(cache.has_failed(uri, STAMP));
        let changed = SourceStamp {
            mtime: STAMP.mtime + 5,
            ..STAMP
        };
        assert!(!cache.has_failed(uri, changed));
    }
}
