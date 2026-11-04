//! Searching under a folder: typing in the search box opens the Search
//! results in place of the folder, first with the folder's own matches and
//! then with every match the background walk finds beneath it. Clearing the
//! box goes back to the folder.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crownos_files_devices::tags::SearchQuery;
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio::{sync::mpsc::UnboundedSender, task::spawn_blocking};
use tokio_util::sync::CancellationToken;

use crate::{
    app::{
        listing::{apply_ops, reorder},
        navigation::{QueryOnEnter, show_dir},
        tasks::{Task, TaskDone},
        virtual_folder::{VirtualFolder, absolute_entry, entry_at},
    },
    fs::search::{SearchScope, search_tree},
    model::{
        dir_stamp::DirStamp, entry::Entry, files_op::FilesOp, filter::Filter, ticket::Ticket,
        timestamp::Timestamp,
    },
    state::Store,
};

/// How long typing must pause before a walk starts.
const TYPING_PAUSE: Duration = Duration::from_millis(150);

#[derive(Clone, Copy)]
pub struct Searching {
    /// The folder the search looks under.
    scope: Cell<Option<Arc<Path>>>,
    walk: Cell<Option<(Ticket, CancellationToken)>>,
    /// A walk is still looking.
    pub active: Signal<bool>,
}

/// What the task worker walks.
#[derive(Debug)]
pub struct SearchRequest {
    pub(crate) scope: Arc<Path>,
    pub(crate) query: String,
    show_hidden: bool,
    pub(crate) ticket: Ticket,
    cancel: CancellationToken,
}

impl Searching {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            scope: cx.store(None),
            walk: cx.store(None),
            active: cx.signal(false),
        }
    }

    fn stop(self, runtime: &mut Runtime) {
        if let Some((_, cancel)) = self.walk.update(runtime, Option::take) {
            cancel.cancel();
        }
        if self.active.with_untracked(runtime, |active| *active) {
            self.active.set(runtime, false);
        }
    }
}

/// Opens, refreshes and leaves the Search results as the query changes.
pub fn follow_query(cx: &mut Cx, store: Store) {
    cx.effect(move |runtime| {
        let query = store.query.get(runtime);
        let dir = store.dir(runtime);
        let shown = VirtualFolder::of(&dir);
        let recursive = !query.is_empty() && !SearchQuery::parse(&query).has_tags();
        match (shown, recursive) {
            (Some(VirtualFolder::Search), true) => walk(runtime, store, query),
            (Some(VirtualFolder::Search), false) => leave(runtime, store),
            (None, true) => {
                open_results(runtime, store, dir);
                walk(runtime, store, query);
            }
            (_, false) | (Some(_), true) => store.searching.stop(runtime),
        }
    });
}

/// Shows the Search results for `scope`, seeded with its own entries so the
/// first matches appear at once.
fn open_results(runtime: &mut Runtime, store: Store, scope: Arc<Path>) {
    let seed: Vec<Entry> = store.folder.with(runtime, |folder| {
        folder
            .iter()
            .map(|entry| absolute_entry(entry.clone(), &entry.path_in(&scope)))
            .collect()
    });
    store
        .searching
        .scope
        .update(runtime, |slot| *slot = Some(scope));
    let results = VirtualFolder::Search.path();
    store.history.update(runtime, |history| {
        history.take(&results);
    });
    show_dir(runtime, store, results.clone(), QueryOnEnter::Keep);
    let ticket = Ticket::next();
    apply_ops(
        runtime,
        store,
        vec![
            FilesOp::Part {
                dir: results.clone(),
                ticket,
                entries: seed,
            },
            done(results, ticket),
        ],
    );
    reorder(runtime, store);
}

/// Starts a walk for `query`, superseding the one running.
fn walk(runtime: &mut Runtime, store: Store, query: String) {
    store.searching.stop(runtime);
    let Some(scope) = store.searching.scope.with(runtime, Clone::clone) else {
        return;
    };
    let ticket = Ticket::next();
    let cancel = CancellationToken::new();
    store
        .searching
        .walk
        .update(runtime, |walk| *walk = Some((ticket, cancel.clone())));
    store.searching.active.set(runtime, true);
    let show_hidden = store.show_hidden.get(runtime);
    store.run(
        runtime,
        Task::Search(SearchRequest {
            scope,
            query,
            show_hidden,
            ticket,
            cancel,
        }),
    );
}

/// Walks again for the query in the box, as after files changed.
pub fn search_again(runtime: &mut Runtime, store: Store) {
    let query = store.query.with_untracked(runtime, Clone::clone);
    walk(runtime, store, query);
}

/// Back to the folder searched, dropping the results.
fn leave(runtime: &mut Runtime, store: Store) {
    store.searching.stop(runtime);
    let Some(scope) = store.searching.scope.update(runtime, Option::take) else {
        return;
    };
    show_dir(runtime, store, scope, QueryOnEnter::Keep);
    store.history.update(runtime, |history| {
        history.take(&VirtualFolder::Search.path());
    });
}

/// Adds what a walk found, if it is still the one asked for.
pub fn finish_search(runtime: &mut Runtime, store: Store, ticket: Ticket, found: Found) {
    let current = store
        .searching
        .walk
        .with(runtime, |walk| walk.as_ref().map(|(known, _)| *known))
        == Some(ticket);
    if !current {
        return;
    }
    let results = VirtualFolder::Search.path();
    let ops = match found {
        Found::Some(entries) => vec![FilesOp::Part {
            dir: results,
            ticket,
            entries,
        }],
        Found::End => {
            store.searching.walk.update(runtime, |walk| *walk = None);
            store.searching.active.set(runtime, false);
            vec![done(results, ticket)]
        }
    };
    apply_ops(runtime, store, ops);
}

/// A batch of a walk's matches, or its end.
#[derive(Debug)]
pub enum Found {
    Some(Vec<Entry>),
    End,
}

fn done(dir: Arc<Path>, ticket: Ticket) -> FilesOp {
    FilesOp::Done {
        dir,
        ticket,
        stamp: DirStamp {
            modified: Timestamp::UNKNOWN,
            changed: Timestamp::UNKNOWN,
            len: 0,
            inode: 0,
            device: 0,
        },
    }
}

/// Walks for `request` once typing pauses, streaming matches to `done`.
pub async fn run_search(request: SearchRequest, done: UnboundedSender<TaskDone>) {
    tokio::time::sleep(TYPING_PAUSE).await;
    let Some(filter) = Filter::new(&request.query) else {
        return;
    };
    let SearchRequest {
        scope,
        show_hidden,
        ticket,
        cancel,
        ..
    } = request;
    let _ = spawn_blocking(move || {
        let scope = SearchScope {
            root: &scope,
            filter: &filter,
            show_hidden,
        };
        let send = |found| {
            let _ = done.send(TaskDone::Found(ticket, found));
        };
        search_tree(&scope, &cancel, |paths: Vec<PathBuf>| {
            let entries = paths
                .iter()
                .filter_map(|path| entry_at(path, None))
                .collect();
            send(Found::Some(entries));
        });
        if !cancel.is_cancelled() {
            send(Found::End);
        }
    })
    .await;
}
