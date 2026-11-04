//! "Delete 3 items permanently?": the confirmation before anything is
//! deleted without the trash.

use crownui::{
    kit::{ButtonOptions, ButtonVariant, button, dialog},
    prelude::{Listeners, Styled, View, component, hstack, spacer, text, vstack},
};

use crate::{app::deleting::confirm_delete, state::Store};

pub fn delete_dialog(store: Store) -> impl View {
    let deletion = store.deletion;
    dialog(deletion.open, move || {
        component(move |cx| {
            let (title, detail, action) = deletion.prompt.with(cx, |prompt| {
                prompt.as_ref().map_or_else(
                    || (String::new(), "", "Delete"),
                    |prompt| (prompt.title.clone(), prompt.detail, prompt.action),
                )
            });
            vstack((
                text(title).font_semibold(),
                text(detail),
                hstack((
                    spacer(),
                    button("Cancel")
                        .variant(ButtonVariant::Secondary)
                        .on_click(move |cx, _| deletion.open.set(cx, false)),
                    button(action)
                        .variant(ButtonVariant::Destructive)
                        .on_click(move |cx, _| confirm_delete(cx, store)),
                ))
                .gap(8.0),
            ))
            .gap(12.0)
        })
    })
}
