use std::{fs, path::Path, sync::Arc, time::Duration};

use crownos_files_core::{
    fs::{mounts::MountTable, watcher::Watcher},
    model::files_op::FilesOp,
    util::hash::FastHashSet,
};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, unbounded_channel},
        watch,
    },
    time::{error::Elapsed, sleep, timeout},
};

const SETTLE: Duration = Duration::from_millis(500);
const PATIENCE: Duration = Duration::from_secs(20);

#[derive(Default)]
struct Seen {
    upserted: FastHashSet<String>,
    deleted: FastHashSet<String>,
}

async fn wait_until(
    ops: &mut UnboundedReceiver<FilesOp>,
    dir: &Path,
    done: impl Fn(&Seen) -> bool,
) -> Result<Seen, Elapsed> {
    let mut seen = Seen::default();
    let collect = async {
        while !done(&seen) {
            match ops.recv().await {
                Some(FilesOp::Upsert {
                    dir: changed,
                    entries,
                }) if *changed == *dir => {
                    seen.upserted
                        .extend(entries.iter().map(|entry| entry.name.to_string()));
                }
                Some(FilesOp::Delete {
                    dir: changed,
                    names,
                }) if *changed == *dir => {
                    seen.deleted.extend(names.iter().map(ToString::to_string));
                }
                Some(_) => {}
                None => break,
            }
        }
    };
    timeout(PATIENCE, collect).await.map(|()| seen)
}

#[tokio::test]
async fn reports_creates_renames_and_deletes() {
    let temp = tempfile::tempdir().unwrap();
    let dir: Arc<Path> = temp.path().into();
    let (sender, mut ops) = unbounded_channel();
    let (_mounts, table) = watch::channel(Arc::new(MountTable::read().unwrap()));
    let watcher = Watcher::spawn(sender, table).unwrap();
    watcher.watch(vec![dir.clone()]);
    sleep(SETTLE).await;

    fs::write(dir.join("created.txt"), b"hi").unwrap();
    let seen = wait_until(&mut ops, &dir, |seen| seen.upserted.contains("created.txt"))
        .await
        .unwrap();
    assert!(seen.deleted.is_empty());

    fs::rename(dir.join("created.txt"), dir.join("renamed.txt")).unwrap();
    wait_until(&mut ops, &dir, |seen| {
        seen.deleted.contains("created.txt") && seen.upserted.contains("renamed.txt")
    })
    .await
    .unwrap();

    fs::remove_file(dir.join("renamed.txt")).unwrap();
    wait_until(&mut ops, &dir, |seen| seen.deleted.contains("renamed.txt"))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_burst_of_writes_to_one_file_is_one_upsert() {
    let temp = tempfile::tempdir().unwrap();
    let dir: Arc<Path> = temp.path().into();
    let (sender, mut ops) = unbounded_channel();
    let (_mounts, table) = watch::channel(Arc::new(MountTable::read().unwrap()));
    let watcher = Watcher::spawn(sender, table).unwrap();
    watcher.watch(vec![dir.clone()]);
    sleep(SETTLE).await;

    for round in 0..50 {
        fs::write(dir.join("busy.log"), vec![b'x'; round]).unwrap();
    }
    let first = timeout(PATIENCE, ops.recv()).await.unwrap().unwrap();
    let FilesOp::Upsert { entries, .. } = first else {
        panic!("expected an upsert, got {first:?}");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].stat.len, 49);
}

#[tokio::test]
async fn unwatched_directories_report_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let dir: Arc<Path> = temp.path().into();
    let (sender, mut ops) = unbounded_channel();
    let (_mounts, table) = watch::channel(Arc::new(MountTable::read().unwrap()));
    let watcher = Watcher::spawn(sender, table).unwrap();
    watcher.watch(vec![dir.clone()]);
    sleep(SETTLE).await;
    watcher.watch(Vec::new());
    sleep(SETTLE).await;

    fs::write(dir.join("ignored"), b"").unwrap();
    assert!(
        timeout(Duration::from_millis(600), ops.recv())
            .await
            .is_err()
    );
}
