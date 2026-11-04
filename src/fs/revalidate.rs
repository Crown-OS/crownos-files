use std::{os::fd::AsFd, path::Path};

use rustix::{
    fs::{AtFlags, CWD, Statx, StatxFlags, makedev, statx},
    io,
};

use crate::{
    fs::{mounts::MountTable, stat_at::timestamp},
    model::dir_stamp::DirStamp,
};

const STAMP_FIELDS: StatxFlags = StatxFlags::MTIME
    .union(StatxFlags::CTIME)
    .union(StatxFlags::SIZE)
    .union(StatxFlags::INO);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    Stale,
}

/// The stamp of an open directory.
pub fn stamp_of(dir: impl AsFd) -> io::Result<DirStamp> {
    let stamp = statx(
        dir,
        c"",
        AtFlags::EMPTY_PATH | AtFlags::STATX_DONT_SYNC,
        STAMP_FIELDS,
    )?;
    Ok(to_stamp(&stamp))
}

pub fn stamp_path(dir: &Path) -> io::Result<DirStamp> {
    let stamp = statx(CWD, dir, AtFlags::STATX_DONT_SYNC, STAMP_FIELDS)?;
    Ok(to_stamp(&stamp))
}

fn to_stamp(stamp: &Statx) -> DirStamp {
    DirStamp {
        modified: timestamp(stamp.stx_mtime),
        changed: timestamp(stamp.stx_ctime),
        len: stamp.stx_size,
        inode: stamp.stx_ino,
        device: makedev(stamp.stx_dev_major, stamp.stx_dev_minor),
    }
}

/// Whether a listing taken at `cached` still describes the directory now at
/// `current`. FAT-family filesystems do not update a directory's mtime when
/// its children change, so listings there are never trusted.
pub fn freshness(cached: Option<DirStamp>, current: DirStamp, timeless: bool) -> Freshness {
    match cached {
        Some(cached) if cached == current && !timeless => Freshness::Fresh,
        _ => Freshness::Stale,
    }
}

/// Stats `dir` (blocking) and compares it with the stamp of its last listing.
pub fn revalidate(dir: &Path, cached: Option<DirStamp>, mounts: &MountTable) -> Freshness {
    let timeless = mounts
        .mount_for(dir)
        .is_some_and(|mount| mount.is_timeless());
    match stamp_path(dir) {
        Ok(current) => freshness(cached, current, timeless),
        Err(_) => Freshness::Stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::timestamp::Timestamp;

    fn stamp(modified: i64) -> DirStamp {
        DirStamp {
            modified: Timestamp::from_parts(modified, 0),
            changed: Timestamp::from_parts(modified, 0),
            len: 4096,
            inode: 7,
            device: 1,
        }
    }

    #[test]
    fn only_an_unchanged_stamp_on_a_timed_filesystem_is_fresh() {
        assert_eq!(freshness(Some(stamp(1)), stamp(1), false), Freshness::Fresh);
        assert_eq!(freshness(Some(stamp(1)), stamp(2), false), Freshness::Stale);
        assert_eq!(freshness(None, stamp(1), false), Freshness::Stale);
        assert_eq!(freshness(Some(stamp(1)), stamp(1), true), Freshness::Stale);
    }
}
