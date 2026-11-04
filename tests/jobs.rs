#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self, File, FileTimes},
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crownos_files::{
    fs::mounts::same_device,
    jobs::{
        Claim, ConflictPolicy, ConflictReply, Job, JobError, Outcome, Priority, Reversible,
        Transfer, copy_file, unique_name,
    },
};
use rustix::fs::{CWD, FileType, Mode, mknodat};
use support::Jobs;
use tokio_util::sync::CancellationToken;

const LARGE_FILE_LEN: usize = 64 * 1024 * 1024;
const RACERS: usize = 16;
const TREE_DEPTH: usize = 120;
const OLD_TIME_SECS: u64 = 1_000_000_000;
const READ_ONLY: u32 = 0o555;
const WRITABLE: u32 = 0o755;

fn old_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(OLD_TIME_SECS)
}

fn set_mode_and_time(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    File::open(path)
        .unwrap()
        .set_times(
            FileTimes::new()
                .set_accessed(old_time())
                .set_modified(old_time()),
        )
        .unwrap();
}

fn reply(policy: ConflictPolicy, apply_to_all: bool) -> Option<ConflictReply> {
    Some(ConflictReply {
        policy,
        apply_to_all,
    })
}

fn patterned(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index * 31 % 251) as u8).collect()
}

/// A tree with nested folders, an executable, symlinks (one dangling), a
/// FIFO and a read-only folder, all with old times.
fn build_tree(root: &Path, locked_mode: u32) {
    fs::create_dir_all(root.join("docs/deep")).unwrap();
    fs::create_dir(root.join("locked")).unwrap();
    fs::write(root.join("docs/deep/note.txt"), b"note").unwrap();
    fs::write(root.join("run.sh"), b"#!/bin/sh\n").unwrap();
    fs::write(root.join("locked/inside"), b"inside").unwrap();
    symlink("docs/deep/note.txt", root.join("link")).unwrap();
    symlink("missing", root.join("dangling")).unwrap();
    mknodat(
        CWD,
        root.join("pipe"),
        FileType::Fifo,
        Mode::from_raw_mode(0o640),
        0,
    )
    .unwrap();
    set_mode_and_time(&root.join("docs/deep/note.txt"), 0o640);
    set_mode_and_time(&root.join("run.sh"), 0o755);
    set_mode_and_time(&root.join("locked/inside"), 0o444);
    set_mode_and_time(&root.join("docs/deep"), 0o750);
    set_mode_and_time(&root.join("locked"), locked_mode);
}

fn assert_tree_copied(target: &Path, locked_mode: u32) {
    assert_eq!(
        fs::read(target.join("docs/deep/note.txt")).unwrap(),
        b"note"
    );
    for (relative, mode) in [
        ("docs/deep/note.txt", 0o640),
        ("run.sh", 0o755),
        ("locked/inside", 0o444),
        ("docs/deep", 0o750),
        ("locked", locked_mode),
    ] {
        let meta = fs::symlink_metadata(target.join(relative)).unwrap();
        assert_eq!(meta.mode() & 0o7777, mode, "{relative}");
        assert_eq!(meta.modified().unwrap(), old_time(), "{relative}");
    }
    assert_eq!(
        fs::read_link(target.join("link")).unwrap(),
        Path::new("docs/deep/note.txt")
    );
    assert_eq!(
        fs::read_link(target.join("dangling")).unwrap(),
        Path::new("missing")
    );
    assert!(
        fs::symlink_metadata(target.join("pipe"))
            .unwrap()
            .file_type()
            .is_fifo()
    );
}

fn unlock(root: &Path) {
    if root.join("locked").exists() {
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[tokio::test]
async fn copies_trees_with_symlinks_modes_and_times() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("tree");
    let into = temp.path().join("into");
    fs::create_dir(&into).unwrap();
    build_tree(&source, READ_ONLY);

    let mut jobs = Jobs::new();
    let report = jobs
        .run(Job::copy_into([source.clone()], &into), None)
        .await;

    assert_eq!(report.outcome, Some(Outcome::Completed));
    assert_eq!(report.totals, Some((20, 10)));
    assert_eq!(report.conflicts, 0);
    assert_eq!(report.undo, Some(Reversible::Copy(vec![into.join("tree")])));
    assert_tree_copied(&into.join("tree"), READ_ONLY);
    assert_tree_copied(&source, READ_ONLY);
    unlock(&into.join("tree"));
    unlock(&source);
}

#[tokio::test]
async fn copies_a_large_file_byte_for_byte() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large.bin");
    let data = patterned(LARGE_FILE_LEN);
    fs::write(&source, &data).unwrap();
    let mut copied = 0;
    let target = temp.path().join("copy.bin");

    copy_file(&source, &target, &CancellationToken::new(), |chunk| {
        copied += chunk
    })
    .unwrap();

    assert_eq!(copied, LARGE_FILE_LEN as u64);
    assert!(fs::read(&target).unwrap() == data);
}

#[test]
fn cancelling_mid_copy_leaves_no_partial_target() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("large.bin");
    fs::write(&source, patterned(LARGE_FILE_LEN)).unwrap();
    let target = temp.path().join("copy.bin");
    let cancel = CancellationToken::new();

    let result = copy_file(&source, &target, &cancel, |_| cancel.cancel());

    assert!(matches!(result, Err(JobError::Cancelled)));
    assert!(fs::symlink_metadata(&target).is_err());
}

#[tokio::test]
async fn a_job_cancelled_while_queued_never_starts() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("file");
    fs::write(&source, b"x").unwrap();
    let mut jobs = Jobs::new();

    let id = jobs
        .scheduler
        .submit(Job::duplicate([source.clone()]), Priority::Low);
    jobs.scheduler.cancel(id);
    let report = jobs.wait(id, None).await;

    assert_eq!(report.outcome, Some(Outcome::Cancelled));
    assert_eq!(report.totals, None);
    assert!(!temp.path().join("file (2)").exists());
}

#[tokio::test]
async fn moves_across_filesystems_by_copying_then_deleting() {
    let near = tempfile::tempdir().unwrap();
    let far = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    if same_device(near.path(), far.path()).unwrap() {
        eprintln!("skipped: the temp and target directories share a filesystem");
        return;
    }
    let source = near.path().join("tree");
    build_tree(&source, WRITABLE);

    let mut jobs = Jobs::new();
    let report = jobs
        .run(Job::move_into([source.clone()], far.path()), None)
        .await;

    assert_eq!(
        report.outcome,
        Some(Outcome::Completed),
        "{:?}",
        report.error
    );
    assert!(!source.exists());
    assert_tree_copied(&far.path().join("tree"), WRITABLE);
    assert_eq!(
        report.undo,
        Some(Reversible::Move(vec![Transfer {
            from: source,
            to: far.path().join("tree"),
        }]))
    );
    unlock(&far.path().join("tree"));
}

#[tokio::test]
async fn moves_on_one_filesystem_rename_and_undo_back() {
    let temp = tempfile::tempdir().unwrap();
    let (file, folder, into) = (
        temp.path().join("a.txt"),
        temp.path().join("folder"),
        temp.path().join("into"),
    );
    fs::write(&file, b"a").unwrap();
    fs::create_dir_all(folder.join("child")).unwrap();
    fs::create_dir(&into).unwrap();
    let inode = fs::metadata(&file).unwrap().ino();

    let mut jobs = Jobs::new();
    let report = jobs
        .run(Job::move_into([file.clone(), folder.clone()], &into), None)
        .await;
    assert_eq!(report.outcome, Some(Outcome::Completed));
    assert_eq!(report.totals, Some((0, 2)));
    assert_eq!(fs::metadata(into.join("a.txt")).unwrap().ino(), inode);
    assert!(into.join("folder/child").is_dir() && !file.exists() && !folder.exists());

    let undo = report.undo.unwrap().inverse();
    let undone = jobs.run(undo, None).await;
    assert_eq!(undone.outcome, Some(Outcome::Completed));
    assert!(file.exists() && folder.join("child").is_dir());
    assert_eq!(fs::read_dir(&into).unwrap().count(), 0);
}

#[tokio::test]
async fn deletes_deep_trees_permanently() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("deep");
    let mut dir = root.clone();
    for level in 0..TREE_DEPTH {
        dir.push(format!("level{level}"));
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("leaf"), b"x").unwrap();
    symlink(temp.path(), root.join("escape")).unwrap();
    let single = temp.path().join("single");
    fs::write(&single, b"x").unwrap();

    let mut jobs = Jobs::new();
    let report = jobs
        .run(Job::Delete(vec![root.clone(), single.clone()]), None)
        .await;

    assert_eq!(report.outcome, Some(Outcome::Completed));
    let items = TREE_DEPTH as u64 + 4;
    assert_eq!(report.totals, Some((0, items)));
    assert!(!root.exists() && !single.exists() && temp.path().exists());
}

#[tokio::test]
async fn conflicts_follow_the_chosen_policy() {
    let temp = tempfile::tempdir().unwrap();
    let (from, into) = (temp.path().join("from"), temp.path().join("into"));
    fs::create_dir_all(&from).unwrap();
    fs::create_dir_all(&into).unwrap();
    for name in ["a.txt", "b.tar.gz"] {
        fs::write(from.join(name), b"new").unwrap();
        fs::write(into.join(name), b"old").unwrap();
    }
    let sources = || [from.join("a.txt"), from.join("b.tar.gz")];
    let mut jobs = Jobs::new();

    let skipped = jobs
        .run(
            Job::copy_into(sources(), &into),
            reply(ConflictPolicy::Skip, false),
        )
        .await;
    assert_eq!((skipped.conflicts, skipped.undo), (2, None));
    assert_eq!(fs::read(into.join("a.txt")).unwrap(), b"old");

    let kept = jobs
        .run(
            Job::copy_into(sources(), &into),
            reply(ConflictPolicy::KeepBoth, true),
        )
        .await;
    assert_eq!(kept.conflicts, 1);
    assert_eq!(
        kept.undo,
        Some(Reversible::Copy(vec![
            into.join("a (2).txt"),
            into.join("b (2).tar.gz")
        ]))
    );

    let replaced = jobs
        .run(
            Job::copy_into(sources(), &into),
            reply(ConflictPolicy::Replace, false),
        )
        .await;
    assert_eq!(replaced.conflicts, 2);
    assert_eq!(fs::read(into.join("b.tar.gz")).unwrap(), b"new");

    let unanswered = jobs.run(Job::copy_into(sources(), &into), None).await;
    assert_eq!(unanswered.outcome, Some(Outcome::Cancelled));
    assert_eq!(fs::read_dir(&into).unwrap().count(), 4);
}

#[tokio::test]
async fn replacing_a_folder_merges_and_asks_about_its_files() {
    let temp = tempfile::tempdir().unwrap();
    let (from, into) = (
        temp.path().join("from/album"),
        temp.path().join("into/album"),
    );
    fs::create_dir_all(&from).unwrap();
    fs::create_dir_all(&into).unwrap();
    fs::write(from.join("new.jpg"), b"new").unwrap();
    fs::write(from.join("same.jpg"), b"theirs").unwrap();
    fs::write(into.join("same.jpg"), b"mine").unwrap();
    fs::write(into.join("kept.jpg"), b"kept").unwrap();

    let mut jobs = Jobs::new();
    let report = jobs
        .run(
            Job::move_into([from.clone()], &temp.path().join("into")),
            reply(ConflictPolicy::Replace, false),
        )
        .await;

    assert_eq!(report.outcome, Some(Outcome::Completed));
    assert_eq!(report.conflicts, 2);
    assert_eq!(fs::read(into.join("same.jpg")).unwrap(), b"theirs");
    assert!(into.join("kept.jpg").exists() && into.join("new.jpg").exists());
    assert!(!from.exists());
}

#[tokio::test]
async fn duplicates_get_numbered_names_without_asking() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("photo.jpg");
    let folder = temp.path().join("album");
    fs::write(&file, b"x").unwrap();
    fs::create_dir(&folder).unwrap();

    let mut jobs = Jobs::new();
    let report = jobs.run(Job::duplicate([file, folder]), None).await;

    assert_eq!(report.conflicts, 0);
    assert_eq!(
        report.undo,
        Some(Reversible::Copy(vec![
            temp.path().join("photo (2).jpg"),
            temp.path().join("album (2)")
        ]))
    );
}

#[tokio::test]
async fn guards_refuse_impossible_operations() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("folder");
    fs::create_dir_all(folder.join("child")).unwrap();
    let mut jobs = Jobs::new();

    let into_itself = jobs
        .run(
            Job::copy_into([folder.clone()], &folder.join("child")),
            None,
        )
        .await;
    assert_eq!(into_itself.outcome, Some(Outcome::Failed));
    assert!(matches!(
        into_itself.error,
        Some(JobError::IntoItself { .. })
    ));
    assert!(!folder.join("child/folder").exists());

    let read_only = temp.path().join("read-only");
    fs::create_dir(&read_only).unwrap();
    fs::set_permissions(&read_only, fs::Permissions::from_mode(0o555)).unwrap();
    let denied = jobs.run(Job::copy_into([folder], &read_only), None).await;
    assert_eq!(denied.outcome, Some(Outcome::Failed));
    assert!(denied.needs_privilege, "{:?}", denied.error);
}

#[test]
fn racing_claims_never_share_a_name() {
    let temp = tempfile::tempdir().unwrap();
    let dir = Arc::new(temp.path().to_path_buf());
    let start = Arc::new(Barrier::new(RACERS));
    let claimers: Vec<_> = (0..RACERS)
        .map(|index| {
            let (dir, start) = (dir.clone(), start.clone());
            thread::spawn(move || {
                start.wait();
                let claim = if index % 2 == 0 {
                    Claim::File
                } else {
                    Claim::Dir
                };
                unique_name(&dir, OsStr::new("Untitled.txt"), claim).unwrap()
            })
        })
        .collect();
    let claimed: HashSet<PathBuf> = claimers
        .into_iter()
        .map(|claimer| claimer.join().unwrap())
        .collect();

    assert_eq!(claimed.len(), RACERS);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), RACERS);
}
