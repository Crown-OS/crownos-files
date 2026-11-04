use crate::mime::MimeTable;

/// An interned MIME type: entries compare and store it as a 2-byte integer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MimeId(pub u16);

impl MimeId {
    pub const UNKNOWN: Self = Self(0);

    pub fn intern(name: &str) -> Self {
        MimeTable::intern(name)
    }

    pub fn name(self) -> Option<&'static str> {
        MimeTable::name(self)
    }
}
