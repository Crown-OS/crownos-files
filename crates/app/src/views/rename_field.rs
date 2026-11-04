use crownui::{
    kit::{FieldShape, TextInputOptions, text_input},
    prelude::{Listeners, Semantic, Styled, View},
};

use crate::{
    app::editing::commit,
    state::Store,
    views::focus_slot::{FocusSlotOptions, autofocused},
};

/// The inline editor F2 puts over an entry's name; Enter or leaving it renames.
pub fn rename_field(store: Store) -> impl View {
    autofocused(
        text_input(store.edit_text)
            .shape(FieldShape::Rounded)
            .stretch()
            .label("Rename")
            .on_submit(move |cx, _| commit(cx, store)),
    )
    .on_focus_out(move |cx| commit(cx, store))
    .w_full()
}
