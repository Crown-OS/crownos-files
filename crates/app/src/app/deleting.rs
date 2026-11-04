//! Moving to the trash, deleting for good (after asking, as the config
//! says), and the trash's own Restore, Delete and Empty.

use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};

use crate::{
    app::{
        file_jobs::{Purpose, submit},
        selection::selected_paths,
        virtual_folder::{VirtualFolder, trashed_at},
    },
    jobs::Job,
    state::Store,
};

/// The question the delete dialog asks, and the job its button runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePrompt {
    pub title: String,
    pub detail: &'static str,
    pub action: &'static str,
    pub job: Job,
}

#[derive(Clone, Copy)]
pub struct Deletion {
    pub prompt: Cell<Option<DeletePrompt>>,
    pub open: Signal<bool>,
}

impl Deletion {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            prompt: cx.store(None),
            open: cx.signal(false),
        }
    }
}

const CANNOT_UNDO: &str = "This cannot be undone.";

fn in_trash(runtime: &Runtime, store: Store) -> bool {
    VirtualFolder::of(&store.dir(runtime)) == Some(VirtualFolder::Trash)
}

fn counted(count: usize) -> String {
    match count {
        1 => "this item".to_owned(),
        count => format!("{count} items"),
    }
}

/// Delete: to the trash, or out of it for good when the trash is open.
pub fn trash_selection(cx: &mut Cx, store: Store) {
    if in_trash(cx, store) {
        delete_selection(cx, store);
        return;
    }
    let paths = selected_paths(cx, store);
    submit(cx, store, Job::Trash(paths), Purpose::Plain);
}

/// Shift+Delete: deletes the selection without the trash.
pub fn delete_selection(cx: &mut Cx, store: Store) {
    let paths = selected_paths(cx, store);
    if paths.is_empty() {
        return;
    }
    let title = format!("Delete {} permanently?", counted(paths.len()));
    let job = if in_trash(cx, store) {
        Job::DeleteFromTrash(trashed_at(cx, store, &paths))
    } else {
        Job::Delete(paths)
    };
    ask_or_run(
        cx,
        store,
        DeletePrompt {
            title,
            detail: CANNOT_UNDO,
            action: "Delete",
            job,
        },
    );
}

pub fn restore_selection(cx: &mut Cx, store: Store) {
    let paths = selected_paths(cx, store);
    let items = trashed_at(cx, store, &paths);
    if !items.is_empty() {
        submit(cx, store, Job::Restore(items), Purpose::Plain);
    }
}

pub fn empty_trash(cx: &mut Cx, store: Store) {
    ask_or_run(
        cx,
        store,
        DeletePrompt {
            title: "Empty the Trash?".to_owned(),
            detail: "Everything in the Trash will be deleted permanently.",
            action: "Empty",
            job: Job::EmptyTrash,
        },
    );
}

fn ask_or_run(cx: &mut Cx, store: Store, prompt: DeletePrompt) {
    if !store.confirm_permanent_delete.get(cx) {
        submit(cx, store, prompt.job, Purpose::Plain);
        return;
    }
    store
        .deletion
        .prompt
        .update(cx, |slot| *slot = Some(prompt));
    store.deletion.open.set(cx, true);
}

/// The dialog's destructive button.
pub fn confirm_delete(cx: &mut Cx, store: Store) {
    store.deletion.open.set(cx, false);
    if let Some(prompt) = store.deletion.prompt.update(cx, Option::take) {
        submit(cx, store, prompt.job, Purpose::Plain);
    }
}
