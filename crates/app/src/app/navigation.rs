//! Moving between folders: the backstack, the history cache and the engine
//! commands each move owes.

use std::{ffi::OsStr, mem, path::Path, sync::Arc};

use crownui::prelude::{Runtime, Vector};

use crate::{
    app::{
        editing::Editing,
        listing::{AfterChange, publish, resolve_after_change, view_options},
        tasks::Task,
        virtual_folder::VirtualFolder,
    },
    fs::engine::EngineCmd,
    model::{
        dir_stamp::DirStamp,
        folder::{Folder, Stage},
    },
    state::{NavigationState, Store},
};

/// Whether moving to a folder empties the search box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryOnEnter {
    Clear,
    Keep,
}

/// Shows `dir` in place of the current folder without a history step, as
/// search does while it runs.
pub fn show_dir(runtime: &mut Runtime, store: Store, dir: Arc<Path>, query: QueryOnEnter) {
    enter(runtime, store, dir, query);
}

/// Opens `dir`, as a click on a folder or a sidebar place does.
pub fn open_dir(runtime: &mut Runtime, store: Store, dir: Arc<Path>) {
    if store.dir(runtime) == dir {
        return;
    }
    store
        .backstack
        .update(runtime, |backstack| backstack.push(dir.clone()));
    enter(runtime, store, dir, QueryOnEnter::Clear);
}

pub fn go_back(runtime: &mut Runtime, store: Store) {
    if let Some(dir) = store
        .backstack
        .update(runtime, |backstack| backstack.back().cloned())
    {
        enter(runtime, store, dir, QueryOnEnter::Clear);
    }
}

pub fn go_forward(runtime: &mut Runtime, store: Store) {
    if let Some(dir) = store
        .backstack
        .update(runtime, |backstack| backstack.forward().cloned())
    {
        enter(runtime, store, dir, QueryOnEnter::Clear);
    }
}

/// The folder holding `dir` and `dir`'s name in it; virtual folders and the root have none.
fn parent_of(dir: &Path) -> Option<(&Path, &OsStr)> {
    if VirtualFolder::of(dir).is_some() {
        return None;
    }
    Some((dir.parent()?, dir.file_name()?))
}

pub fn has_parent(dir: &Path) -> bool {
    parent_of(dir).is_some()
}

/// Opens the parent folder with the folder just left selected.
pub fn go_up(runtime: &mut Runtime, store: Store) {
    let dir = store.dir(runtime);
    let Some((parent, name)) = parent_of(&dir) else {
        return;
    };
    let name = name.to_string_lossy().into();
    store
        .after_change
        .update(runtime, |slot| *slot = Some(AfterChange::Select(name)));
    open_dir(runtime, store, parent.into());
}

/// Lists and watches the folder the store was opened on.
pub fn load_current(runtime: &mut Runtime, store: Store) {
    let dir = store.dir(runtime);
    request_listing(runtime, store, dir, None);
    publish(runtime, store);
}

/// Lists `dir` (again only if it changed since `revisit`'s stamp) and
/// watches it; a virtual folder is read by a task instead.
fn request_listing(
    runtime: &mut Runtime,
    store: Store,
    dir: Arc<Path>,
    revisit: Option<Option<DirStamp>>,
) {
    if let Some(folder) = VirtualFolder::of(&dir) {
        store.send(runtime, EngineCmd::Watch(Vec::new()));
        if folder != VirtualFolder::Search {
            store.run(runtime, Task::ListVirtual(folder));
        }
        return;
    }
    store.send(
        runtime,
        match revisit {
            Some(stamp) => EngineCmd::Revalidate {
                dir: dir.clone(),
                stamp,
            },
            None => EngineCmd::Load(dir.clone()),
        },
    );
    store.send(runtime, EngineCmd::Watch(vec![dir.clone()]));
    let home = store.places.with(runtime, |places| places.home.clone());
    store.run(runtime, Task::MeasureDisk { dir, home });
}

fn enter(runtime: &mut Runtime, store: Store, dir: Arc<Path>, query: QueryOnEnter) {
    if query == QueryOnEnter::Clear && store.query.with(runtime, |query| !query.is_empty()) {
        store.query.set(runtime, String::new());
    }
    if store.editing.get(runtime) != Editing::Nothing {
        store.editing.set(runtime, Editing::Nothing);
    }
    let view = view_options(runtime, store);
    let cached = store.history.update(runtime, |history| history.take(&dir));
    let revisit = cached.as_ref().map(Folder::stamp);
    let mut next = cached.unwrap_or_else(|| Folder::new(dir.clone(), view.clone()));
    next.set_sort(view.sort);
    next.set_show_hidden(view.show_hidden);
    next.set_filter(None);
    next.refresh_order();
    let restored = next.scroll_offset;

    let left_at = store
        .scroll
        .offset()
        .with_untracked(runtime, |offset| offset.y);
    let mut previous = store
        .folder
        .update(runtime, |folder| mem::replace(folder, next));
    previous.scroll_offset = left_at;
    if previous.stage() == Stage::Loading {
        store.send(runtime, EngineCmd::Cancel(previous.dir().clone()));
    }
    store
        .history
        .update(runtime, |history| history.store(previous));

    request_listing(runtime, store, dir.clone(), revisit);
    let can_go_up = has_parent(&dir);
    store.cwd.set(runtime, dir);
    let state = store.backstack.with(runtime, |backstack| NavigationState {
        can_go_back: backstack.can_go_back(),
        can_go_forward: backstack.can_go_forward(),
        can_go_up,
    });
    store.navigation.set(runtime, state);
    store
        .scroll
        .offset()
        .set(runtime, Vector::new(0.0, restored));
    publish(runtime, store);
    resolve_after_change(runtime, store);
}
