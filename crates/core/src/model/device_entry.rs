use std::{path::Path, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Drive,
    Phone,
}

/// One row of the sidebar's Devices section: a drive or a paired phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    pub name: String,
    pub kind: DeviceKind,
    /// Where a drive is mounted; `None` while it is not.
    pub mount: Option<Arc<Path>>,
    /// Whether a phone is reachable right now.
    pub online: bool,
}

impl DeviceEntry {
    pub fn can_receive_files(&self) -> bool {
        self.kind == DeviceKind::Phone && self.online
    }
}
