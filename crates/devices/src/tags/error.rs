use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("{} is on a filesystem without user attributes", .0.display())]
    Unsupported(PathBuf),
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
