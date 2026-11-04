//! What opening a file does.

use std::path::Path;

use crate::{
    model::{kind::FileKind, mime_id::MimeId},
    open::{desktop_entry::DesktopEntry, mimeapps::MimeApps},
};

/// The extensions crownos-preview's `DocumentKind::of` accepts.
const PREVIEW_EXTENSIONS: &str = "png jpg jpeg gif webp tif tiff bmp ico svg heic heif avif pdf \
    mp4 m4v mkv webm mov avi ogv mp3 flac ogg oga opus wav m4a aac md markdown";
const CODE_EXTENSIONS: &str = "rs py js mjs ts tsx jsx c h cc cpp hpp cs go java kt swift rb php \
    lua sh bash zsh fish toml json yaml yml xml html htm css scss sql ron ini conf diff patch \
    makefile cmake dockerfile zig hs ml ex exs erl clj scala pl r";
const PLAIN_TEXT_EXTENSIONS: &str = "txt log csv tsv text nfo srt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenRoute {
    /// A folder: show it in this window.
    Navigate,
    /// Quick look in crownos-preview.
    Preview,
    /// The type's default application.
    App(DesktopEntry),
    /// No application is known for the type: ask with the "Open With" chooser.
    ChooseApp(MimeId),
}

/// Folders navigate, what crownos-preview shows is previewed, the rest goes
/// to the default application or, without one, to the chooser. Blocking:
/// it reads desktop entries.
pub fn route(path: &Path, kind: FileKind, mime: MimeId, apps: &MimeApps) -> OpenRoute {
    if kind == FileKind::Folder {
        return OpenRoute::Navigate;
    }
    if is_previewable(path) {
        return OpenRoute::Preview;
    }
    let mime = if mime == MimeId::UNKNOWN {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        apps.db().mime_of_name(name)
    } else {
        mime
    };
    apps.default_app(mime)
        .map_or(OpenRoute::ChooseApp(mime), OpenRoute::App)
}

/// Whether crownos-preview can show `path`: a UTF-8 path, as its protocol
/// carries strings, with an extension it knows.
pub fn is_previewable(path: &Path) -> bool {
    if path.to_str().is_none() {
        return false;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            let extension = extension.to_ascii_lowercase();
            [PREVIEW_EXTENSIONS, CODE_EXTENSIONS, PLAIN_TEXT_EXTENSIONS]
                .iter()
                .any(|listed| listed.split_whitespace().any(|listed| listed == extension))
        })
}

/// The previewable files of `paths` in their order, for `open_list`, and
/// where `target` is among them.
pub fn preview_list<'a>(
    paths: impl IntoIterator<Item = &'a Path>,
    target: &Path,
) -> Option<(Vec<String>, u32)> {
    let list: Vec<String> = paths
        .into_iter()
        .filter(|path| is_previewable(path))
        .filter_map(|path| path.to_str().map(str::to_owned))
        .collect();
    let index = list.iter().position(|path| Path::new(path) == target)?;
    Some((list, u32::try_from(index).ok()?))
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    use super::*;

    #[test]
    fn previewable_matches_crownos_preview() {
        assert!(is_previewable(Path::new("/a/shot.PNG")));
        assert!(is_previewable(Path::new("/a/main.rs")));
        assert!(is_previewable(Path::new("/a/notes.txt")));
        assert!(is_previewable(Path::new("/a/clip.webm")));
        assert!(!is_previewable(Path::new("/a/archive.zip")));
        assert!(!is_previewable(Path::new("/a/Makefile")));
        assert!(!is_previewable(Path::new(OsStr::from_bytes(
            b"/a/\xFF.png"
        ))));
    }

    #[test]
    fn the_preview_list_skips_what_cannot_be_previewed() {
        let paths = [Path::new("/a.png"), Path::new("/b.zip"), Path::new("/c.md")];
        assert_eq!(
            preview_list(paths, Path::new("/c.md")),
            Some((vec!["/a.png".to_owned(), "/c.md".to_owned()], 1))
        );
        assert_eq!(preview_list(paths, Path::new("/b.zip")), None);
    }
}
