use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::{
    udisks::{
        objects::{Interfaces, ObjectTree},
        proxies::{BLOCK, DRIVE, ENCRYPTED, FILESYSTEM, LOOP, PARTITION},
    },
    util::dbus_props::Properties,
};

pub const USER: u32 = 1000;
pub const STICK: &str = "/org/freedesktop/UDisks2/drives/Stick";
pub const DISK: &str = "/org/freedesktop/UDisks2/drives/Disk";

pub fn path(text: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(text).unwrap()
}

pub fn block_path(name: &str) -> OwnedObjectPath {
    path(&format!("/org/freedesktop/UDisks2/block_devices/{name}"))
}

pub fn properties(entries: Vec<(&str, Value<'static>)>) -> Properties {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), OwnedValue::try_from(value).unwrap()))
        .collect()
}

fn byte_string(text: &str) -> Value<'static> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    Value::from(bytes)
}

fn object_value(text: &str) -> Value<'static> {
    Value::from(ObjectPath::try_from(text.to_owned()).unwrap())
}

pub fn block(name: &str, usage: &str, fs_type: &str, drive: &str) -> Properties {
    properties(vec![
        ("Device", byte_string(&format!("/dev/{name}"))),
        ("IdUsage", Value::from(usage.to_owned())),
        ("IdType", Value::from(fs_type.to_owned())),
        ("IdLabel", Value::from(format!("{name}-label"))),
        ("Size", Value::from(1_000_u64)),
        ("Drive", object_value(drive)),
        ("CryptoBackingDevice", object_value("/")),
        ("HintAuto", Value::from(drive == STICK)),
        ("HintSystem", Value::from(drive != STICK)),
    ])
}

pub fn with(mut base: Properties, extra: Vec<(&str, Value<'static>)>) -> Properties {
    base.extend(properties(extra));
    base
}

pub fn filesystem(mount_points: &[&str]) -> Properties {
    let points: Vec<Vec<u8>> = mount_points
        .iter()
        .map(|point| [point.as_bytes(), &[0]].concat())
        .collect();
    properties(vec![("MountPoints", Value::from(points))])
}

pub fn encrypted(cleartext: &str) -> Properties {
    properties(vec![("CleartextDevice", object_value(cleartext))])
}

pub fn looped(uid: u32) -> Properties {
    properties(vec![("SetupByUID", Value::from(uid))])
}

pub fn partition_of(table: &OwnedObjectPath) -> Properties {
    properties(vec![("Table", object_value(table.as_str()))])
}

pub fn drive(removable: bool) -> Properties {
    properties(vec![
        ("Vendor", Value::from("Acme")),
        ("Model", Value::from("Stick")),
        ("Removable", Value::from(removable)),
        ("Ejectable", Value::from(false)),
        ("CanPowerOff", Value::from(removable)),
        (
            "ConnectionBus",
            Value::from(if removable { "usb" } else { "" }),
        ),
    ])
}

pub fn interfaces(entries: Vec<(&str, Properties)>) -> Interfaces {
    entries
        .into_iter()
        .map(|(name, properties)| (name.to_owned(), properties))
        .collect()
}

/// Two drives: a USB stick with a mounted FAT partition and a locked LUKS
/// partition, and an internal disk with a system root, a swap partition and
/// its partition table.
pub fn sample_tree() -> ObjectTree {
    let mut tree = ObjectTree::default();
    tree.add_interfaces(path(STICK), interfaces(vec![(DRIVE, drive(true))]));
    tree.add_interfaces(path(DISK), interfaces(vec![(DRIVE, drive(false))]));
    tree.add_interfaces(
        block_path("sdb1"),
        interfaces(vec![
            (BLOCK, block("sdb1", "filesystem", "vfat", STICK)),
            (FILESYSTEM, filesystem(&["/run/media/u/STICK"])),
        ]),
    );
    tree.add_interfaces(
        block_path("sdb2"),
        interfaces(vec![
            (BLOCK, block("sdb2", "crypto", "crypto_LUKS", STICK)),
            (ENCRYPTED, encrypted("/")),
        ]),
    );
    tree.add_interfaces(
        block_path("nvme0n1"),
        interfaces(vec![
            (BLOCK, block("nvme0n1", "", "", DISK)),
            ("org.freedesktop.UDisks2.PartitionTable", Properties::new()),
        ]),
    );
    tree.add_interfaces(
        block_path("nvme0n1p2"),
        interfaces(vec![
            (BLOCK, block("nvme0n1p2", "filesystem", "btrfs", DISK)),
            (FILESYSTEM, filesystem(&["/"])),
        ]),
    );
    tree.add_interfaces(
        block_path("nvme0n1p3"),
        interfaces(vec![(
            BLOCK,
            with(
                block("nvme0n1p3", "other", "swap", DISK),
                vec![("HintAuto", Value::from(true))],
            ),
        )]),
    );
    tree
}

/// A loop block UDisks does not consider a system device.
fn loop_block(name: &str) -> Properties {
    with(
        block(name, "filesystem", "ext4", "/"),
        vec![("HintSystem", Value::from(false))],
    )
}

pub fn add_loop(tree: &mut ObjectTree, name: &str, uid: u32) {
    tree.add_interfaces(
        block_path(name),
        interfaces(vec![
            (BLOCK, loop_block(name)),
            (FILESYSTEM, filesystem(&[])),
            (LOOP, looped(uid)),
        ]),
    );
}

pub fn add_loop_partition(tree: &mut ObjectTree, table: &str, name: &str) {
    tree.add_interfaces(
        block_path(name),
        interfaces(vec![
            (BLOCK, loop_block(name)),
            (FILESYSTEM, filesystem(&[])),
            (PARTITION, partition_of(&block_path(table))),
        ]),
    );
}
