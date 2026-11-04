use std::{io, path::Path};

/// How full the filesystem holding a directory is, from `statvfs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub available: u64,
}

impl DiskUsage {
    pub fn of(path: &Path) -> io::Result<Self> {
        let stats = rustix::fs::statvfs(path)?;
        let block = stats.f_frsize.max(1);
        Ok(Self {
            total: stats.f_blocks.saturating_mul(block),
            available: stats.f_bavail.saturating_mul(block),
        })
    }

    pub fn used(self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    pub fn used_fraction(self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used() as f64 / self.total as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_filesystem_reports_a_consistent_usage() {
        let usage = DiskUsage::of(Path::new("/")).unwrap();
        assert!(usage.available <= usage.total);
        assert!((0.0..=1.0).contains(&usage.used_fraction()));
    }

    #[test]
    fn an_empty_filesystem_is_not_divided_by() {
        let usage = DiskUsage {
            total: 0,
            available: 0,
        };
        assert_eq!(usage.used_fraction(), 0.0);
    }
}
