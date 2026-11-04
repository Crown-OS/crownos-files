use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use rustix::{
    buffer::spare_capacity,
    fs::{XattrFlags, getxattr, getxattr_query, removexattr, setxattr},
    io::Errno,
};

use crate::tags::{
    codec::{Tags, decode_tags, encode_tags},
    error::TagError,
};

pub const TAGS_ATTRIBUTE: &str = "user.xdg.tags";

const INITIAL_CAPACITY: usize = 256;

/// Reads tags with one reusable buffer, for listing many files.
#[derive(Debug, Default)]
pub struct TagReader {
    buffer: Vec<u8>,
}

impl TagReader {
    /// The tags of `path`; none when it has no tags or its filesystem
    /// keeps no user attributes.
    pub fn read(&mut self, path: &Path) -> Result<Tags, TagError> {
        loop {
            self.buffer.clear();
            self.buffer.reserve(INITIAL_CAPACITY);
            match getxattr(path, TAGS_ATTRIBUTE, spare_capacity(&mut self.buffer)) {
                Ok(_) => return Ok(decode_tags(&self.buffer)),
                Err(Errno::NODATA | Errno::NOTSUP) => return Ok(Tags::new()),
                Err(Errno::RANGE) => {
                    let needed = getxattr_query(path, TAGS_ATTRIBUTE).map_err(io(path))?;
                    self.buffer.reserve(needed.max(self.buffer.capacity() * 2));
                }
                Err(error) => return Err(io(path)(error)),
            }
        }
    }
}

pub fn read_tags(path: &Path) -> Result<Tags, TagError> {
    TagReader::default().read(path)
}

/// The tags of each of `names` in `dir`, in order; unreadable ones have none.
pub fn read_tags_in<'a>(dir: &Path, names: impl IntoIterator<Item = &'a OsStr>) -> Vec<Tags> {
    let mut reader = TagReader::default();
    let mut path = dir.to_path_buf();
    names
        .into_iter()
        .map(|name| {
            path.push(name);
            let tags = reader.read(&path).unwrap_or_default();
            path.pop();
            tags
        })
        .collect()
}

/// Replaces the tags of `path`; no tags removes the attribute.
pub fn write_tags<S: AsRef<str>>(path: &Path, tags: &[S]) -> Result<(), TagError> {
    let written = if tags.is_empty() {
        match removexattr(path, TAGS_ATTRIBUTE) {
            Err(Errno::NODATA) => Ok(()),
            other => other,
        }
    } else {
        setxattr(
            path,
            TAGS_ATTRIBUTE,
            encode_tags(tags).as_bytes(),
            XattrFlags::empty(),
        )
    };
    written.map_err(|error| match error {
        Errno::NOTSUP => TagError::Unsupported(path.to_path_buf()),
        error => io(path)(error),
    })
}

pub fn add_tag(path: &Path, tag: &str) -> Result<Tags, TagError> {
    update(path, |tags| {
        if !tags.iter().any(|known| known == tag) {
            tags.push(tag.into());
        }
    })
}

pub fn remove_tag(path: &Path, tag: &str) -> Result<Tags, TagError> {
    update(path, |tags| tags.retain(|known| known != tag))
}

/// Adds `tag` when `path` lacks it, removes it otherwise.
pub fn toggle_tag(path: &Path, tag: &str) -> Result<Tags, TagError> {
    update(path, |tags| {
        match tags.iter().position(|known| known == tag) {
            Some(index) => {
                tags.remove(index);
            }
            None => tags.push(tag.into()),
        }
    })
}

fn update(path: &Path, change: impl FnOnce(&mut Tags)) -> Result<Tags, TagError> {
    let mut tags = read_tags(path)?;
    let before = tags.clone();
    change(&mut tags);
    if tags != before {
        write_tags(path, &tags)?;
    }
    Ok(tags)
}

fn io(path: &Path) -> impl Fn(Errno) -> TagError + '_ {
    move |error| TagError::Io {
        path: PathBuf::from(path),
        source: error.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary file on a filesystem with user attributes, if there is one.
    fn taggable() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let file = dir.path().join("note.txt");
        std::fs::write(&file, b"x").ok()?;
        match write_tags(&file, &["probe"]) {
            Ok(()) => Some((dir, file)),
            Err(TagError::Unsupported(_)) => None,
            Err(error) => panic!("{error}"),
        }
    }

    #[test]
    fn tags_round_trip_through_the_attribute() {
        let Some((dir, file)) = taggable() else {
            eprintln!("skipped: no user xattrs here");
            return;
        };
        write_tags(&file, &["work", "a,b"]).unwrap();
        assert_eq!(read_tags(&file).unwrap(), ["work", "a,b"]);
        assert_eq!(add_tag(&file, "urgent").unwrap(), ["work", "a,b", "urgent"]);
        assert_eq!(toggle_tag(&file, "work").unwrap(), ["a,b", "urgent"]);
        assert_eq!(
            toggle_tag(&file, "work").unwrap(),
            ["a,b", "urgent", "work"]
        );
        assert_eq!(remove_tag(&file, "a,b").unwrap(), ["urgent", "work"]);
        let long: Vec<String> = (0..100)
            .map(|index| format!("tag number {index}"))
            .collect();
        write_tags(&file, &long).unwrap();
        assert_eq!(read_tags(&file).unwrap(), long);
        std::fs::write(dir.path().join("plain"), b"").unwrap();
        let names = [
            OsStr::new("note.txt"),
            OsStr::new("plain"),
            OsStr::new("missing"),
        ];
        let batch = read_tags_in(dir.path(), names);
        assert_eq!(batch[0].len(), 100);
        assert!(batch[1].is_empty() && batch[2].is_empty());
        write_tags::<&str>(&file, &[]).unwrap();
        write_tags::<&str>(&file, &[]).unwrap();
        assert!(read_tags(&file).unwrap().is_empty());
    }
}
