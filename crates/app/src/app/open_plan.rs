//! What opening the selection does, decided from the folder alone: a folder
//! navigates, a file crownos-preview shows is previewed with its previewable
//! neighbours, anything else goes to its application.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    model::{folder::Folder, kind::FileKind, mime_id::MimeId},
    open::is_previewable,
};

/// A file for the opener: its path and what the listing knows of its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTarget {
    pub path: PathBuf,
    pub kind: FileKind,
    pub mime: MimeId,
}

/// The previewable files of a folder in view order, and which one to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewList {
    pub paths: Vec<String>,
    /// The folder position of each of `paths`.
    pub positions: Vec<usize>,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPlan {
    Nothing,
    Navigate(Arc<Path>),
    Preview(PreviewList),
    Launch(Vec<OpenTarget>),
}

/// Opening what is selected: a lone folder navigates; of the files, the one
/// under the cursor (or the first) decides between quick look and apps.
pub fn plan_open(folder: &Folder) -> OpenPlan {
    let dir = folder.dir();
    let selected: Vec<usize> = (0..folder.len())
        .filter(|&position| folder.is_selected(position))
        .collect();
    let files: Vec<usize> = selected
        .iter()
        .copied()
        .filter(|&position| folder.get(position).is_some_and(|entry| !entry.is_dir()))
        .collect();
    let Some(&first) = files.first() else {
        return selected
            .first()
            .and_then(|&position| folder.get(position))
            .map_or(OpenPlan::Nothing, |entry| {
                OpenPlan::Navigate(entry.path_in(dir).into())
            });
    };
    let cursor = folder.cursor();
    let primary = if files.contains(&cursor) {
        cursor
    } else {
        first
    };
    if let Some(list) = preview_list_at(folder, primary) {
        return OpenPlan::Preview(list);
    }
    OpenPlan::Launch(
        files
            .into_iter()
            .filter_map(|position| folder.get(position))
            .map(|entry| OpenTarget {
                path: entry.path_in(dir),
                kind: entry.kind,
                mime: entry.mime,
            })
            .collect(),
    )
}

/// The folder's previewable files with the one at `position` shown, if it is one.
pub fn preview_list_at(folder: &Folder, position: usize) -> Option<PreviewList> {
    let dir = folder.dir();
    let mut paths = Vec::new();
    let mut positions = Vec::new();
    let mut index = None;
    for (at, entry) in folder.iter().enumerate() {
        if entry.is_dir() {
            continue;
        }
        let path = entry.path_in(dir);
        if !is_previewable(&path) {
            continue;
        }
        let Ok(path) = path.into_os_string().into_string() else {
            continue;
        };
        if at == position {
            index = u32::try_from(paths.len()).ok();
        }
        paths.push(path);
        positions.push(at);
    }
    Some(PreviewList {
        paths,
        positions,
        index: index?,
    })
}
