//! The Properties dialog over the selected entry, or the folder itself when
//! nothing is selected: what the listing already knows at once, the rest
//! (times, owner, permissions, a folder's size) read off the UI thread.

use std::{
    fs::Metadata,
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use compact_str::CompactString;
use crownos_files_devices::storage::{SpaceUsage, space_usage};
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio_util::sync::CancellationToken;

use crate::{
    app::{tasks::Task, virtual_folder::VirtualFolder},
    fs::dir_size::measure_dirs,
    mime::mime_of_name,
    model::{entry::Entry, files_op::FilesOp, mime_id::MimeId, timestamp::Timestamp},
    state::Store,
    util::{
        account_names::{group_name, user_name},
        breadcrumbs::describe_location,
        kind_label::kind_label,
    },
};

const FOLDER_MIME: &str = "inode/directory";
const UNKNOWN_MIME: &str = "application/octet-stream";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertySheet {
    pub path: PathBuf,
    pub name: String,
    pub kind: String,
    pub mime: &'static str,
    pub location: String,
    pub is_dir: bool,
    /// A file's length, or a folder's size once measured.
    pub size: Option<u64>,
    pub facts: Option<FileFacts>,
}

/// What a `stat` of the path tells beyond the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub created: Timestamp,
    pub modified: Timestamp,
    pub accessed: Timestamp,
    pub mode: u32,
    pub owner: String,
    pub group: String,
    pub len: u64,
    /// Size and free space, when a volume is mounted at the path.
    pub volume: Option<SpaceUsage>,
}

#[derive(Clone, Copy)]
pub struct Properties {
    pub open: Signal<bool>,
    pub sheet: Signal<Option<PropertySheet>>,
    measuring: Cell<Option<CancellationToken>>,
}

impl Properties {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            open: cx.signal(false),
            sheet: cx.signal(None),
            measuring: cx.store(None),
        }
    }
}

/// Opens the dialog on the first selected entry, or on the folder.
pub fn show_properties(cx: &mut Cx, store: Store) {
    let home = store.places.with(cx, |places| places.home.clone());
    let sheet = store.folder.with(cx, |folder| {
        let dir = folder.dir();
        match folder.selected_entries().next() {
            Some(entry) => Some(entry_sheet(entry, dir, home.as_deref())),
            None if VirtualFolder::of(dir).is_none() => Some(folder_sheet(dir, home.as_deref())),
            None => None,
        }
    });
    let Some(sheet) = sheet else {
        return;
    };
    let path = sheet.path.clone();
    if let Some(previous) = store.properties.measuring.update(cx, Option::take) {
        previous.cancel();
    }
    if sheet.is_dir {
        let cancel = CancellationToken::new();
        store
            .properties
            .measuring
            .update(cx, |slot| *slot = Some(cancel.clone()));
        store.run(
            cx,
            Task::MeasureFolder {
                path: path.clone(),
                cancel,
            },
        );
    }
    store.run(cx, Task::Inspect(path));
    store.properties.sheet.set(cx, Some(sheet));
    store.properties.open.set(cx, true);
}

/// Stops measuring once the dialog is dismissed.
pub fn follow_properties_dismissal(cx: &mut Cx, store: Store) {
    cx.effect(move |runtime| {
        if !store.properties.open.get(runtime)
            && let Some(cancel) = store.properties.measuring.update(runtime, Option::take)
        {
            cancel.cancel();
        }
    });
}

fn entry_sheet(entry: &Entry, dir: &Path, home: Option<&Path>) -> PropertySheet {
    let path = entry.path_in(dir);
    let is_dir = entry.is_dir();
    PropertySheet {
        name: path.file_name().map_or_else(
            || entry.name.to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        kind: kind_label(entry).into_owned(),
        mime: if is_dir {
            FOLDER_MIME
        } else {
            Some(entry.mime)
                .filter(|mime| *mime != MimeId::UNKNOWN)
                .unwrap_or_else(|| mime_of_name(&entry.name))
                .name()
                .unwrap_or(UNKNOWN_MIME)
        },
        location: path
            .parent()
            .map(|parent| describe_location(parent, home))
            .unwrap_or_default(),
        is_dir,
        size: (!is_dir).then_some(entry.stat.len),
        facts: None,
        path,
    }
}

fn folder_sheet(dir: &Path, home: Option<&Path>) -> PropertySheet {
    PropertySheet {
        path: dir.to_path_buf(),
        name: dir.file_name().map_or_else(
            || "Computer".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
        kind: "Folder".to_owned(),
        mime: FOLDER_MIME,
        location: dir
            .parent()
            .map(|parent| describe_location(parent, home))
            .unwrap_or_default(),
        is_dir: true,
        size: None,
        facts: None,
    }
}

/// Stats `path`, on a blocking thread.
pub fn inspect(path: &Path) -> io::Result<FileFacts> {
    let metadata = std::fs::metadata(path)?;
    let is_mount_point = path.parent().is_none_or(|parent| {
        std::fs::metadata(parent).is_ok_and(|parent| parent.dev() != metadata.dev())
    });
    Ok(FileFacts {
        created: system_stamp(metadata.created()),
        modified: stamp(metadata.mtime(), metadata.mtime_nsec()),
        accessed: stamp(metadata.atime(), metadata.atime_nsec()),
        mode: metadata.mode(),
        owner: user_name(metadata.uid()),
        group: group_name(metadata.gid()),
        len: file_len(&metadata),
        volume: is_mount_point.then(|| space_usage(path).ok()).flatten(),
    })
}

/// The bytes under the folder at `path`, on a blocking thread; `None` when
/// cancelled.
pub fn measure_folder(path: &Path, cancel: &CancellationToken) -> Option<u64> {
    let (parent, name) = (path.parent()?, path.file_name()?.to_str()?);
    let mut total = None;
    measure_dirs(
        &Arc::from(parent),
        &[CompactString::from(name)],
        cancel,
        |op| {
            if let FilesOp::Size { sizes, .. } = op {
                total = sizes.last().map(|(_, bytes)| *bytes).or(total);
            }
        },
    );
    total.filter(|_| !cancel.is_cancelled())
}

/// Adds what was read about `path` to the open sheet.
pub fn adopt_facts(runtime: &mut Runtime, store: Store, path: &Path, facts: FileFacts) {
    update_sheet(runtime, store, path, |sheet| {
        if !sheet.is_dir {
            sheet.size = Some(facts.len);
        }
        sheet.facts = Some(facts);
    });
}

pub fn adopt_folder_size(runtime: &mut Runtime, store: Store, path: &Path, bytes: u64) {
    update_sheet(runtime, store, path, |sheet| sheet.size = Some(bytes));
}

fn update_sheet(
    runtime: &mut Runtime,
    store: Store,
    path: &Path,
    change: impl FnOnce(&mut PropertySheet),
) {
    let shown = store.properties.sheet.get(runtime);
    if let Some(mut sheet) = shown.filter(|sheet| sheet.path == path) {
        change(&mut sheet);
        store.properties.sheet.set(runtime, Some(sheet));
    }
}

fn file_len(metadata: &Metadata) -> u64 {
    if metadata.is_dir() { 0 } else { metadata.len() }
}

fn stamp(seconds: i64, nanoseconds: i64) -> Timestamp {
    Timestamp::from_parts(seconds, u32::try_from(nanoseconds).unwrap_or(0))
}

fn system_stamp(time: io::Result<SystemTime>) -> Timestamp {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| {
            Some(Timestamp::from_parts(
                i64::try_from(since.as_secs()).ok()?,
                since.subsec_nanos(),
            ))
        })
        .unwrap_or(Timestamp::UNKNOWN)
}
