use std::{path::Path, sync::Arc};

use crate::{model::folder::Folder, util::hash::FastIndexMap};

pub const HISTORY_CAPACITY: usize = 100;

/// Recently visited folders, so going back shows the old listing instantly
/// while it revalidates. Least recently stored folders are evicted first.
#[derive(Debug, Default)]
pub struct History {
    folders: FastIndexMap<Arc<Path>, Folder>,
}

impl History {
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    pub fn contains(&self, dir: &Path) -> bool {
        self.folders.contains_key(dir)
    }

    /// Takes a folder out to become the current one.
    pub fn take(&mut self, dir: &Path) -> Option<Folder> {
        self.folders.shift_remove(dir)
    }

    pub fn get_mut(&mut self, dir: &Path) -> Option<&mut Folder> {
        self.folders.get_mut(dir)
    }

    /// Stores a folder being left as the most recent one.
    pub fn store(&mut self, folder: Folder) {
        self.folders.shift_remove(folder.dir().as_ref());
        self.folders.insert(folder.dir().clone(), folder);
        while self.folders.len() > HISTORY_CAPACITY {
            self.folders.shift_remove_index(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::view_options::ViewOptions;

    fn folder(index: usize) -> Folder {
        Folder::new(
            Path::new(&format!("/d{index}")).into(),
            ViewOptions::default(),
        )
    }

    #[test]
    fn evicts_the_least_recently_stored_folder() {
        let mut history = History::default();
        for index in 0..HISTORY_CAPACITY {
            history.store(folder(index));
        }
        history.store(folder(0));
        history.store(folder(HISTORY_CAPACITY));
        assert_eq!(history.len(), HISTORY_CAPACITY);
        assert!(history.contains(Path::new("/d0")));
        assert!(!history.contains(Path::new("/d1")));
        assert!(history.take(Path::new("/d2")).is_some());
        assert!(!history.contains(Path::new("/d2")));
    }
}
