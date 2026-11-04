use std::{
    fs,
    path::{Path, PathBuf},
};

use rustix::{
    fs::{CWD, RenameFlags, renameat, renameat_with},
    io::Errno,
};
use tokio_util::sync::CancellationToken;

use crate::{
    fs::mounts::same_device,
    jobs::{
        conflict::{Claim, ConflictPolicy, unique_name},
        context::JobContext,
        copy::{TransferMode, ensure_room, transfer_tree},
        delete::remove_tree,
        error::{AtPath, JobError},
        guard::ensure_not_into_itself,
        job::Transfer,
        plan::RootPlan,
    },
};

enum Renamed {
    To(PathBuf),
    Skipped,
    /// The rename cannot do it: copy, then delete, answering the root
    /// conflict with the policy already chosen.
    NeedsCopy(Option<ConflictPolicy>),
}

pub(crate) fn run_move(
    transfers: &[Transfer],
    ctx: &mut JobContext,
    moved: &mut Vec<Transfer>,
) -> Result<(), JobError> {
    let plans = transfers
        .iter()
        .map(|transfer| {
            ensure_not_into_itself(&transfer.from, &transfer.to)?;
            cross_device_plan(transfer, &ctx.cancel)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = plans.iter().flatten().map(|plan| plan.bytes).sum();
    let items = plans
        .iter()
        .map(|plan| plan.as_ref().map_or(1, RootPlan::items))
        .sum();
    ctx.start(bytes, items);
    ensure_room(
        transfers
            .iter()
            .zip(&plans)
            .filter_map(|(transfer, plan)| Some((transfer, plan.as_ref()?))),
    )?;
    for (transfer, plan) in transfers.iter().zip(plans) {
        if let Some(to) = move_one(transfer, plan, ctx)? {
            moved.push(Transfer {
                from: transfer.from.clone(),
                to,
            });
        }
    }
    Ok(())
}

/// The measured tree of a source that has to be copied across filesystems.
fn cross_device_plan(
    transfer: &Transfer,
    cancel: &CancellationToken,
) -> Result<Option<RootPlan>, JobError> {
    let source_dir = transfer.from.parent().unwrap_or(&transfer.from);
    let target_dir = transfer.to.parent().unwrap_or(&transfer.to);
    if same_device(source_dir, target_dir).at(target_dir)? {
        return Ok(None);
    }
    RootPlan::scan(&transfer.from, cancel).map(Some)
}

/// Moves one source: a `renameat2(RENAME_NOREPLACE)` when `plan` is `None`
/// (same filesystem), otherwise copy then delete. Returns where it ended up.
pub(crate) fn move_one(
    transfer: &Transfer,
    plan: Option<RootPlan>,
    ctx: &mut JobContext,
) -> Result<Option<PathBuf>, JobError> {
    let Transfer { from, to } = transfer;
    if from == to {
        ctx.progress.add_item(from);
        return Ok(None);
    }
    let root_policy = match plan {
        Some(_) => None,
        None => match rename_into_place(from, to, ctx)? {
            Renamed::To(target) => {
                ctx.progress.add_item(from);
                return Ok(Some(target));
            }
            Renamed::Skipped => {
                ctx.progress.add_item(from);
                return Ok(None);
            }
            Renamed::NeedsCopy(policy) => policy,
        },
    };
    let plan = match plan {
        Some(plan) => plan,
        None => RootPlan::scan(from, &ctx.cancel)?,
    };
    transfer_tree(transfer, &plan, TransferMode::Move, root_policy, ctx)
}

fn rename_into_place(from: &Path, to: &Path, ctx: &mut JobContext) -> Result<Renamed, JobError> {
    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => return Ok(Renamed::To(to.to_path_buf())),
        Err(Errno::XDEV) => return Ok(Renamed::NeedsCopy(None)),
        Err(Errno::EXIST) => {}
        Err(error) => return Err(error).at(from),
    }
    match ctx.resolve(from, to)? {
        ConflictPolicy::Skip => Ok(Renamed::Skipped),
        ConflictPolicy::KeepBoth => keep_both(from, to),
        ConflictPolicy::Replace => replace(from, to, ctx),
    }
}

/// Claims a free " (2)"-style name and renames over the placeholder.
fn keep_both(from: &Path, to: &Path) -> Result<Renamed, JobError> {
    let (Some(dir), Some(name)) = (to.parent(), to.file_name()) else {
        return Err(JobError::Exists(to.to_path_buf()));
    };
    let is_dir = is_dir(from);
    let claim = if is_dir { Claim::Dir } else { Claim::File };
    let placeholder = unique_name(dir, name, claim).at(to)?;
    if let Err(error) = renameat(CWD, from, CWD, &placeholder) {
        let _ = if is_dir {
            fs::remove_dir(&placeholder)
        } else {
            fs::remove_file(&placeholder)
        };
        return Err(error).at(from);
    }
    Ok(Renamed::To(placeholder))
}

/// Files replace atomically; folders merge into folders.
fn replace(from: &Path, to: &Path, ctx: &JobContext) -> Result<Renamed, JobError> {
    if from.starts_with(to) {
        return Err(JobError::IntoItself {
            folder: to.to_path_buf(),
        });
    }
    if is_dir(from) && is_dir(to) {
        return Ok(Renamed::NeedsCopy(Some(ConflictPolicy::Replace)));
    }
    match renameat(CWD, from, CWD, to) {
        Ok(()) => {}
        Err(Errno::ISDIR | Errno::NOTDIR) => {
            remove_tree(to, &ctx.cancel, || {})?;
            renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).at(from)?;
        }
        Err(Errno::XDEV) => return Ok(Renamed::NeedsCopy(Some(ConflictPolicy::Replace))),
        Err(error) => return Err(error).at(from),
    }
    Ok(Renamed::To(to.to_path_buf()))
}

fn is_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}
