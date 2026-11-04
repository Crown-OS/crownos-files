use zbus::zvariant::OwnedObjectPath;

use crate::{
    udisks::{
        objects::ObjectTree,
        proxies::{BLOCK, ENCRYPTED, FILESYSTEM},
    },
    util::dbus_props::{byte_paths, object},
};

/// One step of letting go of a drive before it is ejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    Unmount(OwnedObjectPath),
    Lock(OwnedObjectPath),
}

/// What to unmount and lock, in order, so nothing on `drive` is in use.
pub fn release_plan(tree: &ObjectTree, drive: &OwnedObjectPath) -> Vec<Release> {
    let mounted = |path: &OwnedObjectPath| {
        tree.interface(path, FILESYSTEM)
            .is_some_and(|filesystem| !byte_paths(filesystem, "MountPoints").is_empty())
    };
    let mut plan = Vec::new();
    let on_drive = tree.objects().filter(|(_, interfaces)| {
        interfaces
            .get(BLOCK)
            .and_then(|block| object(block, "Drive"))
            .is_some_and(|owner| owner == *drive)
    });
    for (path, interfaces) in on_drive {
        if let Some(encrypted) = interfaces.get(ENCRYPTED) {
            let Some(cleartext) = object(encrypted, "CleartextDevice") else {
                continue;
            };
            if mounted(&cleartext) {
                plan.push(Release::Unmount(cleartext));
            }
            plan.push(Release::Lock(path.clone()));
        } else if mounted(path) {
            plan.push(Release::Unmount(path.clone()));
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udisks::fixtures::*;

    #[test]
    fn eject_unmounts_and_locks_only_the_drive_being_ejected() {
        let mut tree = sample_tree();
        assert_eq!(
            release_plan(&tree, &path(STICK)),
            [Release::Unmount(block_path("sdb1"))]
        );
        let cleartext = path("/org/freedesktop/UDisks2/block_devices/dm_2d0");
        tree.add_interfaces(
            block_path("sdb2"),
            interfaces(vec![(ENCRYPTED, encrypted(cleartext.as_str()))]),
        );
        tree.add_interfaces(
            cleartext.clone(),
            interfaces(vec![(FILESYSTEM, filesystem(&["/run/media/u/secret"]))]),
        );
        let mut plan = release_plan(&tree, &path(STICK));
        plan.sort_by_key(|step| matches!(step, Release::Lock(_)));
        assert_eq!(plan.len(), 3);
        assert!(plan.contains(&Release::Unmount(cleartext)));
        assert_eq!(plan[2], Release::Lock(block_path("sdb2")));
        assert!(
            release_plan(&tree, &path(DISK)).contains(&Release::Unmount(block_path("nvme0n1p2")))
        );
    }
}
