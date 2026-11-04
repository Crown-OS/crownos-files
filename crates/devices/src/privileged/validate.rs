use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    jobs::validate_name,
    privileged::{error::ValidationError, request::PrivilegedRequest},
};

/// Trees the helper never reads or writes in.
const FORBIDDEN_TREES: [&str; 4] = ["/proc", "/sys", "/dev", "/boot/efi"];
const PERMISSION_BITS: u32 = 0o7777;

/// A request whose paths are absolute, canonical and outside protected
/// locations, as only [`validate`] makes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidRequest(PrivilegedRequest);

impl ValidRequest {
    pub fn request(&self) -> &PrivilegedRequest {
        &self.0
    }

    pub fn into_inner(self) -> PrivilegedRequest {
        self.0
    }
}

/// Resolves every path of `request` and refuses `..`, relative paths, `/`
/// and the top-level folders, and anything in [`FORBIDDEN_TREES`], also when
/// a symlink leads there.
pub fn validate(request: PrivilegedRequest) -> Result<ValidRequest, ValidationError> {
    let valid = match request {
        PrivilegedRequest::Copy {
            sources,
            destination,
            on_conflict,
        } => PrivilegedRequest::Copy {
            sources: entries(&sources)?,
            destination: folder(&destination)?,
            on_conflict,
        },
        PrivilegedRequest::Move {
            sources,
            destination,
            on_conflict,
        } => PrivilegedRequest::Move {
            sources: entries(&sources)?,
            destination: folder(&destination)?,
            on_conflict,
        },
        PrivilegedRequest::Delete { paths } => PrivilegedRequest::Delete {
            paths: entries(&paths)?,
        },
        PrivilegedRequest::CreateFolder { parent, name } => {
            validate_name(&name)?;
            PrivilegedRequest::CreateFolder {
                parent: folder(&parent)?,
                name,
            }
        }
        PrivilegedRequest::Rename { path, new_name } => {
            validate_name(&new_name)?;
            PrivilegedRequest::Rename {
                path: entry(&path)?,
                new_name,
            }
        }
        PrivilegedRequest::Chmod { paths, mode } => {
            non_empty(&paths)?;
            PrivilegedRequest::Chmod {
                paths: paths
                    .iter()
                    .map(|path| target(path))
                    .collect::<Result<_, _>>()?,
                mode: mode & PERMISSION_BITS,
            }
        }
    };
    Ok(ValidRequest(valid))
}

fn entries(paths: &[PathBuf]) -> Result<Vec<PathBuf>, ValidationError> {
    non_empty(paths)?;
    paths.iter().map(|path| entry(path)).collect()
}

fn non_empty(paths: &[PathBuf]) -> Result<(), ValidationError> {
    if paths.is_empty() {
        return Err(ValidationError::Empty);
    }
    Ok(())
}

/// The entry itself, its folder resolved: a symlink stays the link.
fn entry(path: &Path) -> Result<PathBuf, ValidationError> {
    let (parent, name) = split(path)?;
    let entry = canonical(parent)?.join(name);
    fs::symlink_metadata(&entry).map_err(unreachable(&entry))?;
    ensure_changeable(&entry)?;
    Ok(entry)
}

/// What the path leads to, symlinks followed.
fn target(path: &Path) -> Result<PathBuf, ValidationError> {
    ensure_lexically_safe(path)?;
    let target = canonical(path)?;
    ensure_changeable(&target)?;
    Ok(target)
}

/// A folder to create things in.
fn folder(path: &Path) -> Result<PathBuf, ValidationError> {
    ensure_lexically_safe(path)?;
    let folder = canonical(path)?;
    if !folder.is_dir() {
        return Err(ValidationError::NotADirectory(folder));
    }
    ensure_outside_forbidden(&folder)?;
    Ok(folder)
}

fn split(path: &Path) -> Result<(&Path, &OsStr), ValidationError> {
    ensure_lexically_safe(path)?;
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => Ok((parent, name)),
        _ => Err(ValidationError::Forbidden(path.to_path_buf())),
    }
}

fn ensure_lexically_safe(path: &Path) -> Result<(), ValidationError> {
    if !path.is_absolute() {
        return Err(ValidationError::NotAbsolute(path.to_path_buf()));
    }
    if path.components().any(|part| part == Component::ParentDir) {
        return Err(ValidationError::ParentComponent(path.to_path_buf()));
    }
    Ok(())
}

/// Refuses `/`, the folders right under it, and the forbidden trees.
fn ensure_changeable(path: &Path) -> Result<(), ValidationError> {
    if path.parent().is_none_or(|parent| parent == Path::new("/")) {
        return Err(ValidationError::Forbidden(path.to_path_buf()));
    }
    ensure_outside_forbidden(path)
}

fn ensure_outside_forbidden(path: &Path) -> Result<(), ValidationError> {
    if FORBIDDEN_TREES.iter().any(|tree| path.starts_with(tree)) {
        return Err(ValidationError::Forbidden(path.to_path_buf()));
    }
    Ok(())
}

fn canonical(path: &Path) -> Result<PathBuf, ValidationError> {
    fs::canonicalize(path).map_err(unreachable(path))
}

fn unreachable(path: &Path) -> impl FnOnce(std::io::Error) -> ValidationError + '_ {
    move |source| ValidationError::Unreachable {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;
    use crownos_files_core::jobs::ConflictPolicy;

    fn delete(path: impl Into<PathBuf>) -> Result<ValidRequest, ValidationError> {
        validate(PrivilegedRequest::Delete {
            paths: vec![path.into()],
        })
    }

    #[test]
    fn refuses_relative_parent_and_protected_paths() {
        assert!(matches!(
            delete("etc/x"),
            Err(ValidationError::NotAbsolute(_))
        ));
        assert!(matches!(
            delete("/tmp/../etc"),
            Err(ValidationError::ParentComponent(_))
        ));
        assert!(matches!(delete("/"), Err(ValidationError::Forbidden(_))));
        assert!(matches!(delete("/usr"), Err(ValidationError::Forbidden(_))));
        assert!(matches!(
            delete("/proc/self"),
            Err(ValidationError::Forbidden(_))
        ));
        assert!(matches!(
            delete("/sys/kernel"),
            Err(ValidationError::Forbidden(_))
        ));
        assert!(matches!(
            delete("/dev/null"),
            Err(ValidationError::Forbidden(_))
        ));
        assert!(matches!(
            validate(PrivilegedRequest::Delete { paths: vec![] }),
            Err(ValidationError::Empty)
        ));
        let into_dev = validate(PrivilegedRequest::Copy {
            sources: vec!["/etc/hostname".into()],
            destination: "/dev".into(),
            on_conflict: ConflictPolicy::Skip,
        });
        assert!(matches!(into_dev, Err(ValidationError::Forbidden(_))));
    }

    #[test]
    fn symlinks_cannot_escape_into_protected_places() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        symlink("/proc", root.join("proc_link")).unwrap();
        symlink("/", root.join("root_link")).unwrap();
        symlink("/dev", root.join("dev_link")).unwrap();
        assert!(matches!(
            delete(root.join("proc_link/self")),
            Err(ValidationError::Forbidden(path)) if path == Path::new("/proc/self")
        ));
        let chmod_root = validate(PrivilegedRequest::Chmod {
            paths: vec![root.join("root_link")],
            mode: 0o777,
        });
        assert!(matches!(chmod_root, Err(ValidationError::Forbidden(_))));
        let into_dev = validate(PrivilegedRequest::CreateFolder {
            parent: root.join("dev_link"),
            name: "x".into(),
        });
        assert!(matches!(into_dev, Err(ValidationError::Forbidden(_))));
        let link_itself = delete(root.join("root_link")).unwrap();
        assert_eq!(
            link_itself.request(),
            &PrivilegedRequest::Delete {
                paths: vec![root.join("root_link")]
            }
        );
    }

    #[test]
    fn resolves_folders_and_checks_names_and_modes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("real")).unwrap();
        std::fs::write(root.join("real/file"), b"x").unwrap();
        symlink(root.join("real"), root.join("alias")).unwrap();
        let moved = validate(PrivilegedRequest::Move {
            sources: vec![root.join("alias/file")],
            destination: root.join("alias"),
            on_conflict: ConflictPolicy::Replace,
        })
        .unwrap();
        assert_eq!(
            moved.into_inner(),
            PrivilegedRequest::Move {
                sources: vec![root.join("real/file")],
                destination: root.join("real"),
                on_conflict: ConflictPolicy::Replace,
            }
        );
        let bad_name = validate(PrivilegedRequest::Rename {
            path: root.join("real/file"),
            new_name: "a/b".into(),
        });
        assert!(matches!(bad_name, Err(ValidationError::InvalidName(_))));
        let not_dir = validate(PrivilegedRequest::CreateFolder {
            parent: root.join("real/file"),
            name: "x".into(),
        });
        assert!(matches!(not_dir, Err(ValidationError::NotADirectory(_))));
        assert!(matches!(
            delete(root.join("missing")),
            Err(ValidationError::Unreachable { .. })
        ));
        let chmod = validate(PrivilegedRequest::Chmod {
            paths: vec![root.join("alias")],
            mode: 0o170_755,
        })
        .unwrap();
        assert!(matches!(
            chmod.request(),
            PrivilegedRequest::Chmod { mode: 0o755, .. }
        ));
    }
}
