use crate::model::timestamp::Timestamp;

/// What a directory's own inode looked like when it was listed; a listing is
/// still current while the directory's stamp is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirStamp {
    pub modified: Timestamp,
    pub changed: Timestamp,
    pub len: u64,
    pub inode: u64,
    pub device: u64,
}
