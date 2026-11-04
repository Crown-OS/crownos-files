use zbus::zvariant::OwnedObjectPath;

use crate::{
    udisks::{
        objects::{Interfaces, ObjectTree},
        proxies::{BLOCK, ENCRYPTED, FILESYSTEM, LOOP, PARTITION},
    },
    util::dbus_props::{Properties, flag, get, object, text},
};

const SHOWN_USAGES: [&str; 2] = ["filesystem", "crypto"];

/// Whether the block object `interfaces` belongs in the Devices list for
/// user `uid`. Hidden: ignored and swap devices, partition tables, cleartext
/// devices (shown under their encrypted parent), system partitions UDisks
/// would not mount on its own, and loop devices of other users.
pub fn is_user_visible(tree: &ObjectTree, interfaces: &Interfaces, uid: u32) -> bool {
    let Some(block) = interfaces.get(BLOCK) else {
        return false;
    };
    let mountable = interfaces.contains_key(FILESYSTEM) || interfaces.contains_key(ENCRYPTED);
    let usage_shown = text(block, "IdUsage").is_some_and(|usage| SHOWN_USAGES.contains(&usage));
    mountable
        && usage_shown
        && !flag(block, "HintIgnore")
        && object(block, "CryptoBackingDevice").is_none()
        && (!flag(block, "HintSystem") || flag(block, "HintAuto"))
        && loop_owner(tree, interfaces).is_none_or(|owner| owner == uid)
}

/// The user that set up the loop device behind a block, or its partition table.
fn loop_owner(tree: &ObjectTree, interfaces: &Interfaces) -> Option<u32> {
    let table_loop = || {
        let table: OwnedObjectPath = object(interfaces.get(PARTITION)?, "Table")?;
        tree.interface(&table, LOOP)
    };
    let setup: &Properties = interfaces.get(LOOP).or_else(table_loop)?;
    Some(get(setup, "SetupByUID").unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::Value;

    use super::*;
    use crate::udisks::{fixtures::*, volume::Volume};

    fn visible(tree: &ObjectTree, name: &str) -> bool {
        Volume::from_tree(tree, &block_path(name), USER).is_some()
    }

    #[test]
    fn shows_user_filesystems_and_hides_system_swap_and_tables() {
        let tree = sample_tree();
        assert!(visible(&tree, "sdb1"));
        assert!(visible(&tree, "sdb2"));
        assert!(!visible(&tree, "nvme0n1"), "partition table");
        assert!(
            !visible(&tree, "nvme0n1p2"),
            "system partition without HintAuto"
        );
        assert!(!visible(&tree, "nvme0n1p3"), "swap");
    }

    #[test]
    fn hint_ignore_hides_and_hint_auto_reveals_system_partitions() {
        let mut tree = sample_tree();
        let ignored = with(
            block("sdb1", "filesystem", "vfat", STICK),
            vec![("HintIgnore", Value::from(true))],
        );
        tree.add_interfaces(block_path("sdb1"), interfaces(vec![(BLOCK, ignored)]));
        assert!(!visible(&tree, "sdb1"));
        let data = with(
            block("nvme0n1p2", "filesystem", "btrfs", DISK),
            vec![("HintAuto", Value::from(true))],
        );
        tree.add_interfaces(block_path("nvme0n1p2"), interfaces(vec![(BLOCK, data)]));
        assert!(visible(&tree, "nvme0n1p2"));
    }

    #[test]
    fn loop_devices_show_only_for_the_user_that_set_them_up() {
        let mut tree = sample_tree();
        add_loop(&mut tree, "loop0", USER);
        add_loop(&mut tree, "loop1", 0);
        add_loop(&mut tree, "loop2", USER + 1);
        add_loop_partition(&mut tree, "loop2", "loop2p1");
        assert!(visible(&tree, "loop0"));
        assert!(!visible(&tree, "loop1"));
        assert!(
            !visible(&tree, "loop2p1"),
            "partition of another user's loop"
        );
    }

    #[test]
    fn cleartext_devices_show_under_their_encrypted_parent() {
        let mut tree = sample_tree();
        let cleartext = "/org/freedesktop/UDisks2/block_devices/dm_2d0";
        tree.add_interfaces(
            block_path("sdb2"),
            interfaces(vec![(ENCRYPTED, encrypted(cleartext))]),
        );
        let inner = with(
            block("dm-0", "filesystem", "ext4", "/"),
            vec![
                (
                    "CryptoBackingDevice",
                    Value::from(block_path("sdb2").into_inner()),
                ),
                ("IdLabel", Value::from("")),
            ],
        );
        tree.add_interfaces(
            path(cleartext),
            interfaces(vec![
                (BLOCK, inner),
                (FILESYSTEM, filesystem(&["/run/media/u/secret"])),
            ]),
        );
        assert!(Volume::from_tree(&tree, &path(cleartext), USER).is_none());
        let parent = Volume::from_tree(&tree, &block_path("sdb2"), USER).unwrap();
        assert!(parent.is_encrypted && parent.is_mounted());
        assert_eq!(parent.fs_type, "ext4");
        assert_eq!(parent.label, "sdb2-label");
        assert_eq!(parent.filesystem_object(), Some(&path(cleartext)));
        assert_eq!(
            parent.mount_points,
            [std::path::PathBuf::from("/run/media/u/secret")]
        );
    }

    #[test]
    fn volumes_carry_their_drive_and_hints() {
        let tree = sample_tree();
        let stick = Volume::from_tree(&tree, &block_path("sdb1"), USER).unwrap();
        let drive = stick.drive.as_ref().unwrap();
        assert!(stick.is_removable() && drive.can_power_off && !drive.ejectable);
        assert_eq!(drive.connection_bus, "usb");
        assert_eq!(stick.device, std::path::Path::new("/dev/sdb1"));
        assert_eq!(stick.name(), "sdb1-label");
        assert!(stick.hints.auto && !stick.hints.system);
        let locked = Volume::from_tree(&tree, &block_path("sdb2"), USER).unwrap();
        assert_eq!(locked.filesystem_object(), None);
    }
}
