//! `file://` URIs and `text/uri-list` payloads (RFC 2483), byte-exact so names
//! that are not UTF-8 round-trip.

use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

const FILE_SCHEME: &[u8] = b"file://";
const LOCALHOST: &[u8] = b"localhost";
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

/// The paths in a `text/uri-list` payload, skipping blank lines and comments.
pub fn parse_uri_list(payload: &[u8]) -> impl Iterator<Item = PathBuf> + '_ {
    payload
        .split(|&byte| byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with(b"#"))
        .filter_map(uri_to_path)
}

/// The local path a `file://` URI names, or `None` for another scheme or host.
pub fn uri_to_path(uri: &[u8]) -> Option<PathBuf> {
    let rest = uri.strip_prefix(FILE_SCHEME)?;
    let path = match rest.iter().position(|&byte| byte == b'/') {
        Some(0) => rest,
        Some(slash) if &rest[..slash] == LOCALHOST => &rest[slash..],
        _ => return None,
    };
    Some(PathBuf::from(OsString::from_vec(percent_decode(path))))
}

pub fn path_to_uri(path: &Path) -> String {
    let bytes = path.as_os_str().as_bytes();
    let mut uri = String::with_capacity(FILE_SCHEME.len() + bytes.len());
    uri.push_str("file://");
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || b"-_.~/".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
            uri.push(char::from(HEX_DIGITS[usize::from(byte & 0xF)]));
        }
    }
    uri
}

/// One `file://` URI per line, CRLF-terminated, for the clipboard and drags.
pub fn uri_list<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    paths.into_iter().fold(String::new(), |mut list, path| {
        list.push_str(&path_to_uri(path));
        list.push_str("\r\n");
        list
    })
}

fn percent_decode(input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut rest = input;
    while let Some((&byte, tail)) = rest.split_first() {
        match (byte, tail) {
            (b'%', [high, low, after @ ..])
                if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() =>
            {
                decoded.push((hex_value(*high) << 4) | hex_value(*low));
                rest = after;
            }
            _ => {
                decoded.push(byte);
                rest = tail;
            }
        }
    }
    decoded
}

fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        _ => digit - b'A' + 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_escapes_decode_and_stray_percents_survive() {
        assert_eq!(percent_decode(b"/tmp/my%20file"), b"/tmp/my file");
        assert_eq!(percent_decode(b"/tmp/a%23b"), b"/tmp/a#b");
        assert_eq!(percent_decode(b"100%"), b"100%");
        assert_eq!(percent_decode(b"%zz"), b"%zz");
        assert_eq!(percent_decode(b"%4"), b"%4");
    }

    #[test]
    fn only_local_file_uris_become_paths() {
        assert_eq!(
            uri_to_path(b"file:///home/u/a.txt"),
            Some(PathBuf::from("/home/u/a.txt"))
        );
        assert_eq!(
            uri_to_path(b"file://localhost/tmp/x"),
            Some(PathBuf::from("/tmp/x"))
        );
        assert_eq!(uri_to_path(b"http://example.com/x"), None);
        assert_eq!(uri_to_path(b"file://otherhost/tmp/x"), None);
    }

    #[test]
    fn a_uri_list_skips_blanks_and_comments() {
        let payload = b"# comment\r\nfile:///a\r\n\r\nfile:///b%20c\r\n";
        let paths: Vec<_> = parse_uri_list(payload).collect();
        assert_eq!(paths, [PathBuf::from("/a"), PathBuf::from("/b c")]);
    }

    #[test]
    fn paths_round_trip_including_non_utf8_names() {
        let non_utf8 = PathBuf::from(OsString::from_vec(b"/tmp/\xFFname".to_vec()));
        let paths = [
            PathBuf::from("/tmp/my file.txt"),
            PathBuf::from("/tmp/a#b"),
            PathBuf::from("/tmp/héllo"),
            PathBuf::from("/tmp/100%"),
            non_utf8,
        ];
        let list = uri_list(paths.iter().map(PathBuf::as_path));
        assert_eq!(parse_uri_list(list.as_bytes()).collect::<Vec<_>>(), paths);
    }
}
