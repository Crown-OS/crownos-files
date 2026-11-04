use std::{
    collections::HashMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    sync::Arc,
};

use crownos_files::{
    fs::{lister::list_dir, stat_at::stat_entry},
    model::{entry::Entry, files_op::FilesOp, kind::FileKind, ticket::Ticket},
};
use rustix::fs::{CWD, Mode, OFlags, openat};
use tokio_util::sync::CancellationToken;

fn listed(dir: &Path) -> HashMap<String, Entry> {
    let dir: Arc<Path> = dir.into();
    let mut entries = Vec::new();
    list_dir(&dir, Ticket::next(), &CancellationToken::new(), |op| {
        if let FilesOp::Part { entries: chunk, .. } = op {
            entries.extend(chunk);
        }
    });
    entries
        .into_iter()
        .map(|entry| (entry.name.to_string(), entry))
        .collect()
}

#[test]
fn symlinks_describe_their_target_and_dangling_ones_are_broken() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("target.txt"), b"hello").unwrap();
    fs::create_dir(root.join("folder")).unwrap();
    symlink("target.txt", root.join("link")).unwrap();
    symlink("folder", root.join("folder-link")).unwrap();
    symlink("nowhere", root.join("broken")).unwrap();
    fs::write(root.join(".hidden"), b"").unwrap();
    fs::write(root.join("run.sh"), b"#!/bin/sh").unwrap();
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();

    let entries = listed(root);
    assert_eq!(entries.len(), 7);

    let link = &entries["link"];
    assert!(link.stat.is_symlink() && !link.stat.is_broken());
    assert_eq!(link.stat.len, 5);
    assert_eq!(link.link_target(), Some(Path::new("target.txt")));

    let folder_link = &entries["folder-link"];
    assert!(folder_link.is_dir() && folder_link.stat.is_symlink());
    assert_eq!(folder_link.kind, FileKind::Folder);

    let broken = &entries["broken"];
    assert!(broken.stat.is_symlink() && broken.stat.is_broken());
    assert_eq!(broken.link_target(), Some(Path::new("nowhere")));

    assert!(&entries[".hidden"].is_hidden());
    assert!(&entries["run.sh"].stat.is_executable());
    assert!(!&entries["target.txt"].stat.is_executable());
    assert!(&entries["target.txt"].extra.info().is_none());
}

#[test]
fn a_name_that_cannot_be_statted_lists_as_a_dummy() {
    let temp = tempfile::tempdir().unwrap();
    let dir = openat(
        CWD,
        temp.path(),
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .unwrap();
    let ghost = stat_entry(std::os::fd::AsFd::as_fd(&dir), b".ghost");
    assert!(ghost.stat.is_dummy() && ghost.is_hidden());
    assert_eq!(ghost.name, ".ghost");
}
