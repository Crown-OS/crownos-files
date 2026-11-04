use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use compact_str::CompactString;

use crate::{
    model::{kind::FileKind, mime_id::MimeId, stat::Stat},
    util::extension::extension,
};

/// The rarely needed parts of an entry, allocated only when present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraInfo {
    pub link_target: Option<PathBuf>,
    /// The on-disk name when it is not UTF-8; `Entry::name` is then lossy.
    pub raw_name: Option<Box<OsStr>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryExtra(Option<Box<ExtraInfo>>);

impl EntryExtra {
    pub fn new(link_target: Option<PathBuf>, raw_name: Option<Box<OsStr>>) -> Self {
        if link_target.is_none() && raw_name.is_none() {
            return Self::default();
        }
        Self(Some(Box::new(ExtraInfo {
            link_target,
            raw_name,
        })))
    }

    pub fn info(&self) -> Option<&ExtraInfo> {
        self.0.as_deref()
    }
}

/// One child of a [`Folder`](crate::model::folder::Folder). The parent path is
/// stored once by the folder, so an entry only carries its own name, which
/// is stored inline up to 24 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: CompactString,
    pub stat: Stat,
    pub kind: FileKind,
    pub mime: MimeId,
    pub extra: EntryExtra,
}

impl Entry {
    pub fn new(name: CompactString, stat: Stat, extra: EntryExtra) -> Self {
        Self {
            kind: FileKind::classify(&name, stat.is_dir()),
            name,
            stat,
            mime: MimeId::UNKNOWN,
            extra,
        }
    }

    /// The exact on-disk name, for building paths.
    pub fn os_name(&self) -> &OsStr {
        self.extra
            .info()
            .and_then(|info| info.raw_name.as_deref())
            .unwrap_or_else(|| OsStr::new(self.name.as_str()))
    }

    pub fn path_in(&self, dir: &Path) -> PathBuf {
        dir.join(self.os_name())
    }

    pub fn link_target(&self) -> Option<&Path> {
        self.extra.info()?.link_target.as_deref()
    }

    pub fn extension(&self) -> Option<&str> {
        extension(&self.name)
    }

    pub fn is_dir(&self) -> bool {
        self.stat.is_dir()
    }

    pub fn is_hidden(&self) -> bool {
        self.stat.is_hidden()
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    use super::*;

    #[test]
    fn stays_compact() {
        assert!(
            size_of::<Entry>() <= 72,
            "Entry is {} bytes",
            size_of::<Entry>()
        );
        assert_eq!(size_of::<EntryExtra>(), size_of::<usize>());
    }

    #[test]
    fn a_plain_entry_allocates_no_extra() {
        let entry = Entry::new(
            "notes.md".into(),
            Stat::default(),
            EntryExtra::new(None, None),
        );
        assert!(entry.extra.info().is_none());
        assert_eq!(entry.kind, FileKind::Markdown);
        assert_eq!(entry.path_in(Path::new("/tmp")), Path::new("/tmp/notes.md"));
    }

    #[test]
    fn a_non_utf8_name_keeps_its_raw_bytes_for_paths() {
        let raw = OsString::from_vec(b"bad\xFF.txt".to_vec());
        let entry = Entry::new(
            CompactString::from_utf8_lossy(raw.as_encoded_bytes()),
            Stat::default(),
            EntryExtra::new(None, Some(raw.clone().into_boxed_os_str())),
        );
        assert_eq!(entry.os_name(), raw.as_os_str());
        assert_eq!(entry.name, "bad\u{FFFD}.txt");
    }
}
