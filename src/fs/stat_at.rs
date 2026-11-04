use std::{
    ffi::{OsStr, OsString},
    os::{
        fd::BorrowedFd,
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::{Path, PathBuf},
};

use compact_str::CompactString;
use rustix::{
    fs::{
        AtFlags, CWD, FileType, Mode, RawMode, Statx, StatxFlags, StatxTimestamp, readlinkat, statx,
    },
    io::{self, Errno},
};

use crate::model::{
    entry::{Entry, EntryExtra},
    stat::{Stat, StatFlags},
    timestamp::Timestamp,
};

const ENTRY_FIELDS: StatxFlags = StatxFlags::TYPE
    .union(StatxFlags::MODE)
    .union(StatxFlags::SIZE)
    .union(StatxFlags::MTIME)
    .union(StatxFlags::BTIME);
const EXECUTE_BITS: Mode = Mode::XUSR.union(Mode::XGRP).union(Mode::XOTH);

/// Stats `name` inside the open directory `dir`. A name that cannot be
/// statted still lists, as a [`StatFlags::DUMMY`] entry.
pub fn stat_entry(dir: BorrowedFd<'_>, name: &[u8]) -> Entry {
    lookup(dir, name, name).unwrap_or_else(|_| {
        let (display, raw_name) = display_name(name);
        Entry::new(
            display,
            Stat::dummy(hidden_flag(name)),
            EntryExtra::new(None, raw_name),
        )
    })
}

/// Stats an absolute path; fails when it no longer exists.
pub fn stat_path(path: &Path) -> io::Result<Entry> {
    let name = path.file_name().ok_or(Errno::INVAL)?;
    lookup(CWD, path.as_os_str().as_bytes(), name.as_bytes())
}

fn lookup(dir: BorrowedFd<'_>, path: &[u8], name: &[u8]) -> io::Result<Entry> {
    let hidden = hidden_flag(name);
    let own = statx(
        dir,
        path,
        AtFlags::SYMLINK_NOFOLLOW | AtFlags::STATX_DONT_SYNC,
        ENTRY_FIELDS,
    )?;
    let (display, raw_name) = display_name(name);
    if file_type(&own) != FileType::Symlink {
        return Ok(Entry::new(
            display,
            stat_from(&own, hidden),
            EntryExtra::new(None, raw_name),
        ));
    }
    let link_target = readlinkat(dir, path, Vec::new())
        .ok()
        .map(|target| PathBuf::from(OsString::from_vec(target.into_bytes())));
    let link = hidden | StatFlags::SYMLINK;
    let stat = match statx(dir, path, AtFlags::STATX_DONT_SYNC, ENTRY_FIELDS) {
        Ok(target) => stat_from(&target, link),
        Err(_) => stat_from(&own, link | StatFlags::BROKEN),
    };
    Ok(Entry::new(
        display,
        stat,
        EntryExtra::new(link_target, raw_name),
    ))
}

fn display_name(name: &[u8]) -> (CompactString, Option<Box<OsStr>>) {
    match CompactString::from_utf8(name) {
        Ok(name) => (name, None),
        Err(_) => (
            CompactString::from_utf8_lossy(name),
            Some(OsStr::from_bytes(name).into()),
        ),
    }
}

fn hidden_flag(name: &[u8]) -> StatFlags {
    if name.first() == Some(&b'.') {
        StatFlags::HIDDEN
    } else {
        StatFlags::empty()
    }
}

fn file_type(statx: &Statx) -> FileType {
    FileType::from_raw_mode(RawMode::from(statx.stx_mode))
}

fn stat_from(statx: &Statx, flags: StatFlags) -> Stat {
    let kind = file_type(statx);
    let executable = kind == FileType::RegularFile
        && Mode::from_raw_mode(RawMode::from(statx.stx_mode)).intersects(EXECUTE_BITS);
    let created = if statx.stx_mask & StatxFlags::BTIME.bits() != 0 {
        timestamp(statx.stx_btime)
    } else {
        Timestamp::UNKNOWN
    };
    Stat {
        len: if kind == FileType::Directory {
            0
        } else {
            statx.stx_size
        },
        modified: timestamp(statx.stx_mtime),
        created,
        mode: statx.stx_mode,
        flags: if executable {
            flags | StatFlags::EXEC
        } else {
            flags
        },
    }
}

pub(crate) fn timestamp(time: StatxTimestamp) -> Timestamp {
    Timestamp::from_parts(time.tv_sec, time.tv_nsec)
}
