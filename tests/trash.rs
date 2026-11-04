#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::{fs, path::Path};

use crownos_files::{
    jobs::{ConflictPolicy, ConflictReply, Job, Outcome, Reversible, home_trash, list_trash},
    model::trash::TrashedItem,
};
use support::Jobs;

fn trashed_from<'a>(items: &'a [TrashedItem], dir: &Path) -> Vec<&'a TrashedItem> {
    items
        .iter()
        .filter(|item| item.info.original.starts_with(dir))
        .collect()
}

/// One test, since it points `XDG_DATA_HOME` at a temporary directory for
/// the whole process.
#[tokio::test]
async fn trash_list_restore_and_delete_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let data_home = temp.path().join("data");
    let work = temp.path().join("work");
    fs::create_dir_all(&data_home).unwrap();
    fs::create_dir_all(work.join("folder/inner")).unwrap();
    // SAFETY: this binary runs a single test, so no other thread reads the
    // environment concurrently.
    unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
    assert_eq!(home_trash(), Some(data_home.join("Trash")));
    let work = fs::canonicalize(&work).unwrap();
    let (file, folder) = (work.join("my file.txt"), work.join("folder"));
    fs::write(&file, b"hello").unwrap();
    fs::write(folder.join("inner/x"), b"x").unwrap();

    let mut jobs = Jobs::new();
    let trashed = jobs
        .run(Job::Trash(vec![file.clone(), folder.clone()]), None)
        .await;
    assert_eq!(
        trashed.outcome,
        Some(Outcome::Completed),
        "{:?}",
        trashed.error
    );
    assert!(!file.exists() && !folder.exists());
    let info = fs::read_to_string(data_home.join("Trash/info/my file.txt.trashinfo")).unwrap();
    assert!(info.starts_with("[Trash Info]\nPath="));
    assert!(info.contains("my%20file.txt\nDeletionDate="));
    assert!(data_home.join("Trash/files/folder/inner/x").exists());

    let listed = list_trash().await.unwrap();
    let mine = trashed_from(&listed, &work);
    assert_eq!(mine.len(), 2);
    let listed_file = mine.iter().find(|item| item.info.original == file).unwrap();
    assert_eq!((listed_file.size, listed_file.is_dir), (Some(5), false));
    assert!(listed_file.info.deleted_at.is_some());

    let Some(Reversible::Trash(items)) = trashed.undo else {
        panic!("trashing is undoable");
    };
    let restored = jobs.run(Reversible::Trash(items).inverse(), None).await;
    assert_eq!(restored.outcome, Some(Outcome::Completed));
    assert_eq!(fs::read(&file).unwrap(), b"hello");
    assert!(folder.join("inner/x").exists());
    assert!(trashed_from(&list_trash().await.unwrap(), &work).is_empty());

    let again = jobs.run(Job::Trash(vec![file.clone()]), None).await;
    fs::write(&file, b"replacement").unwrap();
    let again_listed = list_trash().await.unwrap();
    let in_trash: Vec<TrashedItem> = trashed_from(&again_listed, &work)
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(in_trash.len(), 1);
    let keep_both = Some(ConflictReply {
        policy: ConflictPolicy::KeepBoth,
        apply_to_all: false,
    });
    let restored = jobs.run(Job::Restore(in_trash), keep_both).await;
    assert_eq!(
        (restored.outcome, restored.conflicts),
        (Some(Outcome::Completed), 1)
    );
    assert_eq!(fs::read(work.join("my file (2).txt")).unwrap(), b"hello");
    assert_eq!(fs::read(&file).unwrap(), b"replacement");
    assert!(again.undo.is_some());

    let last = jobs.run(Job::Trash(vec![file.clone()]), None).await;
    let Some(Reversible::Trash(items)) = last.undo else {
        panic!("trashing is undoable");
    };
    let deleted = jobs.run(Job::DeleteFromTrash(items), None).await;
    assert_eq!(deleted.outcome, Some(Outcome::Completed));
    assert!(trashed_from(&list_trash().await.unwrap(), &work).is_empty());
    assert_eq!(
        fs::read_dir(data_home.join("Trash/files")).unwrap().count(),
        0
    );
    assert_eq!(
        fs::read_dir(data_home.join("Trash/info")).unwrap().count(),
        0
    );
}
