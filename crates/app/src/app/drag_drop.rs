//! Dragging files out of the window, and dropping files (from here or from
//! other apps) on folders, the path bar, the sidebar and the open folder.
//! A drop moves within one filesystem and copies across; Shift forces a
//! move and Ctrl a copy.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crownui::{
    ext::{DragOverEvent, DropEvent, Modifiers},
    prelude::{
        Color, Cx, DndAction, DndActions, DragSource, Event, Runtime, Signal, Store as Cell,
    },
};

use crate::{
    app::{
        file_jobs::{Purpose, TransferMode, submit, transfer},
        pinning::show_pin_zone,
        selection::{ensure_selected, selected_paths},
        spring_load::{SpringLoad, arm, disarm, disarm_from},
        virtual_folder::VirtualFolder,
    },
    fs::mounts::device_id,
    jobs::Job,
    model::{badge::kind_color, places::Places},
    state::Store,
    util::{
        clipboard_items::{all_in, file_items, read_files},
        clipboard_payload::{ClipboardAction, URI_LIST_MIME},
        drag_icon::{DragArt, DragIconColors, drag_icon},
    },
    views::{
        entry_facts::glyph_of,
        glyph::{
            palette::{FOLDER_FRONT, badge_color},
            render_drag_glyph,
        },
    },
};

/// What a drag started here carries, so drops back into the window know it.
#[derive(Debug)]
struct DragSources {
    paths: Vec<PathBuf>,
    device: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct Dragging {
    sources: Cell<Option<DragSources>>,
    target_device: Cell<Option<(Arc<Path>, Option<u64>)>>,
    pub spring: Cell<SpringLoad>,
    /// The sidebar's drop zone for pinning folders is open.
    pub pin_zone: Signal<bool>,
}

impl Dragging {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            sources: cx.store(None),
            target_device: cx.store(None),
            spring: cx.store(SpringLoad::default()),
            pin_zone: cx.signal(false),
        }
    }

    /// The drag in progress started in this window.
    pub fn is_internal(self, runtime: &Runtime) -> bool {
        self.sources.with(runtime, Option::is_some)
    }
}

/// The action a drop takes: Shift moves, Ctrl copies, otherwise a move on
/// the same filesystem and a copy across, among those the source allows.
pub fn choose_action(
    modifiers: Modifiers,
    allowed: DndActions,
    same_device: Option<bool>,
) -> Option<DndAction> {
    let preferred = if modifiers.shift_key {
        DndAction::Move
    } else if modifiers.ctrl_key || same_device != Some(true) {
        DndAction::Copy
    } else {
        DndAction::Move
    };
    [preferred, DndAction::Copy, DndAction::Move]
        .into_iter()
        .find(|action| allowed.contains(*action))
}

/// Starts dragging the selection from the entry at `position`.
pub fn start_entry_drag(
    cx: &mut Cx,
    store: Store,
    position: usize,
    badge: Color,
    badge_text: Color,
) {
    ensure_selected(cx, store, position);
    let paths = selected_paths(cx, store);
    let places = store.places.with(cx, Places::clone);
    let Some((path, card, glyph)) = store.folder.with(cx, |folder| {
        let entry = folder.get(position)?;
        let card = if entry.is_dir() {
            FOLDER_FRONT.0
        } else {
            badge_color(kind_color(entry.kind))
        };
        let glyph = glyph_of(entry, folder.dir(), &places);
        Some((entry.path_in(folder.dir()), card, glyph))
    }) else {
        return;
    };
    let swatch = store
        .thumbs
        .images
        .with(cx, |images| images.swatch(&path).cloned());
    let colors = DragIconColors {
        card,
        badge,
        badge_text,
    };
    let scale = store.scale.get(cx);
    let art = match swatch.as_ref() {
        Some(swatch) => DragArt::Thumbnail(swatch),
        None => render_drag_glyph(glyph, scale).map_or(DragArt::Card, DragArt::Rendered),
    };
    let icon = drag_icon(art, paths.len(), colors, scale);
    let source = DragSource {
        items: file_items(ClipboardAction::Copy, &paths),
        icon: Some(icon),
        actions: DndActions::COPY_MOVE,
    };
    let device = paths.first().and_then(|path| device_id(path).ok());
    let carries_folder = paths.iter().any(|path| path.is_dir());
    store
        .drag
        .sources
        .update(cx, |sources| *sources = Some(DragSources { paths, device }));
    show_pin_zone(cx, store, carries_folder);
    if !cx.start_drag(source, move |cx, _| end_drag(cx, store)) {
        end_drag(cx, store);
    }
}

fn end_drag(runtime: &mut Runtime, store: Store) {
    store
        .drag
        .sources
        .update(runtime, |sources| *sources = None);
    store
        .drag
        .target_device
        .update(runtime, |cached| *cached = None);
    show_pin_zone(runtime, store, false);
    disarm(runtime, store);
}

/// The filesystem of `dir`, remembered for the drag's current target.
fn target_device(runtime: &mut Runtime, store: Store, dir: &Arc<Path>) -> Option<u64> {
    let cached = store.drag.target_device.with(runtime, |cached| {
        cached
            .as_ref()
            .filter(|(target, _)| target == dir)
            .map(|(_, device)| *device)
    });
    if let Some(device) = cached {
        return device;
    }
    let device = device_id(dir).ok();
    store
        .drag
        .target_device
        .update(runtime, |cached| *cached = Some((dir.clone(), device)));
    device
}

/// A drag over the folder `dir`: accepts it with the action a drop would
/// take, and with `spring` opens the folder if the drag rests on it.
pub fn drag_over_folder(
    cx: &mut Cx,
    store: Store,
    dir: &Arc<Path>,
    spring: bool,
    event: &mut Event<DragOverEvent>,
) -> bool {
    event.stop_propagation();
    if !event.offers(URI_LIST_MIME) || VirtualFolder::of(dir).is_some() {
        return false;
    }
    let internal = store.drag.sources.with(cx, |sources| {
        sources.as_ref().map(|sources| {
            let into_itself = sources.paths.iter().any(|path| dir.starts_with(path));
            (into_itself, all_in(&sources.paths, dir), sources.device)
        })
    });
    if internal.is_none() {
        show_pin_zone(cx, store, true);
    }
    let same_device = match internal {
        Some((true, ..)) => return false,
        Some((false, _, Some(device))) => Some(target_device(cx, store, dir) == Some(device)),
        _ => None,
    };
    let Some(action) = choose_action(event.modifiers, event.source_actions, same_device) else {
        return false;
    };
    if action == DndAction::Move && internal.is_some_and(|(_, already_there, _)| already_there) {
        return false;
    }
    event.accept_with(URI_LIST_MIME, action);
    if spring {
        arm(cx, store, dir.clone());
    }
    true
}

pub fn leave_folder(runtime: &mut Runtime, store: Store, dir: &Path) {
    disarm_from(runtime, store, dir);
}

/// Files dropped on `dir`: moved or copied as the drop settled.
pub fn drop_on_folder(cx: &mut Cx, store: Store, dir: &Path, event: &mut Event<DropEvent>) {
    event.stop_propagation();
    show_pin_zone(cx, store, false);
    disarm(cx, store);
    let Some((_, paths)) = read_files(&event.mime_type, &event.data) else {
        return;
    };
    let mode = match event.action {
        DndAction::Move => TransferMode::Move,
        DndAction::Copy | DndAction::Ask => TransferMode::Copy,
    };
    transfer(cx, store, paths, dir, mode, Purpose::Plain);
}

/// The Trash place takes files as a move.
pub fn drag_over_trash(event: &mut Event<DragOverEvent>) {
    event.stop_propagation();
    if event.offers(URI_LIST_MIME) && event.source_actions.contains(DndAction::Move) {
        event.accept_with(URI_LIST_MIME, DndAction::Move);
    }
}

pub fn drop_on_trash(cx: &mut Cx, store: Store, event: &mut Event<DropEvent>) {
    event.stop_propagation();
    show_pin_zone(cx, store, false);
    if let Some((_, paths)) = read_files(&event.mime_type, &event.data) {
        submit(cx, store, Job::Trash(paths), Purpose::Plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: Modifiers = Modifiers {
        shift_key: true,
        ..Modifiers::NONE
    };
    const CTRL: Modifiers = Modifiers {
        ctrl_key: true,
        ..Modifiers::NONE
    };

    #[test]
    fn the_same_filesystem_moves_others_copy_and_modifiers_force_either() {
        let both = DndActions::COPY_MOVE;
        assert_eq!(
            choose_action(Modifiers::NONE, both, Some(true)),
            Some(DndAction::Move)
        );
        assert_eq!(
            choose_action(Modifiers::NONE, both, Some(false)),
            Some(DndAction::Copy)
        );
        assert_eq!(
            choose_action(Modifiers::NONE, both, None),
            Some(DndAction::Copy)
        );
        assert_eq!(
            choose_action(SHIFT, both, Some(false)),
            Some(DndAction::Move)
        );
        assert_eq!(choose_action(CTRL, both, Some(true)), Some(DndAction::Copy));
    }

    #[test]
    fn only_what_the_source_allows_is_chosen() {
        assert_eq!(
            choose_action(SHIFT, DndActions::COPY, None),
            Some(DndAction::Copy)
        );
        assert_eq!(
            choose_action(Modifiers::NONE, DndActions::MOVE, Some(false)),
            Some(DndAction::Move)
        );
        assert_eq!(choose_action(Modifiers::NONE, DndActions::NONE, None), None);
    }
}
