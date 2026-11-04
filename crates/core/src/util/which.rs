use std::{
    env,
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const EXECUTABLE_BITS: u32 = 0o111;

/// Where `program` runs from: itself when it names a path, otherwise the
/// first executable of that name on `$PATH`.
pub fn find_program(program: &OsStr) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 {
        return is_executable(candidate).then(|| candidate.to_path_buf());
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| {
        metadata.is_file() && metadata.permissions().mode() & EXECUTABLE_BITS != 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs_on_the_path_and_by_absolute_path() {
        assert!(find_program(OsStr::new("sh")).is_some());
        assert!(find_program(OsStr::new("/bin/sh")).is_some());
        assert!(find_program(OsStr::new("no-such-program-anywhere")).is_none());
        assert!(find_program(OsStr::new("/etc/hostname-missing")).is_none());
    }
}
