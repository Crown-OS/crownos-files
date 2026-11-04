use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::util::{
    local_time::LocalDateTime,
    uri::{percent_decode, percent_encode},
};

pub const TRASH_FILES_DIR: &str = "files";
pub const TRASH_INFO_DIR: &str = "info";
pub const TRASH_INFO_SUFFIX: &str = ".trashinfo";

const TRASH_INFO_HEADER: &str = "[Trash Info]";
const PATH_KEY: &[u8] = b"Path=";
const DELETION_DATE_KEY: &[u8] = b"DeletionDate=";

/// The contents of a `.trashinfo` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashInfo {
    pub original: PathBuf,
    pub deleted_at: Option<LocalDateTime>,
}

impl TrashInfo {
    /// Trash cans on other mounts (`topdir` set) store paths relative to it.
    pub fn render(&self, topdir: Option<&Path>) -> String {
        let path = topdir
            .and_then(|topdir| self.original.strip_prefix(topdir).ok())
            .unwrap_or(&self.original);
        let mut text = format!("{TRASH_INFO_HEADER}\nPath=");
        percent_encode(path.as_os_str().as_bytes(), &mut text);
        if let Some(deleted_at) = self.deleted_at {
            text.push_str(&format!("\nDeletionDate={deleted_at}"));
        }
        text.push('\n');
        text
    }

    pub fn parse(text: &[u8], topdir: Option<&Path>) -> Option<Self> {
        let mut lines = text
            .split(|&byte| byte == b'\n')
            .map(<[u8]>::trim_ascii)
            .filter(|line| !line.is_empty() && !line.starts_with(b"#"));
        if lines.next()? != TRASH_INFO_HEADER.as_bytes() {
            return None;
        }
        let (mut original, mut deleted_at) = (None, None);
        for line in lines.take_while(|line| !line.starts_with(b"[")) {
            if let Some(path) = line.strip_prefix(PATH_KEY) {
                original = Some(PathBuf::from(OsString::from_vec(percent_decode(path))));
            } else if let Some(date) = line.strip_prefix(DELETION_DATE_KEY) {
                deleted_at = std::str::from_utf8(date)
                    .ok()
                    .and_then(LocalDateTime::parse);
            }
        }
        let original = original?;
        let original = match topdir {
            Some(topdir) if original.is_relative() => topdir.join(original),
            _ if original.is_relative() => return None,
            _ => original,
        };
        Some(Self {
            original,
            deleted_at,
        })
    }
}

/// One entry of a trash can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedItem {
    /// The trash can directory holding `files/` and `info/`.
    pub trash: Arc<Path>,
    /// The name under `files/`.
    pub name: OsString,
    pub info: TrashInfo,
    pub is_dir: bool,
    /// Known for files, and for folders listed in `directorysizes`.
    pub size: Option<u64>,
}

impl TrashedItem {
    pub fn file_path(&self) -> PathBuf {
        self.trash.join(TRASH_FILES_DIR).join(&self.name)
    }

    pub fn info_path(&self) -> PathBuf {
        let mut info_name = self.name.clone();
        info_name.push(TRASH_INFO_SUFFIX);
        self.trash.join(TRASH_INFO_DIR).join(info_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(original: &str) -> TrashInfo {
        TrashInfo {
            original: original.into(),
            deleted_at: LocalDateTime::parse("2004-08-31T22:32:08"),
        }
    }

    #[test]
    fn renders_the_spec_example_and_parses_it_back() {
        let home = info("/home/u/My Pictures/a#b.png");
        let text = home.render(None);
        assert_eq!(
            text,
            "[Trash Info]\nPath=/home/u/My%20Pictures/a%23b.png\nDeletionDate=2004-08-31T22:32:08\n"
        );
        assert_eq!(TrashInfo::parse(text.as_bytes(), None), Some(home));
    }

    #[test]
    fn topdir_trash_paths_are_relative() {
        let topdir = Path::new("/run/media/u/stick");
        let item = info("/run/media/u/stick/docs/x.txt");
        let text = item.render(Some(topdir));
        assert!(text.contains("\nPath=docs/x.txt\n"));
        assert_eq!(TrashInfo::parse(text.as_bytes(), Some(topdir)), Some(item));
        assert_eq!(TrashInfo::parse(text.as_bytes(), None), None);
    }

    #[test]
    fn rejects_files_without_the_header_or_path() {
        assert_eq!(TrashInfo::parse(b"Path=/a\n", None), None);
        assert_eq!(
            TrashInfo::parse(b"[Trash Info]\nDeletionDate=2004-08-31T22:32:08\n", None),
            None
        );
        let undated = TrashInfo::parse(b"[Trash Info]\nPath=/a\n", None).unwrap();
        assert_eq!(undated.deleted_at, None);
    }
}
