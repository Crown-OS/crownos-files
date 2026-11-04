use std::{path::PathBuf, time::Duration};

use zbus::{
    Connection,
    fdo::ObjectManagerProxy,
    proxy::{Builder, CacheProperties, Defaults},
    zvariant::OwnedObjectPath,
};

use crate::udisks::{
    error::DeviceError,
    objects::ObjectTree,
    proxies::{DriveProxy, EncryptedProxy, FilesystemProxy, Options, ROOT, SERVICE},
    release::{Release, release_plan},
    volume::Volume,
};

const CLEARTEXT_ATTEMPTS: u32 = 10;
const CLEARTEXT_RETRY: Duration = Duration::from_millis(100);
const NOT_READY_ERRORS: [&str; 3] = [
    "org.freedesktop.DBus.Error.UnknownMethod",
    "org.freedesktop.DBus.Error.UnknownInterface",
    "org.freedesktop.DBus.Error.UnknownObject",
];

/// The system bus link to UDisks, for mounting, unlocking and ejecting.
/// Methods pass empty options, so polkit asks the user when it must.
#[derive(Debug, Clone)]
pub struct Udisks {
    connection: Connection,
}

impl Udisks {
    pub async fn connect() -> Result<Self, DeviceError> {
        Ok(Self {
            connection: Connection::system().await?,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub async fn managed_objects(&self) -> Result<ObjectTree, DeviceError> {
        let manager = ObjectManagerProxy::builder(&self.connection)
            .destination(SERVICE)?
            .path(ROOT)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let managed = manager
            .get_managed_objects()
            .await
            .map_err(zbus::Error::from)?;
        Ok(ObjectTree::from_managed(managed))
    }

    /// Mounts the volume and returns where.
    pub async fn mount(&self, volume: &Volume) -> Result<PathBuf, DeviceError> {
        let filesystem = volume.filesystem_object().ok_or(DeviceError::Locked)?;
        Ok(self.try_mount(filesystem).await?)
    }

    pub async fn unmount(&self, volume: &Volume) -> Result<(), DeviceError> {
        let filesystem = volume.filesystem_object().ok_or(DeviceError::NotMounted)?;
        self.unmount_object(filesystem).await
    }

    /// Unlocks an encrypted volume and mounts what is inside.
    pub async fn unlock(&self, volume: &Volume, passphrase: &str) -> Result<PathBuf, DeviceError> {
        let encrypted: EncryptedProxy = self.proxy(&volume.object_path).await?;
        let cleartext = encrypted.unlock(passphrase, &Options::new()).await?;
        self.mount_when_ready(&cleartext).await
    }

    /// Unmounts an unlocked volume, then locks it.
    pub async fn lock(&self, volume: &Volume) -> Result<(), DeviceError> {
        if volume.is_mounted() {
            self.unmount(volume).await?;
        }
        self.lock_object(&volume.object_path).await
    }

    /// Unmounts and locks everything on the volume's drive, then ejects the
    /// media and powers the drive off where it supports that.
    pub async fn eject(&self, volume: &Volume) -> Result<(), DeviceError> {
        let drive = volume.drive.as_ref().ok_or(DeviceError::NoDrive)?;
        let tree = self.managed_objects().await?;
        for step in release_plan(&tree, &drive.object_path) {
            match step {
                Release::Unmount(path) => match self.unmount_object(&path).await {
                    Err(DeviceError::NotMounted) => Ok(()),
                    other => other,
                }?,
                Release::Lock(path) => self.lock_object(&path).await?,
            }
        }
        let proxy: DriveProxy = self.proxy(&drive.object_path).await?;
        if drive.ejectable {
            proxy.eject(&Options::new()).await?;
        }
        if drive.can_power_off {
            proxy.power_off(&Options::new()).await?;
        }
        Ok(())
    }

    async fn try_mount(&self, path: &OwnedObjectPath) -> zbus::Result<PathBuf> {
        let filesystem: FilesystemProxy = self.proxy(path).await?;
        filesystem.mount(&Options::new()).await.map(PathBuf::from)
    }

    /// Mounts a just-unlocked device, waiting for UDisks to probe its filesystem.
    async fn mount_when_ready(&self, path: &OwnedObjectPath) -> Result<PathBuf, DeviceError> {
        for _ in 1..CLEARTEXT_ATTEMPTS {
            match self.try_mount(path).await {
                Err(error) if is_not_ready(&error) => tokio::time::sleep(CLEARTEXT_RETRY).await,
                other => return Ok(other?),
            }
        }
        Ok(self.try_mount(path).await?)
    }

    async fn unmount_object(&self, path: &OwnedObjectPath) -> Result<(), DeviceError> {
        let filesystem: FilesystemProxy = self.proxy(path).await?;
        Ok(filesystem.unmount(&Options::new()).await?)
    }

    async fn lock_object(&self, path: &OwnedObjectPath) -> Result<(), DeviceError> {
        let encrypted: EncryptedProxy = self.proxy(path).await?;
        Ok(encrypted.lock(&Options::new()).await?)
    }

    async fn proxy<'a, P>(&self, path: &'a OwnedObjectPath) -> zbus::Result<P>
    where
        P: Defaults + From<zbus::Proxy<'a>>,
    {
        Builder::new(&self.connection)
            .path(path.as_ref())?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    }
}

fn is_not_ready(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::MethodError(name, ..) if NOT_READY_ERRORS.contains(&name.as_str()))
}
