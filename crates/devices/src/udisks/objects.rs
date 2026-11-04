use std::collections::HashMap;

use zbus::{Message, fdo::ManagedObjects, zvariant::OwnedObjectPath};

use crate::util::dbus_props::Properties;

const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// The interfaces of one UDisks object, by name.
pub type Interfaces = HashMap<String, Properties>;

/// A mirror of the UDisks object tree, kept current from its signals.
#[derive(Debug, Default)]
pub struct ObjectTree {
    objects: HashMap<OwnedObjectPath, Interfaces>,
}

impl ObjectTree {
    pub fn from_managed(managed: ManagedObjects) -> Self {
        let mut tree = Self::default();
        for (path, interfaces) in managed {
            tree.add_interfaces(
                path,
                interfaces
                    .into_iter()
                    .map(|(name, properties)| (name.to_string(), properties))
                    .collect(),
            );
        }
        tree
    }

    pub fn add_interfaces(&mut self, path: OwnedObjectPath, interfaces: Interfaces) {
        self.objects.entry(path).or_default().extend(interfaces);
    }

    pub fn remove_interfaces(&mut self, path: &OwnedObjectPath, names: &[String]) {
        let Some(interfaces) = self.objects.get_mut(path) else {
            return;
        };
        names.iter().for_each(|name| {
            interfaces.remove(name);
        });
        if interfaces.is_empty() {
            self.objects.remove(path);
        }
    }

    pub fn update_properties(
        &mut self,
        path: &OwnedObjectPath,
        interface: &str,
        changed: Properties,
        invalidated: &[String],
    ) {
        let Some(properties) = self
            .objects
            .get_mut(path)
            .and_then(|interfaces| interfaces.get_mut(interface))
        else {
            return;
        };
        properties.extend(changed);
        invalidated.iter().for_each(|name| {
            properties.remove(name);
        });
    }

    pub fn get(&self, path: &OwnedObjectPath) -> Option<&Interfaces> {
        self.objects.get(path)
    }

    pub fn interface(&self, path: &OwnedObjectPath, name: &str) -> Option<&Properties> {
        self.get(path)?.get(name)
    }

    pub fn objects(&self) -> impl Iterator<Item = (&OwnedObjectPath, &Interfaces)> {
        self.objects.iter()
    }

    /// Applies an `InterfacesAdded`, `InterfacesRemoved` or
    /// `PropertiesChanged` signal; `false` when it was none of them.
    pub fn apply(&mut self, signal: &Message) -> bool {
        let header = signal.header();
        let (Some(interface), Some(member)) = (header.interface(), header.member()) else {
            return false;
        };
        let body = signal.body();
        match (interface.as_str(), member.as_str()) {
            (OBJECT_MANAGER, "InterfacesAdded") => body
                .deserialize::<(OwnedObjectPath, Interfaces)>()
                .map(|(path, interfaces)| self.add_interfaces(path, interfaces))
                .is_ok(),
            (OBJECT_MANAGER, "InterfacesRemoved") => body
                .deserialize::<(OwnedObjectPath, Vec<String>)>()
                .map(|(path, names)| self.remove_interfaces(&path, &names))
                .is_ok(),
            (PROPERTIES, "PropertiesChanged") => {
                let Some(path) = header.path() else {
                    return false;
                };
                let path = OwnedObjectPath::from(path.to_owned());
                body.deserialize::<(String, Properties, Vec<String>)>()
                    .map(|(interface, changed, invalidated)| {
                        self.update_properties(&path, &interface, changed, &invalidated)
                    })
                    .is_ok()
            }
            _ => false,
        }
    }
}
