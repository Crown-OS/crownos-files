/// An interned MIME type: entries compare and store it as a 2-byte integer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MimeId(pub u16);

impl MimeId {
    pub const UNKNOWN: Self = Self(0);
}
