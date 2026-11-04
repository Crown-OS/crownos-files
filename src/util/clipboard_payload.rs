//! Clipboard payloads for files: `text/uri-list`, GNOME's
//! `x-special/gnome-copied-files` (`copy`/`cut` then one URI per line) and a
//! `text/plain` fallback.

use std::{
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use crate::util::uri::{parse_uri_list, path_to_uri, uri_list};

pub const URI_LIST_MIME: &str = "text/uri-list";
pub const GNOME_COPIED_FILES_MIME: &str = "x-special/gnome-copied-files";
pub const PLAIN_TEXT_MIME: &str = "text/plain;charset=utf-8";

const COPY_VERB: &str = "copy";
const CUT_VERB: &str = "cut";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAction {
    Copy,
    Cut,
}

impl ClipboardAction {
    fn verb(self) -> &'static str {
        match self {
            Self::Copy => COPY_VERB,
            Self::Cut => CUT_VERB,
        }
    }
}

pub fn encode_uri_list<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    uri_list(paths)
}

pub fn decode_uri_list(payload: &[u8]) -> Vec<PathBuf> {
    parse_uri_list(payload).collect()
}

pub fn encode_gnome_copied_files<'a>(
    action: ClipboardAction,
    paths: impl IntoIterator<Item = &'a Path>,
) -> String {
    paths
        .into_iter()
        .fold(action.verb().to_owned(), |mut payload, path| {
            payload.push('\n');
            payload.push_str(&path_to_uri(path));
            payload
        })
}

pub fn decode_gnome_copied_files(payload: &[u8]) -> Option<(ClipboardAction, Vec<PathBuf>)> {
    let (verb, uris) = match memchr::memchr(b'\n', payload) {
        Some(newline) => (&payload[..newline], &payload[newline + 1..]),
        None => (payload, &[][..]),
    };
    let action = match verb.strip_suffix(b"\r").unwrap_or(verb) {
        verb if verb == COPY_VERB.as_bytes() => ClipboardAction::Copy,
        verb if verb == CUT_VERB.as_bytes() => ClipboardAction::Cut,
        _ => return None,
    };
    Some((action, decode_uri_list(uris)))
}

/// Paths one per line, for pasting into text fields.
pub fn encode_plain_text<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Vec<u8> {
    let mut text = Vec::new();
    for path in paths {
        if !text.is_empty() {
            text.push(b'\n');
        }
        text.extend_from_slice(path.as_os_str().as_bytes());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> [PathBuf; 2] {
        [PathBuf::from("/home/u/a b.txt"), PathBuf::from("/tmp/x#y")]
    }

    #[test]
    fn gnome_payloads_round_trip_both_actions() {
        let paths = sample();
        for action in [ClipboardAction::Copy, ClipboardAction::Cut] {
            let payload = encode_gnome_copied_files(action, paths.iter().map(PathBuf::as_path));
            assert!(payload.starts_with(action.verb()));
            assert!(!payload.ends_with('\n'));
            assert_eq!(
                decode_gnome_copied_files(payload.as_bytes()),
                Some((action, paths.to_vec()))
            );
        }
    }

    #[test]
    fn gnome_payloads_reject_unknown_verbs_and_accept_crlf() {
        assert_eq!(decode_gnome_copied_files(b"move\nfile:///a"), None);
        assert_eq!(
            decode_gnome_copied_files(b"cut\r\nfile:///a\r\n"),
            Some((ClipboardAction::Cut, vec![PathBuf::from("/a")]))
        );
        assert_eq!(
            decode_gnome_copied_files(b"copy"),
            Some((ClipboardAction::Copy, Vec::new()))
        );
    }

    #[test]
    fn uri_lists_and_plain_text_round_trip() {
        let paths = sample();
        let list = encode_uri_list(paths.iter().map(PathBuf::as_path));
        assert_eq!(decode_uri_list(list.as_bytes()), paths);
        let text = encode_plain_text(paths.iter().map(PathBuf::as_path));
        assert_eq!(text, b"/home/u/a b.txt\n/tmp/x#y");
    }
}
