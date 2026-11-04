use bitflags::bitflags;
use rustix::fs::{FileType, Mode, RawMode};

use crate::model::timestamp::Timestamp;

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct StatFlags: u8 {
        const HIDDEN = 1;
        const SYMLINK = 1 << 1;
        const BROKEN = 1 << 2;
        const DUMMY = 1 << 3;
        const EXEC = 1 << 4;
    }
}

const PERMISSION_BITS: u16 = 0o7777;

/// The metadata the views and sorting need, small enough to copy freely.
///
/// For a symlink this describes the target, with [`StatFlags::SYMLINK`] set;
/// a dangling one describes the link itself and adds [`StatFlags::BROKEN`].
/// Directories report `len` 0 until a directory size is measured.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stat {
    pub len: u64,
    pub modified: Timestamp,
    pub created: Timestamp,
    pub mode: u16,
    pub flags: StatFlags,
}

impl Stat {
    /// The stand-in for an entry whose stat failed, so it still lists.
    pub fn dummy(flags: StatFlags) -> Self {
        Self {
            flags: flags | StatFlags::DUMMY,
            ..Self::default()
        }
    }

    pub fn file_type(&self) -> FileType {
        FileType::from_raw_mode(RawMode::from(self.mode))
    }

    pub fn is_dir(&self) -> bool {
        self.file_type() == FileType::Directory
    }

    pub fn permissions(&self) -> Mode {
        Mode::from_raw_mode(RawMode::from(self.mode & PERMISSION_BITS))
    }

    pub fn is_hidden(&self) -> bool {
        self.flags.contains(StatFlags::HIDDEN)
    }

    pub fn is_symlink(&self) -> bool {
        self.flags.contains(StatFlags::SYMLINK)
    }

    pub fn is_broken(&self) -> bool {
        self.flags.contains(StatFlags::BROKEN)
    }

    pub fn is_dummy(&self) -> bool {
        self.flags.contains(StatFlags::DUMMY)
    }

    pub fn is_executable(&self) -> bool {
        self.flags.contains(StatFlags::EXEC)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_small_enough_to_copy() {
        assert!(size_of::<Stat>() <= 32);
    }

    #[test]
    fn mode_carries_type_and_permissions() {
        let stat = Stat {
            mode: 0o040755,
            ..Stat::default()
        };
        assert!(stat.is_dir());
        assert_eq!(stat.permissions().bits(), 0o755);
    }
}
