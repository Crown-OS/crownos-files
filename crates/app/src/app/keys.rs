//! The window's keyboard map.

use std::time::Instant;

use crownconfig::ViewMode;
use crownui::{
    ext::{Key, KeyboardEvent, Modifiers, NamedKey},
    prelude::{Cx, Event},
};

use crate::{
    app::{
        actions::{
            Deferred, deferred, new_folder, open_selection, rename_cursor, set_view_mode,
            toggle_hidden,
        },
        editing::{Editing, cancel, start_path_edit},
        navigation::{go_back, go_forward, go_up},
        selection::{clear, move_cursor, select_all},
        type_ahead::starts_with_folded,
    },
    state::Store,
    views::{file_menu::open_menu, metrics::layout_for},
};

pub fn handle_key(cx: &mut Cx, store: Store, event: &mut Event<KeyboardEvent>) {
    if event.is_default_prevented() {
        return;
    }
    let typing = store.editing.get(cx) != Editing::Nothing || store.search.is_focused(cx);
    let handled = if typing {
        event.key == Key::Named(NamedKey::Escape) && leave_text_field(cx, store)
    } else {
        shortcut(cx, store, event.key, event.modifiers)
    };
    if handled {
        event.prevent_default();
    }
}

fn leave_text_field(cx: &mut Cx, store: Store) -> bool {
    if store.editing.get(cx) != Editing::Nothing {
        cancel(cx, store);
    } else if store.query.with(cx, |query| !query.is_empty()) {
        store.query.set(cx, String::new());
    } else {
        store.file_area.focus(cx);
    }
    true
}

fn shortcut(cx: &mut Cx, store: Store, key: Key, modifiers: Modifiers) -> bool {
    let command = modifiers.ctrl_key || modifiers.meta_key;
    match key {
        Key::Character(character) if command => {
            return command_shortcut(
                cx,
                store,
                character.to_ascii_lowercase(),
                modifiers.shift_key,
            );
        }
        Key::Named(NamedKey::ArrowLeft) if modifiers.alt_key => go_back(cx, store),
        Key::Named(NamedKey::ArrowRight) if modifiers.alt_key => go_forward(cx, store),
        Key::Named(NamedKey::ArrowUp) if modifiers.alt_key => go_up(cx, store),
        Key::Named(NamedKey::Backspace) => go_up(cx, store),
        Key::Named(NamedKey::Enter) => open_selection(cx, store),
        Key::Named(NamedKey::Space) | Key::Character(' ') => {
            deferred(cx, store, Deferred::QuickLook);
        }
        Key::Named(NamedKey::F(2)) => rename_cursor(cx, store),
        Key::Named(NamedKey::Delete) if modifiers.shift_key => {
            deferred(cx, store, Deferred::DeletePermanently);
        }
        Key::Named(NamedKey::Delete) => deferred(cx, store, Deferred::Trash),
        Key::Named(NamedKey::Escape) => clear(cx, store),
        Key::Named(NamedKey::ContextMenu) => open_menu(cx, store),
        Key::Named(NamedKey::F(10)) if modifiers.shift_key => open_menu(cx, store),
        Key::Named(named) if !modifiers.alt_key => {
            return step(cx, store, named, modifiers.shift_key);
        }
        Key::Character(character) if !modifiers.alt_key && !character.is_control() => {
            type_ahead(cx, store, character);
        }
        _ => return false,
    }
    true
}

fn command_shortcut(cx: &mut Cx, store: Store, character: char, shift: bool) -> bool {
    match (character, shift) {
        ('l', _) => start_path_edit(cx, store),
        ('f', _) => store.search.focus(cx),
        ('a', _) => select_all(cx, store),
        ('h', _) => toggle_hidden(cx, store),
        ('1', _) => set_view_mode(cx, store, ViewMode::Grid),
        ('2', _) => set_view_mode(cx, store, ViewMode::List),
        ('n', true) => new_folder(cx, store),
        ('c', _) => deferred(cx, store, Deferred::Copy),
        ('x', _) => deferred(cx, store, Deferred::Cut),
        ('v', _) => deferred(cx, store, Deferred::Paste),
        ('z', _) => deferred(cx, store, Deferred::Undo),
        _ => return false,
    }
    true
}

/// Arrow keys, Home/End and PageUp/PageDown, moving through the grid row by row.
fn step(cx: &mut Cx, store: Store, key: NamedKey, extend: bool) -> bool {
    let count = store.count.get(cx);
    let last = count.saturating_sub(1);
    let mode = store.view_mode.get(cx);
    let columns = match mode {
        ViewMode::Grid => store.scroll.columns().get(cx).max(1),
        ViewMode::List => 1,
    };
    let viewport = store.scroll.size().get(cx).height;
    let rows_per_page = ((viewport / layout_for(mode).stride()).floor() as usize).max(1);
    let (cursor, has_selection) = store.folder.with(cx, |folder| {
        (folder.cursor(), !folder.selection().is_empty())
    });
    let target = match key {
        NamedKey::ArrowLeft => cursor.saturating_sub(1),
        NamedKey::ArrowRight => (cursor + 1).min(last),
        NamedKey::ArrowUp => cursor.checked_sub(columns).unwrap_or(cursor),
        NamedKey::ArrowDown if cursor + columns <= last => cursor + columns,
        NamedKey::ArrowDown => cursor,
        NamedKey::Home => 0,
        NamedKey::End => last,
        NamedKey::PageUp => cursor.saturating_sub(rows_per_page * columns),
        NamedKey::PageDown => (cursor + rows_per_page * columns).min(last),
        _ => return false,
    };
    if count > 0 {
        move_cursor(cx, store, if has_selection { target } else { 0 }, extend);
    }
    true
}

fn type_ahead(cx: &mut Cx, store: Store, character: char) {
    let prefix = store.type_ahead.update(cx, |type_ahead| {
        type_ahead.push(character, Instant::now()).to_owned()
    });
    let found = store.folder.with(cx, |folder| {
        folder
            .iter()
            .position(|entry| starts_with_folded(&entry.name, &prefix))
    });
    if let Some(position) = found {
        move_cursor(cx, store, position, false);
    }
}
