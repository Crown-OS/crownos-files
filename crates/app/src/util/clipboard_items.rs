//! Files on the clipboard and in drags, in every MIME type file managers read.

use std::path::{Path, PathBuf};

use crate::util::clipboard_payload::{
    ClipboardAction, GNOME_COPIED_FILES_MIME, PLAIN_TEXT_MIME, URI_LIST_MIME,
    decode_gnome_copied_files, decode_uri_list, encode_gnome_copied_files, encode_plain_text,
    encode_uri_list,
};

/// The types a paste or a drop reads, best first.
pub const FILE_MIME_TYPES: [&str; 2] = [GNOME_COPIED_FILES_MIME, URI_LIST_MIME];

/// `paths` as `text/uri-list`, `x-special/gnome-copied-files` and plain text.
pub fn file_items(action: ClipboardAction, paths: &[PathBuf]) -> Vec<(String, Vec<u8>)> {
    let each = || paths.iter().map(PathBuf::as_path);
    vec![
        (
            URI_LIST_MIME.to_owned(),
            encode_uri_list(each()).into_bytes(),
        ),
        (
            GNOME_COPIED_FILES_MIME.to_owned(),
            encode_gnome_copied_files(action, each()).into_bytes(),
        ),
        (PLAIN_TEXT_MIME.to_owned(), encode_plain_text(each())),
    ]
}

/// The files `data` of `mime_type` names, and whether they were cut.
pub fn read_files(mime_type: &str, data: &[u8]) -> Option<(ClipboardAction, Vec<PathBuf>)> {
    let (action, paths) = if mime_type == GNOME_COPIED_FILES_MIME {
        decode_gnome_copied_files(data)?
    } else {
        (ClipboardAction::Copy, decode_uri_list(data))
    };
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| path.is_absolute())
        .collect();
    (!paths.is_empty()).then_some((action, paths))
}

/// Whether every path in `paths` already sits directly in `dir`.
pub fn all_in(paths: &[PathBuf], dir: &Path) -> bool {
    paths.iter().all(|path| path.parent() == Some(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_reads_back_as_a_cut_and_a_plain_uri_list_as_a_copy() {
        let paths = vec![PathBuf::from("/home/u/a b.txt"), PathBuf::from("/tmp/c")];
        let items = file_items(ClipboardAction::Cut, &paths);
        let mime_types: Vec<&str> = items.iter().map(|(mime, _)| mime.as_str()).collect();
        assert_eq!(
            mime_types,
            [URI_LIST_MIME, GNOME_COPIED_FILES_MIME, PLAIN_TEXT_MIME]
        );
        assert_eq!(
            read_files(&items[1].0, &items[1].1),
            Some((ClipboardAction::Cut, paths.clone()))
        );
        assert_eq!(
            read_files(&items[0].0, &items[0].1),
            Some((ClipboardAction::Copy, paths))
        );
        assert_eq!(read_files(URI_LIST_MIME, b""), None);
    }
}
