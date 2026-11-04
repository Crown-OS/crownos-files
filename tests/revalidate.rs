use std::{fs, path::Path};

use crownos_files::fs::{
    mounts::MountTable,
    revalidate::{Freshness, revalidate, stamp_path},
};

#[test]
fn an_unchanged_directory_is_fresh_until_a_child_changes() {
    let temp = tempfile::tempdir().unwrap();
    let mounts = MountTable::read().unwrap();
    let stamp = stamp_path(temp.path()).unwrap();

    assert_eq!(
        revalidate(temp.path(), Some(stamp), &mounts),
        Freshness::Fresh
    );
    assert_eq!(revalidate(temp.path(), None, &mounts), Freshness::Stale);

    fs::write(temp.path().join("new"), b"").unwrap();
    assert_eq!(
        revalidate(temp.path(), Some(stamp), &mounts),
        Freshness::Stale
    );
}

#[test]
fn a_vanished_directory_is_stale() {
    let mounts = MountTable::default();
    let gone = Path::new("/definitely/not/here");
    let stamp = stamp_path(Path::new("/")).unwrap();
    assert_eq!(revalidate(gone, Some(stamp), &mounts), Freshness::Stale);
}
