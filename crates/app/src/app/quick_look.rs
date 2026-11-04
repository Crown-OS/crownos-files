//! Quick look: crownos-preview shows the folder's previewable files and the
//! selection and the viewer follow each other while it is open.

use std::{path::Path, sync::Arc};

use crownui::prelude::{Cx, Runtime, Store as Cell};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    app::{
        open_plan::{PreviewList, preview_list_at},
        selection::move_cursor,
    },
    preview::{PreviewEvent, QuickLook},
    state::Store,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickLookCommand {
    Show { paths: Vec<String>, index: u32 },
    Select(u32),
    Close,
}

/// The list the viewer was given, by folder position, and what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSession {
    pub dir: Arc<Path>,
    pub positions: Vec<usize>,
    pub shown: u32,
}

#[derive(Clone, Copy)]
pub struct QuickLookState {
    pub sender: Cell<Option<UnboundedSender<QuickLookCommand>>>,
    pub session: Cell<Option<PreviewSession>>,
}

impl QuickLookState {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            sender: cx.store(None),
            session: cx.store(None),
        }
    }

    fn send(self, runtime: &Runtime, command: QuickLookCommand) {
        self.sender.with(runtime, |sender| {
            if let Some(sender) = sender {
                let _ = sender.send(command);
            }
        });
    }

    pub fn is_open(self, runtime: &Runtime) -> bool {
        self.session.with(runtime, Option::is_some)
    }
}

/// Talks to crownos-preview until the window closes.
pub async fn run_quick_look(
    events: UnboundedSender<PreviewEvent>,
    mut commands: UnboundedReceiver<QuickLookCommand>,
) {
    let (client, mut previews) = QuickLook::spawn();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(QuickLookCommand::Show { paths, index }) => client.show(paths, index),
                Some(QuickLookCommand::Select(index)) => client.select(index),
                Some(QuickLookCommand::Close) => client.close(),
                None => break,
            },
            Some(event) = previews.recv() => {
                if events.send(event).is_err() {
                    break;
                }
            }
        }
    }
}

/// Shows `list` in the viewer.
pub fn show_quick_look(runtime: &mut Runtime, store: Store, list: PreviewList) {
    let PreviewList {
        paths,
        positions,
        index,
    } = list;
    let dir = store.dir(runtime);
    store.quick_look.session.update(runtime, |session| {
        *session = Some(PreviewSession {
            dir,
            positions,
            shown: index,
        });
    });
    store
        .quick_look
        .send(runtime, QuickLookCommand::Show { paths, index });
}

/// Space: closes the viewer, or opens it on the entry under the cursor.
pub fn toggle_quick_look(cx: &mut Cx, store: Store) {
    if store.quick_look.is_open(cx) {
        close_quick_look(cx, store);
        return;
    }
    let cursor = store.folder.with(cx, |folder| {
        let cursor = folder.cursor();
        folder.is_selected(cursor).then(|| {
            (
                preview_list_at(folder, cursor),
                folder.get(cursor).map(|entry| entry.name.clone()),
            )
        })
    });
    match cursor {
        Some((Some(list), _)) => show_quick_look(cx, store, list),
        Some((None, Some(name))) => store
            .toasts
            .show(cx, format!("Quick Look can't show “{name}”")),
        _ => {}
    }
}

pub fn close_quick_look(runtime: &mut Runtime, store: Store) {
    store
        .quick_look
        .session
        .update(runtime, |session| *session = None);
    store.quick_look.send(runtime, QuickLookCommand::Close);
}

/// While the viewer is open, a new cursor in the list moves it too, and
/// leaving the folder closes it.
pub fn follow_selection(cx: &mut Cx, store: Store) {
    cx.effect(move |runtime| {
        store.selection_rev.get(runtime);
        let dir = store.cwd.get(runtime);
        let cursor = store.folder.with(runtime, |folder| folder.cursor());
        enum Step {
            Stay,
            Select(u32),
            Close,
        }
        let step = store.quick_look.session.update(runtime, |session| {
            let Some(session) = session else {
                return Step::Stay;
            };
            if session.dir != dir {
                return Step::Close;
            }
            match session
                .positions
                .iter()
                .position(|&position| position == cursor)
            {
                Some(index) if index as u32 != session.shown => {
                    session.shown = index as u32;
                    Step::Select(session.shown)
                }
                _ => Step::Stay,
            }
        });
        match step {
            Step::Stay => {}
            Step::Select(index) => store
                .quick_look
                .send(runtime, QuickLookCommand::Select(index)),
            Step::Close => close_quick_look(runtime, store),
        }
    });
}

/// The viewer stepped or closed.
pub fn finish_quick_look(cx: &mut Cx, store: Store, events: Vec<PreviewEvent>) {
    for event in events {
        match event {
            PreviewEvent::Navigated { index } => {
                let position = store.quick_look.session.update(cx, |session| {
                    let session = session.as_mut()?;
                    let position = *session.positions.get(index as usize)?;
                    session.shown = index;
                    Some(position)
                });
                if let Some(position) = position {
                    move_cursor(cx, store, position, false);
                }
            }
            PreviewEvent::Closed => store
                .quick_look
                .session
                .update(cx, |session| *session = None),
        }
    }
}
