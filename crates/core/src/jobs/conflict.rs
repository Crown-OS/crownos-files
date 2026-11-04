use std::{
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::{self, ErrorKind},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

const FIRST_DUPLICATE: u32 = 2;
const COMPOUND_EXTENSION_STEMS: &[&[u8]] = &[b".tar"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConflictPolicy {
    /// Overwrite files; merge folders into folders.
    Replace,
    Skip,
    /// Keep the existing entry and give the new one a " (2)"-style name.
    KeepBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictReply {
    pub policy: ConflictPolicy,
    pub apply_to_all: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    File,
    Dir,
}

/// Creates an empty file or folder named `name` in `dir`, or the first free
/// "name (2)", "name (3)"... and returns its path. Creation is the claim, so
/// concurrent callers never get the same path.
pub fn unique_name(dir: &Path, name: &OsStr, claim: Claim) -> io::Result<PathBuf> {
    let (path, ()) = claim_unique(dir, name, claim == Claim::Dir, |path| match claim {
        Claim::File => OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(drop),
        Claim::Dir => fs::create_dir(path),
    })?;
    Ok(path)
}

/// Calls `claim` on `name` and then on numbered variants of it until one does
/// not fail with `AlreadyExists`. Folder names are numbered at the end, file
/// names before their extension ("photo (2).jpg", "archive (2).tar.gz").
pub(crate) fn claim_unique<T>(
    dir: &Path,
    name: &OsStr,
    is_dir: bool,
    mut claim: impl FnMut(&Path) -> io::Result<T>,
) -> io::Result<(PathBuf, T)> {
    let (stem, extension) = split_extension(name.as_bytes(), is_dir);
    let (base, counter) = split_counter(stem);
    let mut candidate = dir.join(name);
    for next in counter.saturating_add(1).. {
        match claim(&candidate) {
            Ok(claimed) => return Ok((candidate, claimed)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        candidate = dir.join(numbered(base, next, extension));
    }
    Err(ErrorKind::AlreadyExists.into())
}

fn split_extension(name: &[u8], is_dir: bool) -> (&[u8], &[u8]) {
    let dot = match memchr::memrchr(b'.', name) {
        Some(dot) if !is_dir && dot > 0 && dot + 1 < name.len() => dot,
        _ => return (name, &[]),
    };
    let stem = &name[..dot];
    let dot = match memchr::memrchr(b'.', stem) {
        Some(inner) if inner > 0 && COMPOUND_EXTENSION_STEMS.contains(&&stem[inner..]) => inner,
        _ => dot,
    };
    name.split_at(dot)
}

/// "photo (4)" is "photo" numbered 4; anything else is numbered 1.
fn split_counter(stem: &[u8]) -> (&[u8], u32) {
    let numbered = stem
        .strip_suffix(b")")
        .and_then(|rest| {
            let open = memchr::memrchr(b'(', rest)?;
            let base = rest[..open].strip_suffix(b" ")?;
            let counter = std::str::from_utf8(&rest[open + 1..]).ok()?.parse().ok()?;
            Some((base, counter))
        })
        .filter(|&(base, counter)| !base.is_empty() && counter >= FIRST_DUPLICATE);
    numbered.unwrap_or((stem, 1))
}

fn numbered(base: &[u8], counter: u32, extension: &[u8]) -> OsString {
    let mut name = base.to_vec();
    name.extend_from_slice(format!(" ({counter})").as_bytes());
    name.extend_from_slice(extension);
    OsString::from_vec(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_names(name: &str, is_dir: bool, taken: usize) -> Vec<String> {
        let mut seen = Vec::new();
        let (path, ()) = claim_unique(Path::new("/d"), OsStr::new(name), is_dir, |path| {
            seen.push(path.file_name().unwrap().to_string_lossy().into_owned());
            if seen.len() <= taken {
                Err(ErrorKind::AlreadyExists.into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(path.file_name().unwrap().to_string_lossy(), seen[taken]);
        seen
    }

    #[test]
    fn numbers_go_before_the_extension_of_files_only() {
        assert_eq!(
            next_names("photo.jpg", false, 2),
            ["photo.jpg", "photo (2).jpg", "photo (3).jpg"]
        );
        assert_eq!(
            next_names("archive.tar.gz", false, 1),
            ["archive.tar.gz", "archive (2).tar.gz"]
        );
        assert_eq!(
            next_names("my.folder", true, 1),
            ["my.folder", "my.folder (2)"]
        );
        assert_eq!(next_names(".bashrc", false, 1), [".bashrc", ".bashrc (2)"]);
        assert_eq!(next_names("notes.", false, 1), ["notes.", "notes. (2)"]);
    }

    #[test]
    fn an_existing_number_is_continued() {
        assert_eq!(
            next_names("photo (2).jpg", false, 1),
            ["photo (2).jpg", "photo (3).jpg"]
        );
        assert_eq!(next_names("(1)", false, 1), ["(1)", "(1) (2)"]);
        assert_eq!(next_names("a (x)", false, 1), ["a (x)", "a (x) (2)"]);
    }

    #[test]
    fn other_errors_stop_the_search() {
        let result = claim_unique(Path::new("/d"), OsStr::new("a"), false, |_| {
            Err::<(), _>(io::Error::from(ErrorKind::PermissionDenied))
        });
        assert_eq!(result.unwrap_err().kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn unique_name_claims_on_disk() {
        let temp = tempfile::tempdir().unwrap();
        let name = OsStr::new("new.txt");
        let first = unique_name(temp.path(), name, Claim::File).unwrap();
        let second = unique_name(temp.path(), name, Claim::File).unwrap();
        let folder = unique_name(temp.path(), name, Claim::Dir).unwrap();
        assert_eq!(first, temp.path().join("new.txt"));
        assert_eq!(second, temp.path().join("new (2).txt"));
        assert_eq!(folder, temp.path().join("new.txt (2)"));
        assert!(folder.is_dir() && second.is_file());
    }
}
