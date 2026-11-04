const NOT_AUTHORIZED: &str = "org.freedesktop.UDisks2.Error.NotAuthorized";
const NOT_AUTHORIZED_CAN_OBTAIN: &str = "org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain";
const NOT_AUTHORIZED_DISMISSED: &str = "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed";
const CANCELLED: &str = "org.freedesktop.UDisks2.Error.Cancelled";
const DEVICE_BUSY: &str = "org.freedesktop.UDisks2.Error.DeviceBusy";
const ALREADY_MOUNTED: &str = "org.freedesktop.UDisks2.Error.AlreadyMounted";
const NOT_MOUNTED: &str = "org.freedesktop.UDisks2.Error.NotMounted";

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("not authorized")]
    NotAuthorized,
    #[error("cancelled")]
    Cancelled,
    #[error("the device is busy")]
    Busy,
    #[error("the volume is already mounted")]
    AlreadyMounted,
    #[error("the volume is not mounted")]
    NotMounted,
    #[error("the volume is locked")]
    Locked,
    #[error("the volume has no drive to eject")]
    NoDrive,
    #[error("{0}")]
    Failed(String),
    #[error("cannot reach UDisks: {0}")]
    Bus(zbus::Error),
}

impl DeviceError {
    /// Maps a UDisks D-Bus error by name; `message` is shown for the rest.
    pub fn from_dbus(name: &str, message: Option<&str>) -> Self {
        match name {
            NOT_AUTHORIZED | NOT_AUTHORIZED_CAN_OBTAIN => Self::NotAuthorized,
            NOT_AUTHORIZED_DISMISSED | CANCELLED => Self::Cancelled,
            DEVICE_BUSY => Self::Busy,
            ALREADY_MOUNTED => Self::AlreadyMounted,
            NOT_MOUNTED => Self::NotMounted,
            _ => Self::Failed(message.unwrap_or(name).to_owned()),
        }
    }
}

impl From<zbus::Error> for DeviceError {
    fn from(error: zbus::Error) -> Self {
        match &error {
            zbus::Error::MethodError(name, message, _) => {
                Self::from_dbus(name.as_str(), message.as_deref())
            }
            _ => Self::Bus(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_udisks_errors_by_name() {
        assert!(matches!(
            DeviceError::from_dbus(NOT_AUTHORIZED_DISMISSED, None),
            DeviceError::Cancelled
        ));
        assert!(matches!(
            DeviceError::from_dbus(NOT_AUTHORIZED_CAN_OBTAIN, Some("x")),
            DeviceError::NotAuthorized
        ));
        assert!(matches!(
            DeviceError::from_dbus(DEVICE_BUSY, Some("target is busy")),
            DeviceError::Busy
        ));
        assert!(matches!(
            DeviceError::from_dbus(NOT_MOUNTED, None),
            DeviceError::NotMounted
        ));
        let failed =
            DeviceError::from_dbus("org.freedesktop.UDisks2.Error.Failed", Some("bad key"));
        assert_eq!(failed.to_string(), "bad key");
        let unnamed = DeviceError::from_dbus("org.example.Odd", None);
        assert_eq!(unnamed.to_string(), "org.example.Odd");
    }
}
