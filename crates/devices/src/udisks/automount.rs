use std::path::PathBuf;

use crate::udisks::{
    actions::Udisks,
    error::DeviceError,
    volume::Volume,
    volume_set::{Arrival, DeviceEvent},
};

/// The `files.automount` setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AutomountPolicy {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mounted {
    pub volume: Volume,
    pub mount_point: PathBuf,
}

/// The volume to mount on its own: a removable, unmounted, unencrypted
/// filesystem just plugged in, that UDisks says may be mounted automatically.
pub fn should_automount(event: &DeviceEvent, policy: AutomountPolicy) -> Option<&Volume> {
    let DeviceEvent::Added {
        volume,
        arrival: Arrival::Plugged,
    } = event
    else {
        return None;
    };
    let wanted = policy == AutomountPolicy::Enabled
        && volume.hints.auto
        && volume.is_removable()
        && !volume.is_encrypted
        && !volume.is_mounted();
    wanted.then_some(volume)
}

/// Mounts the volume `event` brought when [`should_automount`] says so.
/// Polkit may prompt the user; dismissing it reports `Cancelled`.
pub async fn automount(
    udisks: &Udisks,
    event: &DeviceEvent,
    policy: AutomountPolicy,
) -> Option<Result<Mounted, DeviceError>> {
    let volume = should_automount(event, policy)?;
    Some(udisks.mount(volume).await.map(|mount_point| Mounted {
        volume: volume.clone(),
        mount_point,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udisks::fixtures::*;

    fn plugged(name: &str) -> DeviceEvent {
        let mut tree = sample_tree();
        tree.update_properties(
            &block_path("sdb1"),
            crate::udisks::proxies::FILESYSTEM,
            filesystem(&[]),
            &[],
        );
        DeviceEvent::Added {
            volume: Volume::from_tree(&tree, &block_path(name), USER).unwrap(),
            arrival: Arrival::Plugged,
        }
    }

    #[test]
    fn mounts_only_new_unmounted_removable_filesystems_when_enabled() {
        let stick = plugged("sdb1");
        assert!(should_automount(&stick, AutomountPolicy::Enabled).is_some());
        assert!(should_automount(&stick, AutomountPolicy::Disabled).is_none());
        assert!(should_automount(&plugged("sdb2"), AutomountPolicy::Enabled).is_none());
        let DeviceEvent::Added { volume, .. } = stick else {
            unreachable!()
        };
        let present = DeviceEvent::Added {
            volume: volume.clone(),
            arrival: Arrival::Present,
        };
        assert!(should_automount(&present, AutomountPolicy::Enabled).is_none());
        let mut internal = volume.clone();
        internal.drive.as_mut().unwrap().removable = false;
        let internal = DeviceEvent::Added {
            volume: internal,
            arrival: Arrival::Plugged,
        };
        assert!(should_automount(&internal, AutomountPolicy::Enabled).is_none());
        let mut mounted = volume;
        mounted.mount_points.push("/mnt".into());
        let changed = DeviceEvent::Changed(mounted);
        assert!(should_automount(&changed, AutomountPolicy::Enabled).is_none());
    }
}
