use std::{collections::HashMap, ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

use zbus::zvariant::{Array, ObjectPath, OwnedObjectPath, OwnedValue, Value};

/// The properties of one D-Bus interface, as `GetManagedObjects` and
/// `PropertiesChanged` deliver them.
pub type Properties = HashMap<String, OwnedValue>;

const NO_OBJECT: &str = "/";

pub fn get<'a, T: TryFrom<&'a OwnedValue>>(properties: &'a Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| T::try_from(value).ok())
}

pub fn flag(properties: &Properties, key: &str) -> bool {
    get(properties, key).unwrap_or(false)
}

/// A non-empty string property.
pub fn text<'a>(properties: &'a Properties, key: &str) -> Option<&'a str> {
    get::<&str>(properties, key).filter(|text| !text.is_empty())
}

/// An object path property; UDisks spells "none" as `/`.
pub fn object(properties: &Properties, key: &str) -> Option<OwnedObjectPath> {
    get::<&ObjectPath<'_>>(properties, key)
        .filter(|path| path.as_str() != NO_OBJECT)
        .map(|path| OwnedObjectPath::from(path.to_owned()))
}

/// A NUL-terminated byte string (`ay`) property, such as a device file.
pub fn byte_path(properties: &Properties, key: &str) -> Option<PathBuf> {
    get::<&Array<'_>>(properties, key)
        .map(array_bytes)
        .filter(|path| !path.as_os_str().is_empty())
}

/// An array of byte strings (`aay`), such as mount points.
pub fn byte_paths(properties: &Properties, key: &str) -> Vec<PathBuf> {
    get::<&Array<'_>>(properties, key)
        .map(|list| {
            list.iter()
                .filter_map(|item| match item {
                    Value::Array(bytes) => Some(array_bytes(bytes)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn array_bytes(array: &Array<'_>) -> PathBuf {
    let mut bytes: Vec<u8> = array
        .iter()
        .filter_map(|byte| u8::try_from(byte).ok())
        .collect();
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    PathBuf::from(OsString::from_vec(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(entries: Vec<(&str, Value<'static>)>) -> Properties {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), OwnedValue::try_from(value).unwrap()))
            .collect()
    }

    #[test]
    fn reads_byte_strings_paths_and_flags() {
        let props = properties(vec![
            ("Device", Value::from(b"/dev/sdb1\0".to_vec())),
            (
                "MountPoints",
                Value::from(vec![b"/run/media/u/A\0".to_vec(), b"/mnt\0".to_vec()]),
            ),
            (
                "Drive",
                Value::from(ObjectPath::from_static_str_unchecked("/")),
            ),
            ("HintAuto", Value::from(true)),
            ("IdLabel", Value::from("")),
        ]);
        assert_eq!(
            byte_path(&props, "Device"),
            Some(PathBuf::from("/dev/sdb1"))
        );
        assert_eq!(
            byte_paths(&props, "MountPoints"),
            [PathBuf::from("/run/media/u/A"), PathBuf::from("/mnt")]
        );
        assert_eq!(object(&props, "Drive"), None);
        assert!(flag(&props, "HintAuto") && !flag(&props, "HintIgnore"));
        assert_eq!(text(&props, "IdLabel"), None);
    }
}
