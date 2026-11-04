use std::collections::HashMap;

use zbus::zvariant::OwnedObjectPath;

use crate::udisks::{objects::ObjectTree, volume::Volume};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// There when the monitor started.
    Present,
    /// Plugged in, inserted or set up while the monitor ran.
    Plugged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    Added { volume: Volume, arrival: Arrival },
    Changed(Volume),
    Removed(OwnedObjectPath),
}

/// The visible volumes, diffed against each new state of the object tree.
#[derive(Debug, Default)]
pub struct VolumeSet {
    volumes: HashMap<OwnedObjectPath, Volume>,
}

impl VolumeSet {
    /// Adopts the volumes of `tree` and reports how they differ from before.
    pub fn reconcile(&mut self, tree: &ObjectTree, uid: u32, arrival: Arrival) -> Vec<DeviceEvent> {
        let current: HashMap<_, _> = tree
            .objects()
            .filter_map(|(path, _)| Some((path.clone(), Volume::from_tree(tree, path, uid)?)))
            .collect();
        let mut events: Vec<DeviceEvent> = self
            .volumes
            .keys()
            .filter(|path| !current.contains_key(*path))
            .map(|path| DeviceEvent::Removed(path.clone()))
            .collect();
        for (path, volume) in &current {
            match self.volumes.get(path) {
                None => events.push(DeviceEvent::Added {
                    volume: volume.clone(),
                    arrival,
                }),
                Some(known) if known != volume => events.push(DeviceEvent::Changed(volume.clone())),
                Some(_) => {}
            }
        }
        self.volumes = current;
        events
    }

    pub fn volumes(&self) -> impl Iterator<Item = &Volume> {
        self.volumes.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udisks::{fixtures::*, proxies::FILESYSTEM};

    #[test]
    fn reports_present_then_plugged_changed_and_removed_volumes() {
        let mut tree = sample_tree();
        let mut set = VolumeSet::default();
        let first = set.reconcile(&tree, USER, Arrival::Present);
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|event| matches!(
            event,
            DeviceEvent::Added {
                arrival: Arrival::Present,
                ..
            }
        )));
        assert!(set.reconcile(&tree, USER, Arrival::Plugged).is_empty());

        let sdb1 = block_path("sdb1");
        tree.update_properties(&sdb1, FILESYSTEM, filesystem(&[]), &[]);
        let changed = set.reconcile(&tree, USER, Arrival::Plugged);
        assert!(matches!(&changed[..], [DeviceEvent::Changed(volume)] if !volume.is_mounted()));

        tree.remove_interfaces(&sdb1, &[BLOCK.to_owned(), FILESYSTEM.to_owned()]);
        add_loop(&mut tree, "loop0", USER);
        let mut events = set.reconcile(&tree, USER, Arrival::Plugged);
        events.sort_by_key(|event| matches!(event, DeviceEvent::Added { .. }));
        assert!(matches!(&events[0], DeviceEvent::Removed(path) if *path == sdb1));
        assert!(matches!(
            &events[1],
            DeviceEvent::Added { volume, arrival: Arrival::Plugged } if volume.object_path == block_path("loop0")
        ));
        assert_eq!(set.volumes().count(), 2);
    }

    use crate::udisks::proxies::BLOCK;
}
