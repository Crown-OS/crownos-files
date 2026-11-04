use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};

use compact_str::CompactString;
use rustix::{
    fs::{AtFlags, CWD, StatxFlags, makedev, statx},
    io,
};

use crate::error::EngineError;

pub const MOUNTINFO: &str = "/proc/self/mountinfo";

const PSEUDO_FILESYSTEMS: &[&str] = &[
    "autofs",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "proc",
    "pstore",
    "ramfs",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "squashfs",
    "sysfs",
    "tmpfs",
    "tracefs",
    "overlay",
];
const SYSTEM_MOUNT_ROOTS: &[&str] = &["/proc", "/sys", "/dev", "/run/user", "/run/credentials"];
const TIMELESS_FILESYSTEMS: &[&str] = &["exfat", "vfat", "msdos", "fat"];
const REMOTE_FILESYSTEMS: &[&str] = &[
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "9p",
    "afs",
    "ceph",
    "glusterfs",
    "davfs",
    "fuse",
    "fuseblk",
    "sshfs",
];
const FUSE_SUBTYPE_PREFIX: &str = "fuse.";

/// One line of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub id: u32,
    pub device: u64,
    pub mount_point: PathBuf,
    pub fs_type: CompactString,
    pub source: CompactString,
    pub read_only: bool,
}

impl Mount {
    /// Kernel, virtual and system mounts the Devices list never shows.
    pub fn is_pseudo(&self) -> bool {
        PSEUDO_FILESYSTEMS.contains(&self.fs_type.as_str())
            || SYSTEM_MOUNT_ROOTS
                .iter()
                .any(|root| self.mount_point.starts_with(root))
    }

    /// Filesystems that leave a directory's mtime alone when it changes.
    pub fn is_timeless(&self) -> bool {
        TIMELESS_FILESYSTEMS.contains(&self.fs_type.as_str())
    }

    /// Filesystems inotify cannot see remote changes on.
    pub fn is_remote(&self) -> bool {
        self.fs_type.starts_with(FUSE_SUBTYPE_PREFIX)
            || REMOTE_FILESYSTEMS.contains(&self.fs_type.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountTable {
    mounts: Vec<Mount>,
}

impl MountTable {
    pub fn read() -> Result<Self, EngineError> {
        std::fs::read_to_string(MOUNTINFO)
            .map(|text| Self::parse(&text))
            .map_err(|source| EngineError::Read {
                path: MOUNTINFO.into(),
                source,
            })
    }

    pub fn parse(text: &str) -> Self {
        Self {
            mounts: text.lines().filter_map(parse_line).collect(),
        }
    }

    pub fn mounts(&self) -> &[Mount] {
        &self.mounts
    }

    /// The mounts a user would call drives.
    pub fn visible(&self) -> impl Iterator<Item = &Mount> {
        self.mounts.iter().filter(|mount| !mount.is_pseudo())
    }

    /// The mount `path` lives on: the deepest mount point above it, and the
    /// latest of several stacked on the same point.
    pub fn mount_for(&self, path: &Path) -> Option<&Mount> {
        self.mounts
            .iter()
            .filter(|mount| path.starts_with(&mount.mount_point))
            .max_by_key(|mount| mount.mount_point.components().count())
    }

    pub fn fs_type(&self, path: &Path) -> Option<&str> {
        self.mount_for(path).map(|mount| mount.fs_type.as_str())
    }
}

/// The device a path lives on, for "same filesystem?" checks before a move.
pub fn device_id(path: &Path) -> io::Result<u64> {
    let stat = statx(CWD, path, AtFlags::STATX_DONT_SYNC, StatxFlags::TYPE)?;
    Ok(makedev(stat.stx_dev_major, stat.stx_dev_minor))
}

pub fn same_device(left: &Path, right: &Path) -> io::Result<bool> {
    Ok(device_id(left)? == device_id(right)?)
}

fn parse_line(line: &str) -> Option<Mount> {
    let (front, back) = line.split_once(" - ")?;
    let mut front = front.split(' ');
    let id = front.next()?.parse().ok()?;
    let _parent = front.next()?;
    let (major, minor) = front.next()?.split_once(':')?;
    let _root = front.next()?;
    let mount_point = unescape(front.next()?);
    let read_only = front.next()?.split(',').any(|option| option == "ro");
    let mut back = back.split(' ');
    let fs_type = back.next()?.into();
    let source = back.next()?.into();
    Some(Mount {
        id,
        device: makedev(major.parse().ok()?, minor.parse().ok()?),
        mount_point,
        fs_type,
        source,
        read_only,
    })
}

/// Undoes mountinfo's octal escapes (`\040` for a space).
fn unescape(field: &str) -> PathBuf {
    let bytes = field.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some((&byte, tail)) = rest.split_first() {
        match (byte, tail) {
            (b'\\', [a, b, c, after @ ..])
                if [a, b, c].iter().all(|digit| (b'0'..=b'7').contains(digit)) =>
            {
                decoded.push(((a - b'0') << 6) | ((b - b'0') << 3) | (c - b'0'));
                rest = after;
            }
            _ => {
                decoded.push(byte);
                rest = tail;
            }
        }
    }
    PathBuf::from(OsString::from_vec(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
22 1 259:3 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd
23 22 0:21 / /proc rw,nosuid shared:5 - proc proc rw
24 22 0:22 / /tmp rw,nosuid shared:6 - tmpfs tmpfs rw
40 22 8:17 / /run/media/u/MY\\040STICK rw,nosuid shared:30 - exfat /dev/sdb1 rw
41 22 0:50 / /home/u/remote rw shared:31 - fuse.sshfs u@host: rw
42 22 259:4 / /e ro,relatime shared:32 - ext4 /dev/nvme0n1p3 ro
malformed line
";

    #[test]
    fn parses_fields_and_unescapes_mount_points() {
        let table = MountTable::parse(SAMPLE);
        assert_eq!(table.mounts().len(), 6);
        let stick = &table.mounts()[3];
        assert_eq!(stick.mount_point, Path::new("/run/media/u/MY STICK"));
        assert_eq!(stick.fs_type, "exfat");
        assert_eq!(stick.source, "/dev/sdb1");
        assert_eq!(stick.device, makedev(8, 17));
        assert!(table.mounts()[5].read_only);
    }

    #[test]
    fn hides_pseudo_and_system_mounts() {
        let table = MountTable::parse(SAMPLE);
        let visible: Vec<_> = table
            .visible()
            .map(|mount| mount.mount_point.as_path())
            .collect();
        assert_eq!(
            visible,
            [
                Path::new("/"),
                Path::new("/run/media/u/MY STICK"),
                Path::new("/home/u/remote"),
                Path::new("/e"),
            ]
        );
    }

    #[test]
    fn resolves_the_deepest_mount_and_its_traits() {
        let table = MountTable::parse(SAMPLE);
        assert_eq!(table.fs_type(Path::new("/home/u/file")), Some("btrfs"));
        assert_eq!(table.fs_type(Path::new("/e/CrownOS")), Some("ext4"));
        assert!(
            table
                .mount_for(Path::new("/run/media/u/MY STICK/a"))
                .is_some_and(Mount::is_timeless)
        );
        assert!(
            table
                .mount_for(Path::new("/home/u/remote/x"))
                .is_some_and(Mount::is_remote)
        );
        assert!(
            !table
                .mount_for(Path::new("/home/u"))
                .is_some_and(Mount::is_remote)
        );
    }

    #[test]
    fn the_live_table_has_a_root_and_paths_share_its_device() {
        let table = MountTable::read().expect("mountinfo is readable");
        assert!(table.mount_for(Path::new("/")).is_some());
        assert!(same_device(Path::new("/proc/self"), Path::new("/proc/1")).unwrap_or(true));
    }
}
