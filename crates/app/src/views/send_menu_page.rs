//! The context menu's "Send to" page: the paired devices reachable now.

use crownui::{
    kit::{MenuRowOptions, menu_header, menu_item, menu_item_disabled},
    prelude::{AnyView, Listeners, View},
};

use crate::{
    app::peers::{Peer, send_selection},
    state::Store,
    views::file_menu::back_item,
};

pub fn send_items(store: Store, peers: Vec<Peer>) -> Vec<AnyView> {
    let mut items = vec![back_item(store), menu_header("Send to").boxed()];
    if peers.is_empty() {
        items.push(menu_item_disabled("No device is connected").boxed());
    }
    items.extend(peers.into_iter().map(|peer| {
        menu_item(peer.name.clone())
            .leading_icon(peer.kind.icon())
            .on_click(move |cx, _| send_selection(cx, store, &peer))
            .boxed()
    }));
    items
}
