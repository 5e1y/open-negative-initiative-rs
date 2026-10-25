//! A press in the window, told to the interface before any item reads it: a focused field it lands
//! beside ends its editing, and a picker whose release comes without it opens.

use crate::Outside;
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
