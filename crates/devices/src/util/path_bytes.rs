//! Serde for paths and names as raw bytes: Unix paths need not be UTF-8.

use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
};

use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<T: AsRef<OsStr>, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(value.as_ref().as_bytes())
}

pub fn deserialize<'de, T: From<OsString>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Vec::<u8>::deserialize(deserializer).map(|bytes| OsString::from_vec(bytes).into())
}

pub mod list {
    use super::*;

    pub fn serialize<T: AsRef<OsStr>, S: Serializer>(
        values: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(values.iter().map(|value| value.as_ref().as_bytes()))
    }

    pub fn deserialize<'de, T: From<OsString>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<T>, D::Error> {
        Vec::<Vec<u8>>::deserialize(deserializer).map(|list| {
            list.into_iter()
                .map(|bytes| OsString::from_vec(bytes).into())
                .collect()
        })
    }
}
