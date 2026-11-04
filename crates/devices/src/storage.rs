use std::{io, path::Path, sync::Arc};

use compact_str::CompactString;
use rustix::fs::statvfs;
use tokio_util::sync::CancellationToken;

use crate::{
    fs::dir_size::measure_dirs,
    model::{
        files_op::FilesOp,
        special_dir::{SpecialDir, SpecialDirs},
    },
};

/// The user folders the storage card breaks usage down by.
pub const MEASURED_DIRS: [SpecialDir; 6] = [
    SpecialDir::Documents,
    SpecialDir::Downloads,
    SpecialDir::Pictures,
    SpecialDir::Music,
    SpecialDir::Videos,
    SpecialDir::Desktop,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceUsage {
    pub total: u64,
    pub free: u64,
    /// Free to unprivileged users; less than `free` by the reserved blocks.
    pub available: u64,
}

impl SpaceUsage {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    /// How much of the filesystem is used, from 0 to 1.
    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used() as f64 / self.total as f64) as f32
    }
}

/// Size and free space of the filesystem `path` is on.
pub fn space_usage(path: &Path) -> io::Result<SpaceUsage> {
    let stats = statvfs(path)?;
    let blocks = |count: u64| count.saturating_mul(stats.f_frsize);
    Ok(SpaceUsage {
        total: blocks(stats.f_blocks),
        free: blocks(stats.f_bfree),
        available: blocks(stats.f_bavail),
    })
}

/// Bytes in each of [`MEASURED_DIRS`], in that order; zero until measured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageBreakdown {
    pub sizes: [u64; MEASURED_DIRS.len()],
}

impl StorageBreakdown {
    pub fn get(&self, dir: SpecialDir) -> u64 {
        MEASURED_DIRS
            .iter()
            .position(|&measured| measured == dir)
            .map_or(0, |index| self.sizes[index])
    }

    pub fn total(&self) -> u64 {
        self.sizes.iter().sum()
    }
}

/// Measures the user folders on the calling (blocking) thread, handing the
/// running breakdown to `emit` at most every few hundred milliseconds per
/// folder and once more when each is done.
pub fn measure_breakdown(
    dirs: &SpecialDirs,
    cancel: &CancellationToken,
    mut emit: impl FnMut(&StorageBreakdown),
) {
    let mut breakdown = StorageBreakdown::default();
    for (index, special) in MEASURED_DIRS.into_iter().enumerate() {
        let Some(path) = dirs.path(special) else {
            continue;
        };
        let (Some(parent), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
        else {
            continue;
        };
        let parent: Arc<Path> = Arc::from(parent);
        measure_dirs(&parent, &[CompactString::from(name)], cancel, |op| {
            if let FilesOp::Size { sizes, .. } = op {
                sizes
                    .into_iter()
                    .for_each(|(_, bytes)| breakdown.sizes[index] = bytes);
                emit(&breakdown);
            }
        });
        if cancel.is_cancelled() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statvfs_of_root_is_consistent() {
        let usage = space_usage(Path::new("/")).unwrap();
        assert!(usage.total > 0);
        assert!(usage.free <= usage.total && usage.available <= usage.free);
        assert_eq!(usage.used(), usage.total - usage.free);
        assert!((0.0..=1.0).contains(&usage.used_fraction()));
        let empty = SpaceUsage {
            total: 0,
            free: 0,
            available: 0,
        };
        assert_eq!(empty.used_fraction(), 0.0);
    }

    #[test]
    fn breakdown_measures_user_folders() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("Documents/deep")).unwrap();
        std::fs::write(home.path().join("Documents/deep/a"), [0; 300]).unwrap();
        std::fs::create_dir(home.path().join("Music")).unwrap();
        std::fs::write(home.path().join("Music/b"), [0; 50]).unwrap();
        let dirs = SpecialDirs::from_user_dirs("", home.path());
        let mut last = StorageBreakdown::default();
        measure_breakdown(&dirs, &CancellationToken::new(), |breakdown| {
            last = breakdown.clone()
        });
        assert_eq!(last.get(SpecialDir::Documents), 300);
        assert_eq!(last.get(SpecialDir::Music), 50);
        assert_eq!(last.get(SpecialDir::Videos), 0);
        assert_eq!(last.total(), 350);
    }
}
