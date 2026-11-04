use std::{
    borrow::Cow,
    fs::{self, DirBuilder, File},
    io::{self, ErrorKind},
    os::unix::fs::{DirBuilderExt, MetadataExt, symlink},
    path::{Path, PathBuf},
};

use rustix::fs::{CWD, FileType, Mode, mknodat};

use crate::{
    jobs::{
        attributes::copy_dir_attributes,
        conflict::{ConflictPolicy, claim_unique},
        context::JobContext,
        copy_file::{create_exclusive, fill_file},
        delete::remove_tree,
        error::{AtPath, JobError},
        guard::{ensure_free_space, ensure_not_into_itself},
        job::Transfer,
        plan::{RootPlan, totals},
        traverse::{Node, NodeKind},
    },
    util::hash::FastHashMap,
};

const NEW_DIR_MODE: u32 = 0o700;
const PERMISSION_BITS: u32 = 0o7777;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferMode {
    Copy,
    /// Copy, then remove each source entry once its copy is complete.
    Move,
}

enum Claimed {
    File(File),
    Dir { created: bool },
    Link,
}

pub(crate) fn run_copy(
    transfers: &[Transfer],
    ctx: &mut JobContext,
    created: &mut Vec<PathBuf>,
) -> Result<(), JobError> {
    let plans = transfers
        .iter()
        .map(|transfer| {
            ensure_not_into_itself(&transfer.from, &transfer.to)?;
            RootPlan::scan(&transfer.from, &ctx.cancel)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (bytes, items) = totals(&plans);
    ctx.start(bytes, items);
    ensure_room(transfers.iter().zip(&plans))?;
    for (transfer, plan) in transfers.iter().zip(&plans) {
        if let Some(target) = transfer_tree(transfer, plan, TransferMode::Copy, None, ctx)? {
            created.push(target);
        }
    }
    Ok(())
}

/// Checks the free space of every target folder for all that goes there.
pub(crate) fn ensure_room<'a>(
    planned: impl IntoIterator<Item = (&'a Transfer, &'a RootPlan)>,
) -> Result<(), JobError> {
    let mut needed: FastHashMap<&Path, u64> = FastHashMap::default();
    for (transfer, plan) in planned {
        if let Some(dir) = transfer.to.parent() {
            *needed.entry(dir).or_default() += plan.bytes;
        }
    }
    needed
        .into_iter()
        .try_for_each(|(dir, bytes)| ensure_free_space(dir, bytes))
}

/// Recreates `plan` at `transfer.to`, asking about conflicts; `root_policy`
/// answers the conflict on the root in advance. Returns where the root
/// ended up, or `None` when it was skipped.
pub(crate) fn transfer_tree(
    transfer: &Transfer,
    plan: &RootPlan,
    mode: TransferMode,
    root_policy: Option<ConflictPolicy>,
    ctx: &mut JobContext,
) -> Result<Option<PathBuf>, JobError> {
    let mut dir_targets: FastHashMap<&Path, PathBuf> = FastHashMap::default();
    let mut created_dirs = Vec::new();
    let mut root_target = None;
    for node in &plan.nodes {
        ctx.check()?;
        let source = node.under(&transfer.from);
        let desired = match (node.relative.parent(), node.relative.file_name()) {
            (None, _) => Cow::Borrowed(transfer.to.as_path()),
            (Some(parent), Some(name)) if dir_targets.contains_key(parent) => {
                Cow::Owned(dir_targets[parent].join(name))
            }
            _ => {
                ctx.progress.skip(node.len, 1);
                continue;
            }
        };
        let Some((target, claimed)) = place(node, &source, &desired, root_policy, ctx)? else {
            ctx.progress.skip(node.len, 1);
            continue;
        };
        match claimed {
            Claimed::File(output) => {
                fill_file(&source, &target, &output, &ctx.cancel, |copied| {
                    ctx.progress.add_bytes(copied, &source)
                })?;
            }
            Claimed::Dir { created: true } => {
                created_dirs.push((fs::symlink_metadata(&source).at(&source)?, target.clone()))
            }
            Claimed::Dir { created: false } | Claimed::Link => {}
        }
        if mode == TransferMode::Move && node.kind != NodeKind::Dir {
            fs::remove_file(&source).at(&source)?;
        }
        ctx.progress.add_item(&source);
        if node.is_root() {
            root_target = Some(target.clone());
        }
        if node.kind == NodeKind::Dir {
            dir_targets.insert(&node.relative, target);
        }
    }
    for (source_meta, target) in created_dirs.iter().rev() {
        copy_dir_attributes(source_meta, target).at(target)?;
    }
    if mode == TransferMode::Move {
        remove_moved_dirs(transfer, plan, &dir_targets)?;
    }
    Ok(root_target)
}

/// Removes the source folders whose contents all moved, deepest first.
fn remove_moved_dirs(
    transfer: &Transfer,
    plan: &RootPlan,
    moved: &FastHashMap<&Path, PathBuf>,
) -> Result<(), JobError> {
    let dirs =
        plan.nodes.iter().rev().filter(|node| {
            node.kind == NodeKind::Dir && moved.contains_key(node.relative.as_path())
        });
    for node in dirs {
        let source = node.under(&transfer.from);
        match fs::remove_dir(&source) {
            Err(error)
                if !matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                ) =>
            {
                return Err(error).at(&source);
            }
            _ => {}
        }
    }
    Ok(())
}

fn place(
    node: &Node,
    source: &Path,
    desired: &Path,
    root_policy: Option<ConflictPolicy>,
    ctx: &mut JobContext,
) -> Result<Option<(PathBuf, Claimed)>, JobError> {
    match claim(node, source, desired) {
        Ok(Some(claimed)) => return Ok(Some((desired.to_path_buf(), claimed))),
        Ok(None) => return Ok(None),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).at(desired),
    }
    let merge =
        node.kind == NodeKind::Dir && fs::symlink_metadata(desired).is_ok_and(|meta| meta.is_dir());
    let merged = Ok(Some((
        desired.to_path_buf(),
        Claimed::Dir { created: false },
    )));
    if merge && !node.is_root() {
        return merged;
    }
    let policy = match root_policy.filter(|_| node.is_root()) {
        Some(policy) => policy,
        None if source == desired => ConflictPolicy::KeepBoth,
        None => ctx.resolve(source, desired)?,
    };
    match policy {
        ConflictPolicy::Skip => Ok(None),
        ConflictPolicy::Replace if merge => merged,
        ConflictPolicy::Replace => {
            if source.starts_with(desired) {
                return Err(JobError::IntoItself {
                    folder: desired.to_path_buf(),
                });
            }
            remove_tree(desired, &ctx.cancel, || {})?;
            let claimed = claim(node, source, desired).at(desired)?;
            Ok(claimed.map(|claimed| (desired.to_path_buf(), claimed)))
        }
        ConflictPolicy::KeepBoth => {
            let (Some(dir), Some(name)) = (desired.parent(), desired.file_name()) else {
                return Err(JobError::Exists(desired.to_path_buf()));
            };
            let is_dir = node.kind == NodeKind::Dir;
            let (target, claimed) =
                claim_unique(dir, name, is_dir, |path| claim(node, source, path)).at(desired)?;
            Ok(claimed.map(|claimed| (target, claimed)))
        }
    }
}

/// Creates `target` as the kind of entry `node` is; `None` for sockets and
/// devices, which are left out.
fn claim(node: &Node, source: &Path, target: &Path) -> io::Result<Option<Claimed>> {
    let claimed = match node.kind {
        NodeKind::Dir => {
            DirBuilder::new().mode(NEW_DIR_MODE).create(target)?;
            Claimed::Dir { created: true }
        }
        NodeKind::File => Claimed::File(create_exclusive(target)?),
        NodeKind::Symlink => {
            symlink(fs::read_link(source)?, target)?;
            Claimed::Link
        }
        NodeKind::Fifo => {
            let mode = Mode::from_raw_mode(fs::symlink_metadata(source)?.mode() & PERMISSION_BITS);
            mknodat(CWD, target, FileType::Fifo, mode, 0)?;
            Claimed::Link
        }
        NodeKind::Other => return Ok(None),
    };
    Ok(Some(claimed))
}
