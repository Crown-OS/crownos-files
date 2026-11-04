use std::{
    fs::{File, FileTimes, Metadata},
    io,
    path::Path,
};

/// Gives `target` the permissions and access/modification times of `source`.
pub(crate) fn copy_attributes(source: &Metadata, target: &File) -> io::Result<()> {
    target.set_permissions(source.permissions())?;
    target.set_times(
        FileTimes::new()
            .set_accessed(source.accessed()?)
            .set_modified(source.modified()?),
    )
}

pub(crate) fn copy_dir_attributes(source: &Metadata, target: &Path) -> io::Result<()> {
    copy_attributes(source, &File::open(target)?)
}
