//! "Unlock “name”": the passphrase of an encrypted drive.

use crownui::{
    kit::{ButtonOptions, ButtonVariant, TextInputOptions, button, dialog, text_input, use_theme},
    prelude::{
        Listeners, Runtime, Styled, View, bind, component, hstack, show, spacer, text, vstack,
    },
};

use crate::{
    app::unlock::{cancel_unlock, submit_passphrase},
    state::Store,
    util::theme::tint,
};

const WIDTH: f32 = 340.0;

pub fn passphrase_dialog(store: Store) -> impl View {
    let unlock = store.drives.unlock;
    dialog(unlock.open, move || {
        component(move |cx| {
            let theme = use_theme(cx);
            let title = bind(move |runtime: &mut Runtime| {
                unlock.name.with(runtime, |name| format!("Unlock “{name}”"))
            });
            let error = move || {
                text(bind(move |runtime: &mut Runtime| {
                    unlock.error.get(runtime).unwrap_or_default()
                }))
                .text_size(12.0)
                .text_color(tint(theme, |theme| theme.status.danger))
            };
            vstack((
                text(title).font_semibold().truncate(),
                text("Enter the passphrase of this encrypted drive.")
                    .text_size(12.0)
                    .text_color(tint(theme, |theme| theme.text.muted)),
                text_input(unlock.passphrase)
                    .secret()
                    .placeholder("Passphrase")
                    .stretch()
                    .on_submit(move |cx, _| submit_passphrase(cx, store)),
                show(
                    bind(move |runtime: &mut Runtime| unlock.error.with(runtime, Option::is_some)),
                    error,
                    || vstack(()),
                ),
                hstack((
                    spacer(),
                    button("Cancel")
                        .variant(ButtonVariant::Secondary)
                        .on_click(move |cx, _| cancel_unlock(cx, store)),
                    button(bind(move |runtime: &mut Runtime| {
                        let label = if unlock.waiting.get(runtime) {
                            "Unlocking…"
                        } else {
                            "Unlock"
                        };
                        label.to_owned()
                    }))
                    .variant(ButtonVariant::Primary)
                    .on_click(move |cx, _| submit_passphrase(cx, store)),
                ))
                .gap(8.0),
            ))
            .gap(12.0)
            .w(WIDTH)
        })
    })
}
