//! The context menu's "Tags" page: every offered tag, checked when the whole
//! selection has it; picking one adds it to all or removes it from all.

use crownui::{
    kit::{MenuRowOptions, icons, menu_header, menu_item, menu_item_disabled},
    prelude::{AnyView, Listeners, View},
};

use crate::{app::tagging::toggle_selection_tag, state::Store, views::file_menu::back_item};

pub fn tag_items(store: Store, tags: Vec<(String, bool)>) -> Vec<AnyView> {
    let mut items = vec![back_item(store), menu_header("Tags").boxed()];
    if tags.is_empty() {
        items.push(menu_item_disabled("No tags are set up").boxed());
    }
    items.extend(tags.into_iter().map(|(tag, everywhere)| {
        let row = menu_item(tag.clone());
        let row = if everywhere {
            row.leading_icon(icons::CHECK)
        } else {
            row
        };
        row.on_click(move |cx, _| toggle_selection_tag(cx, store, &tag))
            .boxed()
    }));
    items
}
