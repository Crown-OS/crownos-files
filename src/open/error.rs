use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is not a valid desktop entry: {reason}", path.display())]
    InvalidEntry { path: PathBuf, reason: &'static str },
    #[error("{} is hidden", .0.display())]
    Hidden(PathBuf),
    #[error("{0} is not installed")]
    NotInstalled(String),
    #[error("the MIME type is unknown")]
    UnknownMime,
    #[error("{0} has nothing to run")]
    NothingToRun(String),
    #[error("cannot start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("{} is not valid XBEL: {source}", path.display())]
    Xbel {
        path: PathBuf,
        #[source]
        source: quick_xml::Error,
    },
    #[error("the XDG base directories are unknown: HOME is not set")]
    NoHome,
}

impl OpenError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
