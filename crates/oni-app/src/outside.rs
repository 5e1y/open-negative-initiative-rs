//! The window's input, told to the interface before any item reads it: each press counted, which the
//! item holding the keyboard and the pickers read, and each key, which finds a holder first.

use crate::Outside;
use slint::private_unstable_api::re_exports::WindowInner;
use slint::{ComponentHandle, Global};

/// Counts a press of any button; the window's pointer hook calls it before Slint dispatches it.
pub fn pressed<C>(window: &C)
where
    C: ComponentHandle,
    for<'a> Outside<'a>: Global<'a, C>,
{
    let outside = window.global::<Outside>();
    outside.set_presses(outside.get_presses().wrapping_add(1));
}

/// Before a key: Slint hands it to the item holding the keyboard and drops it once that item was
/// destroyed or hidden, as a key a press removed; `refocus` gives the keyboard to the monitor first.
pub fn keyed(window: &slint::Window, refocus: impl FnOnce()) {
    let inner = WindowInner::from_pub(window);
    let held = inner.focus_item.borrow().upgrade().is_some_and(|item| item.is_visible());
    if !held {
        refocus();
    }
}
