//! Everything the window shows, as one `Copy` handle of signals and
//! untracked cells. Views read the current [`Folder`] by index from its cell
//! and redraw when `revision` (the listing) or `selection_rev` (selection and
//! cursor) moves, so no entry is ever cloned into a signal.

use std::{path::Path, sync::Arc};

use crownconfig::{Files, TagColor, ViewMode};
use crownui::{
    kit::{Toasts, provide_toasts},
    prelude::{Cx, Point, Rect, Runtime, ScrollHandle, Signal, Store as Cell},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{
        config_sync::sort_order, conflicts::Conflicts, deleting::Deletion, drag_drop::Dragging,
        editing::Editing, file_jobs::Jobs, listing::AfterChange, opening::Opening,
        privilege::ElevationPrompt, quick_look::QuickLookState, tasks::Task,
        thumbnails::Thumbnails, type_ahead::TypeAhead,
    },
    fs::engine::EngineCmd,
    model::{
        backstack::Backstack, device_entry::DeviceEntry, disk_usage::DiskUsage, folder::Folder,
        history::History, places::Places, sort::SortOrder, trash::TrashedItem,
        view_options::ViewOptions,
    },
    util::friendly_date::Today,
    views::{file_menu::MenuPage, focus_slot::FocusSlot},
};

/// Whether the toolbar's back and forward buttons lead anywhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NavigationState {
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[derive(Clone, Copy)]
pub struct Store {
    pub folder: Cell<Folder>,
    pub history: Cell<History>,
    pub backstack: Cell<Backstack<Arc<Path>>>,
    pub places: Cell<Places>,
    pub today: Cell<Today>,
    pub engine: Cell<Option<UnboundedSender<EngineCmd>>>,
    pub tasks: Cell<Option<UnboundedSender<Task>>>,
    pub after_change: Cell<Option<AfterChange>>,
    pub type_ahead: Cell<TypeAhead>,
    pub pointer: Cell<Point>,
    pub cwd: Signal<Arc<Path>>,
    pub revision: Signal<u64>,
    pub count: Signal<usize>,
    pub selection_rev: Signal<u64>,
    pub navigation: Signal<NavigationState>,
    pub view_mode: Signal<ViewMode>,
    pub show_hidden: Signal<bool>,
    pub sort: Signal<SortOrder>,
    pub query: Signal<String>,
    pub devices: Signal<Vec<DeviceEntry>>,
    pub storage: Signal<Option<DiskUsage>>,
    pub tags: Signal<Vec<TagColor>>,
    pub editing: Signal<Editing>,
    pub edit_text: Signal<String>,
    pub menu: Signal<Option<Point>>,
    pub band: Signal<Option<Rect>>,
    pub scroll: ScrollHandle,
    pub search: FocusSlot,
    pub file_area: FocusSlot,
    pub toasts: Toasts,
    pub scale: Signal<f32>,
    pub thumbs: Thumbnails,
    pub opening: Opening,
    pub quick_look: QuickLookState,
    pub jobs: Jobs,
    pub conflicts: Conflicts,
    pub elevation: Signal<Option<ElevationPrompt>>,
    pub deletion: Deletion,
    pub drag: Dragging,
    pub trash_items: Cell<Vec<TrashedItem>>,
    pub confirm_permanent_delete: Signal<bool>,
    pub menu_page: Signal<MenuPage>,
}

impl Store {
    /// A store showing `dir`, not yet listed, viewed as `files` says.
    pub fn new(cx: &mut Cx, dir: Arc<Path>, files: &Files, places: Places, today: Today) -> Self {
        let sort = sort_order(files);
        let view = ViewOptions {
            sort,
            show_hidden: files.show_hidden,
            filter: None,
        };
        Self {
            folder: cx.store(Folder::new(dir.clone(), view)),
            history: cx.store(History::default()),
            backstack: cx.store(Backstack::new(dir.clone())),
            places: cx.store(places),
            today: cx.store(today),
            engine: cx.store(None),
            tasks: cx.store(None),
            after_change: cx.store(None),
            type_ahead: cx.store(TypeAhead::default()),
            pointer: cx.store(Point::ZERO),
            cwd: cx.signal(dir),
            revision: cx.signal(0),
            count: cx.signal(0),
            selection_rev: cx.signal(0),
            navigation: cx.signal(NavigationState::default()),
            view_mode: cx.signal(files.view_mode),
            show_hidden: cx.signal(files.show_hidden),
            sort: cx.signal(sort),
            query: cx.signal(String::new()),
            devices: cx.signal(Vec::new()),
            storage: cx.signal(None),
            tags: cx.signal(Vec::new()),
            editing: cx.signal(Editing::Nothing),
            edit_text: cx.signal(String::new()),
            menu: cx.signal(None),
            band: cx.signal(None),
            scroll: ScrollHandle::new(cx),
            search: FocusSlot::new(cx),
            file_area: FocusSlot::new(cx),
            toasts: provide_toasts(cx),
            scale: cx.scale_factor(),
            thumbs: Thumbnails::new(cx),
            opening: Opening::new(cx),
            quick_look: QuickLookState::new(cx),
            jobs: Jobs::new(cx),
            conflicts: Conflicts::new(cx),
            elevation: cx.signal(None),
            deletion: Deletion::new(cx),
            drag: Dragging::new(cx),
            trash_items: cx.store(Vec::new()),
            confirm_permanent_delete: cx.signal(files.confirm_permanent_delete),
            menu_page: cx.signal(MenuPage::Main),
        }
    }

    pub fn send(self, runtime: &Runtime, command: EngineCmd) {
        self.engine.with(runtime, |engine| {
            if let Some(engine) = engine {
                let _ = engine.send(command);
            }
        });
    }

    pub fn run(self, runtime: &Runtime, task: Task) {
        self.tasks.with(runtime, |tasks| {
            if let Some(tasks) = tasks {
                let _ = tasks.send(task);
            }
        });
    }

    pub fn dir(self, runtime: &Runtime) -> Arc<Path> {
        self.folder.with(runtime, |folder| folder.dir().clone())
    }
}
