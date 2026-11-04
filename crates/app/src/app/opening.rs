//! Opening the selection, the context menu's "Open With" list and the "Open
//! With" dialog, on the UI side of the opener worker.

use std::path::PathBuf;

use crownui::{
    ext::ImageId,
    prelude::{Cx, Runtime, Signal, Size, Store as Cell},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{
        navigation::open_dir,
        open_plan::{OpenPlan, OpenTarget, plan_open},
        opener::{AppChoices, AppsPurpose, OpenCommand, OpenEvent},
        quick_look::show_quick_look,
    },
    model::mime_id::MimeId,
    state::Store,
};

/// Logical size of an app icon in the "Open With" dialog.
pub const APP_ICON_SIZE: f32 = 28.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuApp {
    pub id: Box<str>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptApp {
    pub id: Box<str>,
    pub name: String,
    pub icon: Option<(ImageId, Size)>,
}

/// What the "Open With" dialog offers; the dialog releases the icons when it closes.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenWithPrompt {
    pub title: String,
    pub mime: MimeId,
    pub files: Vec<PathBuf>,
    pub apps: Vec<PromptApp>,
}

#[derive(Clone, Copy)]
pub struct Opening {
    pub sender: Cell<Option<UnboundedSender<OpenCommand>>>,
    pub menu_apps: Signal<Option<Vec<MenuApp>>>,
    pub prompt: Signal<Option<OpenWithPrompt>>,
    pub open: Signal<bool>,
    pub chosen: Signal<Option<usize>>,
    pub make_default: Signal<bool>,
}

impl Opening {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            sender: cx.store(None),
            menu_apps: cx.signal(None),
            prompt: cx.signal(None),
            open: cx.signal(false),
            chosen: cx.signal(None),
            make_default: cx.signal(false),
        }
    }

    fn send(self, runtime: &Runtime, command: OpenCommand) {
        self.sender.with(runtime, |sender| {
            if let Some(sender) = sender {
                let _ = sender.send(command);
            }
        });
    }
}

/// Enter, a double click and the details panel's Open.
pub fn open_selection(cx: &mut Cx, store: Store) {
    match store.folder.with(cx, plan_open) {
        OpenPlan::Nothing => {}
        OpenPlan::Navigate(dir) => open_dir(cx, store, dir),
        OpenPlan::Preview(list) => show_quick_look(cx, store, list),
        OpenPlan::Launch(targets) => {
            let icon_edge = icon_edge(cx, store);
            store
                .opening
                .send(cx, OpenCommand::Launch { targets, icon_edge });
        }
    }
}

/// The selected files, for "Open With".
fn selected_files(runtime: &Runtime, store: Store) -> Vec<OpenTarget> {
    store.folder.with(runtime, |folder| {
        let dir = folder.dir();
        folder
            .selected_entries()
            .filter(|entry| !entry.is_dir())
            .map(|entry| OpenTarget {
                path: entry.path_in(dir),
                kind: entry.kind,
                mime: entry.mime,
            })
            .collect()
    })
}

fn icon_edge(runtime: &mut Runtime, store: Store) -> u32 {
    (APP_ICON_SIZE * store.scale.get(runtime)).ceil() as u32
}

/// Starts listing the applications for the "Open With" submenu.
pub fn request_menu_apps(cx: &mut Cx, store: Store) {
    store.opening.menu_apps.set(cx, None);
    let targets = selected_files(cx, store);
    if !targets.is_empty() {
        store.opening.send(
            cx,
            OpenCommand::Apps {
                targets,
                purpose: AppsPurpose::Menu,
                icon_edge: 0,
            },
        );
    }
}

/// "Other Application…": the dialog of every application for the type.
pub fn choose_app(cx: &mut Cx, store: Store) {
    let targets = selected_files(cx, store);
    if targets.is_empty() {
        return;
    }
    let icon_edge = icon_edge(cx, store);
    store.opening.send(
        cx,
        OpenCommand::Apps {
            targets,
            purpose: AppsPurpose::Dialog,
            icon_edge,
        },
    );
}

/// A submenu row: opens the selected files in `app`.
pub fn open_with(cx: &mut Cx, store: Store, app: Box<str>) {
    let files = selected_files(cx, store)
        .into_iter()
        .map(|target| target.path)
        .collect();
    store.opening.send(
        cx,
        OpenCommand::OpenWith {
            app,
            files,
            default_for: None,
        },
    );
}

/// The dialog's Open: the chosen application, made the default if asked.
pub fn confirm_choice(cx: &mut Cx, store: Store) {
    let opening = store.opening;
    let chosen = opening.chosen.get(cx);
    let make_default = opening.make_default.get(cx);
    let command = opening.prompt.with(cx, |prompt| {
        let prompt = prompt.as_ref()?;
        let app = prompt.apps.get(chosen?)?;
        Some(OpenCommand::OpenWith {
            app: app.id.clone(),
            files: prompt.files.clone(),
            default_for: make_default.then_some(prompt.mime),
        })
    });
    if let Some(command) = command {
        opening.send(cx, command);
    }
    opening.open.set(cx, false);
}

pub fn finish_opening(cx: &mut Cx, store: Store, events: Vec<OpenEvent>) {
    for event in events {
        match event {
            OpenEvent::Failed(message) => store.toasts.show(cx, message),
            OpenEvent::Apps(choices) => match choices.purpose {
                AppsPurpose::Menu => show_menu_apps(cx, store, choices),
                AppsPurpose::Dialog => show_prompt(cx, store, choices),
            },
        }
    }
}

fn show_menu_apps(cx: &mut Cx, store: Store, choices: AppChoices) {
    if store.menu.with(cx, Option::is_none) {
        return;
    }
    let apps = choices
        .apps
        .into_iter()
        .map(|app| MenuApp {
            id: app.id,
            name: app.name,
        })
        .collect();
    store.opening.menu_apps.set(cx, Some(apps));
}

fn show_prompt(cx: &mut Cx, store: Store, choices: AppChoices) {
    let opening = store.opening;
    if opening.open.get(cx) {
        return;
    }
    let title = match choices.files.as_slice() {
        [one] => one.file_name().map_or_else(
            || "Open With".to_owned(),
            |name| format!("Open “{}” with", name.to_string_lossy()),
        ),
        files => format!("Open {} files with", files.len()),
    };
    let apps: Vec<PromptApp> = choices
        .apps
        .into_iter()
        .map(|app| PromptApp {
            icon: app.icon.map(|frame| {
                let size = Size::new(frame.width as f32, frame.height as f32);
                (
                    cx.register_image(frame.width, frame.height, frame.rgba),
                    size,
                )
            }),
            id: app.id,
            name: app.name,
        })
        .collect();
    opening.chosen.set(cx, (!apps.is_empty()).then_some(0));
    opening.make_default.set(cx, false);
    opening.prompt.set(
        cx,
        Some(OpenWithPrompt {
            title,
            mime: choices.mime,
            files: choices.files,
            apps,
        }),
    );
    opening.open.set(cx, true);
}
