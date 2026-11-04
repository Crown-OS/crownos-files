use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, ErrorKind, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
};

use rustix::{
    fs::{CWD, RenameFlags, renameat_with},
    process::getuid,
};
use tokio::task::spawn_blocking;

use crate::{
    fs::mounts::{MountTable, device_id},
    jobs::{
        conflict::claim_unique,
        context::JobContext,
        delete::remove_tree,
        error::{AtPath, JobError},
        job::Transfer,
        r#move::move_one,
    },
    model::trash::{TRASH_FILES_DIR, TRASH_INFO_DIR, TRASH_INFO_SUFFIX, TrashInfo, TrashedItem},
    util::{
        hash::FastHashMap, local_time::LocalDateTime, uri::percent_decode, user_dirs::home_dir,
    },
};

const DATA_HOME_FALLBACK: &str = ".local/share";
const HOME_TRASH_DIR: &str = "Trash";
const SHARED_TRASH_DIR: &str = ".Trash";
const DIRECTORY_SIZES_FILE: &str = "directorysizes";
const TRASH_DIR_MODE: u32 = 0o700;
const TRASH_INFO_MODE: u32 = 0o600;
const STICKY_BIT: u32 = 0o1000;
const TMPFS: &str = "tmpfs";

/// A trash directory; `topdir` is set for the ones at the top of a mount.
struct TrashCan {
    root: PathBuf,
    topdir: Option<PathBuf>,
}

/// `$XDG_DATA_HOME/Trash`.
pub fn home_trash() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(DATA_HOME_FALLBACK)))
        .map(|data| data.join(HOME_TRASH_DIR))
}

/// Every trashed item in the home trash and in the trash directories of
/// mounted filesystems.
pub async fn list_trash() -> Result<Vec<TrashedItem>, JobError> {
    Ok(spawn_blocking(|| trash_cans().iter().flat_map(scan_can).collect()).await?)
}

pub(crate) fn run_trash(
    paths: &[PathBuf],
    ctx: &mut JobContext,
    trashed: &mut Vec<TrashedItem>,
) -> Result<(), JobError> {
    ctx.start(0, paths.len() as u64);
    for path in paths {
        ctx.check()?;
        trashed.push(trash_one(path)?);
        ctx.progress.add_item(path);
    }
    Ok(())
}

pub(crate) fn run_restore(items: &[TrashedItem], ctx: &mut JobContext) -> Result<(), JobError> {
    ctx.start(0, items.len() as u64);
    for item in items {
        ctx.check()?;
        let original = &item.info.original;
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let transfer = Transfer {
            from: item.file_path(),
            to: original.clone(),
        };
        if move_one(&transfer, None, ctx)?.is_some() {
            remove_info(item)?;
        }
    }
    Ok(())
}

pub(crate) fn run_delete_from_trash(
    items: &[TrashedItem],
    ctx: &mut JobContext,
) -> Result<(), JobError> {
    ctx.start(0, items.len() as u64);
    for item in items {
        let file = item.file_path();
        remove_tree(&file, &ctx.cancel, || {})?;
        remove_info(item)?;
        ctx.progress.add_item(&file);
    }
    Ok(())
}

pub(crate) fn run_empty_trash(ctx: &mut JobContext) -> Result<(), JobError> {
    let cans = trash_cans();
    let entries: Vec<PathBuf> = cans
        .iter()
        .flat_map(|can| [TRASH_FILES_DIR, TRASH_INFO_DIR].map(|dir| can.root.join(dir)))
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    ctx.start(0, entries.len() as u64);
    for entry in &entries {
        remove_tree(entry, &ctx.cancel, || {})?;
        ctx.progress.add_item(entry);
    }
    for can in &cans {
        let _ = fs::remove_file(can.root.join(DIRECTORY_SIZES_FILE));
    }
    Ok(())
}

/// Moves `path` into the trash of its filesystem, writing its `.trashinfo`
/// first.
fn trash_one(path: &Path) -> Result<TrashedItem, JobError> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(JobError::NoTrash(path.to_path_buf()));
    };
    let original = fs::canonicalize(parent).at(parent)?.join(name);
    let is_dir = fs::symlink_metadata(&original).at(&original)?.is_dir();
    let can = trash_can_for(&original)?;
    let files = can.root.join(TRASH_FILES_DIR);
    let (claimed, (info_path, mut info_file)) =
        claim_unique(&can.root.join(TRASH_INFO_DIR), name, is_dir, |candidate| {
            claim_info(candidate, &files)
        })
        .at(&can.root)?;
    let stored_name = claimed.file_name().unwrap_or(name).to_os_string();
    let info = TrashInfo {
        original,
        deleted_at: LocalDateTime::now(),
    };
    let moved = info_file
        .write_all(info.render(can.topdir.as_deref()).as_bytes())
        .and_then(|()| {
            renameat_with(
                CWD,
                &info.original,
                CWD,
                files.join(&stored_name),
                RenameFlags::NOREPLACE,
            )
            .map_err(io::Error::from)
        });
    if let Err(error) = moved {
        let _ = fs::remove_file(&info_path);
        return Err(error).at(&info.original);
    }
    Ok(TrashedItem {
        trash: can.root.into(),
        name: stored_name,
        info,
        is_dir,
        size: None,
    })
}

/// Creates `<candidate>.trashinfo` exclusively, provided `files/` has no
/// leftover entry of that name either.
fn claim_info(candidate: &Path, files: &Path) -> io::Result<(PathBuf, File)> {
    let name = candidate.file_name().unwrap_or_default();
    let mut info_name = name.to_os_string();
    info_name.push(TRASH_INFO_SUFFIX);
    let info_path = candidate.with_file_name(info_name);
    let info_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(TRASH_INFO_MODE)
        .open(&info_path)?;
    if fs::symlink_metadata(files.join(name)).is_ok() {
        let _ = fs::remove_file(&info_path);
        return Err(ErrorKind::AlreadyExists.into());
    }
    Ok((info_path, info_file))
}

/// The home trash when `original` shares its filesystem, otherwise the trash
/// at the top of `original`'s mount.
fn trash_can_for(original: &Path) -> Result<TrashCan, JobError> {
    let no_trash = || JobError::NoTrash(original.to_path_buf());
    let parent = original.parent().unwrap_or(original);
    let device = device_id(parent).at(parent)?;
    let home = home_trash().filter(|home| {
        prepare(home).is_ok() && device_id(home).is_ok_and(|home_device| home_device == device)
    });
    if let Some(root) = home {
        return Ok(TrashCan { root, topdir: None });
    }
    let mounts = MountTable::read().map_err(|_| no_trash())?;
    let topdir = mounts
        .mount_for(parent)
        .map(|mount| mount.mount_point.clone())
        .ok_or_else(no_trash)?;
    let root = topdir_trash(&topdir).ok_or_else(no_trash)?;
    Ok(TrashCan {
        root,
        topdir: Some(topdir),
    })
}

/// `$topdir/.Trash/$uid` when `.Trash` is a sticky, real directory, else
/// `$topdir/.Trash-$uid`; created on demand.
pub(crate) fn topdir_trash(topdir: &Path) -> Option<PathBuf> {
    let uid = getuid().as_raw().to_string();
    let shared = topdir.join(SHARED_TRASH_DIR);
    let shared_usable = fs::symlink_metadata(&shared)
        .is_ok_and(|meta| meta.is_dir() && meta.mode() & STICKY_BIT != 0);
    shared_usable
        .then(|| shared.join(&uid))
        .into_iter()
        .chain([topdir.join(format!("{SHARED_TRASH_DIR}-{uid}"))])
        .find(|root| {
            prepare(root).is_ok() && fs::symlink_metadata(root).is_ok_and(|meta| meta.is_dir())
        })
}

fn prepare(root: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(TRASH_DIR_MODE);
    builder.create(root.join(TRASH_FILES_DIR))?;
    builder.create(root.join(TRASH_INFO_DIR))
}

fn trash_cans() -> Vec<TrashCan> {
    let uid = getuid().as_raw().to_string();
    let mounts = MountTable::read().unwrap_or_default();
    let home = home_trash().map(|root| TrashCan { root, topdir: None });
    let topdirs = mounts
        .mounts()
        .iter()
        .filter(|mount| !mount.is_pseudo() || mount.fs_type == TMPFS)
        .flat_map(|mount| {
            let topdir = &mount.mount_point;
            [
                topdir.join(SHARED_TRASH_DIR).join(&uid),
                topdir.join(format!("{SHARED_TRASH_DIR}-{uid}")),
            ]
            .map(|root| TrashCan {
                root,
                topdir: Some(topdir.clone()),
            })
        });
    home.into_iter()
        .chain(topdirs)
        .filter(|can| can.root.join(TRASH_INFO_DIR).is_dir())
        .collect()
}

fn scan_can(can: &TrashCan) -> Vec<TrashedItem> {
    let Ok(entries) = fs::read_dir(can.root.join(TRASH_INFO_DIR)) else {
        return Vec::new();
    };
    let sizes = directory_sizes(&can.root);
    let trash: Arc<Path> = can.root.as_path().into();
    entries
        .flatten()
        .filter_map(|entry| {
            let info_name = entry.file_name();
            let name = info_name
                .as_bytes()
                .strip_suffix(TRASH_INFO_SUFFIX.as_bytes())?;
            let name = OsStr::from_bytes(name).to_os_string();
            let info = TrashInfo::parse(&fs::read(entry.path()).ok()?, can.topdir.as_deref())?;
            let meta = fs::symlink_metadata(trash.join(TRASH_FILES_DIR).join(&name)).ok()?;
            let size = if meta.is_dir() {
                sizes.get(&name).copied()
            } else {
                Some(meta.len())
            };
            Some(TrashedItem {
                trash: trash.clone(),
                name,
                info,
                is_dir: meta.is_dir(),
                size,
            })
        })
        .collect()
}

/// The optional `directorysizes` cache: `size mtime percent-encoded-name`.
fn directory_sizes(root: &Path) -> FastHashMap<OsString, u64> {
    let Ok(text) = fs::read(root.join(DIRECTORY_SIZES_FILE)) else {
        return FastHashMap::default();
    };
    text.split(|&byte| byte == b'\n')
        .filter_map(|line| {
            let mut fields = line.splitn(3, |&byte| byte == b' ');
            let size = std::str::from_utf8(fields.next()?).ok()?.parse().ok()?;
            let _mtime = fields.next()?;
            Some((OsString::from_vec(percent_decode(fields.next()?)), size))
        })
        .collect()
}

fn remove_info(item: &TrashedItem) -> Result<(), JobError> {
    let info = item.info_path();
    match fs::remove_file(&info) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error).at(&info),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_sticky_shared_trash_gets_a_per_user_folder() {
        let temp = tempfile::tempdir().unwrap();
        let uid = getuid().as_raw();
        let shared = temp.path().join(SHARED_TRASH_DIR);
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o1777)).unwrap();
        assert_eq!(
            topdir_trash(temp.path()),
            Some(shared.join(uid.to_string()))
        );
        assert!(shared.join(uid.to_string()).join(TRASH_INFO_DIR).is_dir());
    }

    #[test]
    fn a_shared_trash_without_the_sticky_bit_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let uid = getuid().as_raw();
        fs::create_dir(temp.path().join(SHARED_TRASH_DIR)).unwrap();
        let own = temp.path().join(format!(".Trash-{uid}"));
        assert_eq!(topdir_trash(temp.path()), Some(own.clone()));
        assert!(own.join(TRASH_FILES_DIR).is_dir());
    }

    #[test]
    fn a_symlinked_shared_trash_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let elsewhere = temp.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o1777)).unwrap();
        std::os::unix::fs::symlink(&elsewhere, temp.path().join(SHARED_TRASH_DIR)).unwrap();
        let uid = getuid().as_raw();
        assert_eq!(
            topdir_trash(temp.path()),
            Some(temp.path().join(format!(".Trash-{uid}")))
        );
    }

    #[test]
    fn directory_sizes_are_read_by_decoded_name() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(DIRECTORY_SIZES_FILE),
            b"4096 1700000000 my%20dir\nbogus\n",
        )
        .unwrap();
        let sizes = directory_sizes(temp.path());
        assert_eq!(sizes.get(OsStr::new("my dir")), Some(&4096));
        assert_eq!(sizes.len(), 1);
    }
}
