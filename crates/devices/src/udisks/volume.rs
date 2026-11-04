use std::path::PathBuf;

use compact_str::CompactString;
use zbus::zvariant::OwnedObjectPath;

use crate::{
    udisks::{
        objects::ObjectTree,
        proxies::{BLOCK, DRIVE, ENCRYPTED, FILESYSTEM},
        visibility::is_user_visible,
    },
    util::dbus_props::{Properties, byte_path, byte_paths, flag, get, object, text},
};

/// What UDisks hints about presenting a block device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub ignore: bool,
    pub system: bool,
    pub auto: bool,
    pub name: Option<CompactString>,
    pub icon: Option<CompactString>,
}

impl Hints {
    fn from_block(block: &Properties) -> Self {
        Self {
            ignore: flag(block, "HintIgnore"),
            system: flag(block, "HintSystem"),
            auto: flag(block, "HintAuto"),
            name: text(block, "HintName").map(CompactString::from),
            icon: text(block, "HintIconName").map(CompactString::from),
        }
    }
}

/// The hardware a volume lives on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drive {
    pub object_path: OwnedObjectPath,
    pub vendor: CompactString,
    pub model: CompactString,
    /// The drive or its media can be removed: USB sticks, SD cards, discs.
    pub removable: bool,
    pub ejectable: bool,
    pub can_power_off: bool,
    /// `usb`, `sdio`, `ieee1394`, or empty for internal buses.
    pub connection_bus: CompactString,
}

impl Drive {
    fn from_tree(tree: &ObjectTree, path: OwnedObjectPath) -> Option<Self> {
        let drive = tree.interface(&path, DRIVE)?;
        let owned = |key| {
            text(drive, key)
                .map(CompactString::from)
                .unwrap_or_default()
        };
        Some(Self {
            vendor: owned("Vendor"),
            model: owned("Model"),
            removable: flag(drive, "Removable") || flag(drive, "MediaRemovable"),
            ejectable: flag(drive, "Ejectable"),
            can_power_off: flag(drive, "CanPowerOff"),
            connection_bus: owned("ConnectionBus"),
            object_path: path,
        })
    }
}

/// One entry of the Devices list: a filesystem, or an encrypted container
/// with the filesystem inside it once unlocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub object_path: OwnedObjectPath,
    pub device: PathBuf,
    pub label: CompactString,
    pub size: u64,
    pub fs_type: CompactString,
    pub mount_points: Vec<PathBuf>,
    pub drive: Option<Drive>,
    pub is_encrypted: bool,
    pub cleartext_device: Option<OwnedObjectPath>,
    pub hints: Hints,
}

impl Volume {
    /// The volume of the block object at `path`, when user `uid` should see it.
    pub fn from_tree(tree: &ObjectTree, path: &OwnedObjectPath, uid: u32) -> Option<Self> {
        let interfaces = tree.get(path)?;
        if !is_user_visible(tree, interfaces, uid) {
            return None;
        }
        let block = interfaces.get(BLOCK)?;
        let encrypted = interfaces.get(ENCRYPTED);
        let cleartext_device = encrypted.and_then(|_| cleartext_of(tree, path));
        let cleartext_block = cleartext_device
            .as_ref()
            .and_then(|cleartext| tree.interface(cleartext, BLOCK));
        let filesystem = match &cleartext_device {
            Some(cleartext) => tree.interface(cleartext, FILESYSTEM),
            None => interfaces.get(FILESYSTEM),
        };
        let inner = |key| cleartext_block.and_then(|inner| text(inner, key));
        Some(Self {
            object_path: path.clone(),
            device: byte_path(block, "PreferredDevice")
                .or_else(|| byte_path(block, "Device"))
                .unwrap_or_default(),
            label: text(block, "IdLabel")
                .or_else(|| inner("IdLabel"))
                .map(CompactString::from)
                .unwrap_or_default(),
            size: get(block, "Size").unwrap_or(0),
            fs_type: inner("IdType")
                .or_else(|| text(block, "IdType"))
                .map(CompactString::from)
                .unwrap_or_default(),
            mount_points: filesystem
                .map(|filesystem| byte_paths(filesystem, "MountPoints"))
                .unwrap_or_default(),
            drive: object(block, "Drive").and_then(|drive| Drive::from_tree(tree, drive)),
            is_encrypted: encrypted.is_some(),
            cleartext_device,
            hints: Hints::from_block(block),
        })
    }

    /// The object to mount or unmount: the cleartext device of an unlocked
    /// container, `None` for a locked one.
    pub fn filesystem_object(&self) -> Option<&OwnedObjectPath> {
        if self.is_encrypted {
            self.cleartext_device.as_ref()
        } else {
            Some(&self.object_path)
        }
    }

    pub fn is_mounted(&self) -> bool {
        !self.mount_points.is_empty()
    }

    pub fn is_removable(&self) -> bool {
        self.drive.as_ref().is_some_and(|drive| drive.removable)
    }

    /// The name to show: UDisks' hint, the label, the drive model, the device.
    pub fn name(&self) -> &str {
        self.hints
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or_else(|| Some(self.label.as_str()).filter(|label| !label.is_empty()))
            .or_else(|| self.drive.as_ref().map(|drive| drive.model.as_str()))
            .filter(|name| !name.is_empty())
            .or_else(|| self.device.file_name().and_then(|name| name.to_str()))
            .unwrap_or_default()
    }
}

/// The unlocked device of the container at `path`, from `CleartextDevice` or,
/// on older UDisks, the block backed by it.
fn cleartext_of(tree: &ObjectTree, path: &OwnedObjectPath) -> Option<OwnedObjectPath> {
    tree.interface(path, ENCRYPTED)
        .and_then(|encrypted| object(encrypted, "CleartextDevice"))
        .or_else(|| {
            tree.objects()
                .find(|(_, interfaces)| {
                    interfaces
                        .get(BLOCK)
                        .and_then(|block| object(block, "CryptoBackingDevice"))
                        .is_some_and(|backing| backing == *path)
                })
                .map(|(cleartext, _)| cleartext.clone())
        })
}
