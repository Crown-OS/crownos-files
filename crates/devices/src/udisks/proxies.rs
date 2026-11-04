use std::collections::HashMap;

use zbus::{
    proxy,
    zvariant::{OwnedObjectPath, Value},
};

pub const SERVICE: &str = "org.freedesktop.UDisks2";
pub const ROOT: &str = "/org/freedesktop/UDisks2";

pub const BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
pub const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
pub const LOOP: &str = "org.freedesktop.UDisks2.Loop";
pub const PARTITION: &str = "org.freedesktop.UDisks2.Partition";

/// The `a{sv}` options every UDisks method takes.
pub type Options<'a> = HashMap<&'a str, Value<'a>>;

#[proxy(
    interface = "org.freedesktop.UDisks2.Manager",
    default_service = "org.freedesktop.UDisks2",
    default_path = "/org/freedesktop/UDisks2/Manager",
    gen_blocking = false
)]
pub trait Manager {
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2",
    gen_blocking = false
)]
pub trait Filesystem {
    fn mount(&self, options: &Options<'_>) -> zbus::Result<String>;
    fn unmount(&self, options: &Options<'_>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Encrypted",
    default_service = "org.freedesktop.UDisks2",
    gen_blocking = false
)]
pub trait Encrypted {
    fn unlock(&self, passphrase: &str, options: &Options<'_>) -> zbus::Result<OwnedObjectPath>;
    fn lock(&self, options: &Options<'_>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Drive",
    default_service = "org.freedesktop.UDisks2",
    gen_blocking = false
)]
pub trait Drive {
    fn eject(&self, options: &Options<'_>) -> zbus::Result<()>;
    fn power_off(&self, options: &Options<'_>) -> zbus::Result<()>;
}
