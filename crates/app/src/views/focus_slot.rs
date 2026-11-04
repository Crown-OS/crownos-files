//! Reaching a text field the kit builds inside a widget: focusing it from a
//! shortcut and knowing whether it has focus.

use crownui::{
    ext::{Kind, Listener, NodeId},
    prelude::{Cx, Element, Runtime, Signal, Store, View},
};

/// Where one focusable view is, once mounted, and whether it has focus.
#[derive(Debug, Clone, Copy)]
pub struct FocusSlot {
    node: Store<Option<NodeId>>,
    focused: Signal<bool>,
}

impl FocusSlot {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            node: cx.store(None),
            focused: cx.signal(false),
        }
    }

    /// Moves keyboard focus into the slot's view, if it is mounted.
    pub fn focus(self, cx: &mut Cx) {
        if let Some(node) = self.node.with(cx, |node| *node) {
            cx.request_focus(node);
        }
    }

    pub fn is_focused(self, runtime: &mut Runtime) -> bool {
        self.focused.get(runtime)
    }
}

type BlurHandler = Box<dyn FnMut(&mut Cx)>;

/// The kind behind [`focus_slot`].
pub struct FocusSlotKind<V> {
    content: V,
    slot: Option<FocusSlot>,
    autofocus: bool,
    on_blur: Option<BlurHandler>,
}

impl<V: View> Kind for FocusSlotKind<V> {
    const NAME: &'static str = "focus_slot";

    fn mount(self, cx: &mut Cx, node: NodeId) {
        let Self {
            content,
            slot,
            autofocus,
            on_blur,
        } = self;
        let child = cx.build(content);
        cx.adopt_children(node, vec![child]);
        let Some(target) = first_focusable(cx, child) else {
            return;
        };
        if let Some(slot) = slot {
            track(cx, slot, target);
        }
        if let Some(mut on_blur) = on_blur {
            cx.add_listener(target, Listener::Blur(Box::new(move |cx, _| on_blur(cx))));
        }
        if autofocus {
            cx.request_focus(target);
        }
    }
}

fn track(cx: &mut Cx, slot: FocusSlot, target: NodeId) {
    slot.node.update(cx, |node| *node = Some(target));
    cx.on_cleanup(move |runtime| {
        slot.node.update(runtime, |node| *node = None);
        if slot.focused.with_untracked(runtime, |focused| *focused) {
            slot.focused.set(runtime, false);
        }
    });
    cx.add_listener(
        target,
        Listener::Focus(Box::new(move |cx, _| slot.focused.set(cx, true))),
    );
    cx.add_listener(
        target,
        Listener::Blur(Box::new(move |cx, _| slot.focused.set(cx, false))),
    );
}

fn first_focusable(cx: &Cx, node: NodeId) -> Option<NodeId> {
    let element = cx.tree().get(node)?;
    if element.is_focusable() {
        return Some(node);
    }
    element
        .children()
        .iter()
        .find_map(|&child| first_focusable(cx, child))
}

/// `content`, whose first focusable view `slot` reaches.
pub fn focus_slot<V: View>(content: V, slot: FocusSlot) -> Element<FocusSlotKind<V>> {
    Element::new(FocusSlotKind {
        content,
        slot: Some(slot),
        autofocus: false,
        on_blur: None,
    })
}

/// `content`, focused as soon as it mounts, as an inline editor wants.
pub fn autofocused<V: View>(content: V) -> Element<FocusSlotKind<V>> {
    Element::new(FocusSlotKind {
        content,
        slot: None,
        autofocus: true,
        on_blur: None,
    })
}

/// The options of a [`focus_slot`], chained before its modifiers.
pub trait FocusSlotOptions: Sized {
    /// Runs when the view loses focus.
    fn on_focus_out(self, handler: impl FnMut(&mut Cx) + 'static) -> Self;
}

impl<V: View> FocusSlotOptions for Element<FocusSlotKind<V>> {
    fn on_focus_out(self, handler: impl FnMut(&mut Cx) + 'static) -> Self {
        self.configure(|kind| kind.on_blur = Some(Box::new(handler)))
    }
}
