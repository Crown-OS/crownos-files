use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum MimeError {
    #[error("cannot read {path}: {source}", path = path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
