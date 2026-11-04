use std::{fs, os::unix::fs::MetadataExt, path::Path, sync::Arc};

use crownos_files_core::{
    fs::lister::list_dir,
    model::{
        files_op::FilesOp,
        folder::{Folder, Stage},
        ticket::Ticket,
        view_options::ViewOptions,
    },
};
use rustix::io::Errno;
use tokio_util::sync::CancellationToken;

const FILE_COUNT: usize = 10_000;
const FIRST_CHUNK: usize = 256;

fn list(dir: &Arc<Path>, cancel: &CancellationToken) -> Vec<FilesOp> {
    let mut ops = Vec::new();
    list_dir(dir, Ticket::next(), cancel, |op| ops.push(op));
    ops
}

#[test]
fn streams_every_entry_of_a_large_directory_with_correct_stats() {
    let temp = tempfile::tempdir().unwrap();
    for index in 0..FILE_COUNT {
        fs::write(
            temp.path().join(format!("file{index}")),
            vec![b'x'; index % 100],
        )
        .unwrap();
    }
    fs::create_dir(temp.path().join("sub")).unwrap();
    let dir: Arc<Path> = temp.path().into();

    let ops = list(&dir, &CancellationToken::new());

    let Some((FilesOp::Done { stamp, .. }, parts)) = ops.split_last() else {
        panic!("listing did not end with Done");
    };
    assert_eq!(stamp.inode, fs::metadata(temp.path()).unwrap().ino());
    let FilesOp::Part { entries: first, .. } = &parts[0] else {
        panic!("listing did not start with a Part");
    };
    assert_eq!(first.len(), FIRST_CHUNK);
    let mut folder = Folder::new(dir.clone(), ViewOptions::default());
    for op in ops {
        folder.apply(op);
    }
    assert_eq!(folder.stage(), Stage::Loaded);
    assert_eq!(folder.len(), FILE_COUNT + 1);
    let names: Vec<_> = folder
        .iter()
        .take(4)
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, ["sub", "file0", "file1", "file2"]);
    assert!(folder.get(0).is_some_and(|entry| entry.is_dir()));
    for entry in folder.iter().skip(1) {
        let index: usize = entry.name["file".len()..].parse().unwrap();
        assert_eq!(entry.stat.len, (index % 100) as u64, "{}", entry.name);
        assert!(entry.stat.modified.system_time().is_some());
        assert!(!entry.stat.is_dummy());
    }
}

#[test]
fn an_unreadable_directory_fails_with_its_errno() {
    let dir: Arc<Path> = Path::new("/definitely/not/here").into();
    let ops = list(&dir, &CancellationToken::new());
    assert!(matches!(
        ops.as_slice(),
        [FilesOp::Fail {
            error: Errno::NOENT,
            ..
        }]
    ));
}

#[test]
fn a_cancelled_listing_emits_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("a"), b"").unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(list(&temp.path().into(), &cancel).is_empty());
}

#[test]
fn an_empty_directory_is_just_done() {
    let temp = tempfile::tempdir().unwrap();
    let ops = list(&temp.path().into(), &CancellationToken::new());
    assert!(matches!(ops.as_slice(), [FilesOp::Done { .. }]));
}
