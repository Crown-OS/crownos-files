//! "“name” already exists": Replace, Skip or Keep Both, for one or all.

use crownui::{
    kit::{ButtonOptions, ButtonVariant, ToggleOptions, button, dialog, toggle},
    prelude::{Listeners, Runtime, Styled, View, bind, hstack, spacer, text},
};

use crate::{app::conflicts::answer, jobs::ConflictPolicy, state::Store};

pub fn conflict_dialog(store: Store) -> impl View {
    let conflicts = store.conflicts;
    let title = move |runtime: &mut Runtime| {
        conflicts.prompt.with(runtime, |prompt| {
            prompt
                .as_ref()
                .map(|prompt| format!("“{}” already exists", prompt.name))
                .unwrap_or_default()
        })
    };
    let body = move |runtime: &mut Runtime| {
        conflicts.prompt.with(runtime, |prompt| {
            prompt
                .as_ref()
                .map(|prompt| {
                    format!(
                        "“{}” already has an item with this name. Replacing it overwrites it.",
                        prompt.folder
                    )
                })
                .unwrap_or_default()
        })
    };
    let choice = move |label: &'static str, variant: ButtonVariant, policy: ConflictPolicy| {
        button(label)
            .variant(variant)
            .on_click(move |cx, _| answer(cx, store, policy))
    };
    dialog(conflicts.open, move || {
        (
            text(bind(title)).font_semibold(),
            text(bind(body)),
            toggle(conflicts.apply_to_all).caption("Do this for every conflict"),
            hstack((
                spacer(),
                choice("Skip", ButtonVariant::Secondary, ConflictPolicy::Skip),
                choice(
                    "Keep Both",
                    ButtonVariant::Secondary,
                    ConflictPolicy::KeepBoth,
                ),
                choice(
                    "Replace",
                    ButtonVariant::Destructive,
                    ConflictPolicy::Replace,
                ),
            ))
            .gap(8.0),
        )
    })
}
