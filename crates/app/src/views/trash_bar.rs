//! The strip above the Trash: restore what is selected, or empty it all.

use crownui::{
    kit::{ButtonOptions, ButtonSize, ButtonVariant, button, use_theme},
    prelude::{
        Cx, Listeners, Runtime, Semantic, Styled, View, bind, hstack, show, spacer, text, vstack,
    },
};

use crate::{
    app::{
        deleting::{empty_trash, restore_selection},
        virtual_folder::VirtualFolder,
    },
    state::Store,
    util::theme::tint,
};

const HEIGHT: f32 = 40.0;

pub fn trash_bar(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let in_trash = move |runtime: &mut Runtime| {
        store.cwd.with(runtime, |dir| {
            VirtualFolder::of(dir) == Some(VirtualFolder::Trash)
        })
    };
    let bar = move || {
        let actions = hstack((
            text("Items in the Trash are deleted for good when it is emptied")
                .text_size(12.0)
                .truncate()
                .min_w(0.0)
                .text_color(tint(theme, |theme| theme.text.muted)),
            spacer(),
            button("Restore")
                .variant(ButtonVariant::Secondary)
                .control_size(ButtonSize::Small)
                .disabled(bind(move |runtime: &mut Runtime| {
                    store.selection_rev.get(runtime);
                    store
                        .folder
                        .with(runtime, |folder| folder.selection().is_empty())
                }))
                .on_click(move |cx, _| restore_selection(cx, store)),
            button("Empty Trash")
                .variant(ButtonVariant::Destructive)
                .control_size(ButtonSize::Small)
                .disabled(bind(move |runtime: &mut Runtime| {
                    store.count.get(runtime) == 0
                }))
                .on_click(move |cx, _| empty_trash(cx, store)),
        ))
        .items_center()
        .gap(8.0)
        .h(HEIGHT)
        .shrink(0.0)
        .px(12.0);
        let hairline = vstack(())
            .h(1.0)
            .shrink(0.0)
            .bg(tint(theme, |theme| theme.surface.hairline));
        vstack((actions, hairline))
    };
    vstack(show(bind(in_trash), bar, || hstack(())))
        .w_full()
        .shrink(0.0)
}
