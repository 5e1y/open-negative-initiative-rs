//! The header of a canvas tool, Crop's and De-dust's, at the smallest window: its title whole in every
//! language, read as its laid width against the width its words want. Twin: a title longer than the
//! panel is caught cut.

use i_slint_core::item_tree::ItemRc;
use i_slint_core::items::{ComplexText, Item};
use i_slint_core::layout::Orientation;
use oni_app::{HeaderProbe, PanelState, PanelTab};
use oni_i18n::{Language, tr};
use slint::{ComponentHandle, PhysicalSize};
use std::ops::ControlFlow;
use std::path::PathBuf;

use crate::common::{self, Placed};

/// `ShellStyle.min-width` and `min-height`: the smallest window the app opens at.
const SMALLEST: (u32, u32) = (1100, 680);
/// `ShellStyle.panel-width`: the panel stands at the window's right edge.
const PANEL: f32 = 340.0;

/// Every text shown right of `left` reading `text`: where the layout put it, and the width its words
/// want on one line, which a title shows whole only when laid at least as wide.
fn titles(window: &impl ComponentHandle, text: &str, left: f32) -> Vec<(Placed, f32)> {
    let inner = i_slint_core::window::WindowInner::from_pub(window.window());
    let adapter = inner.window_adapter();
    let mut found = Vec::new();
    ItemRc::new(inner.component(), 0).visit_descendants(|item| {
        if let Some(label) = item.downcast::<ComplexText>() {
            let label = label.as_pin_ref();
            let (geometry, at) = (item.geometry(), item.map_to_window(item.geometry().origin));
            if label.text().as_str() == text && item.is_visible() && at.x >= left {
                let wanted = label.layout_info(Orientation::Horizontal, geometry.height(), &adapter, item).preferred;
                found.push((Placed { x: at.x, y: at.y, width: geometry.width(), height: geometry.height() }, wanted));
            }
        }
        ControlFlow::<()>::Continue(())
    });
    found
}

/// Whether the title is laid as wide as its words want, half a point of rounding aside.
fn whole((laid, wanted): (Placed, f32)) -> bool {
    laid.width + 0.5 >= wanted
}

pub(crate) fn every_tool_title_is_whole_at_the_smallest_window() {
    let Some((rig, shell, _root)) = crate::panel_bindings::opened("mode-header") else { return };
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let panel = w.global::<PanelState>();
    rig.window.set_size(PhysicalSize::new(SMALLEST.0, SMALLEST.1));
    rig.settle();
    let left = SMALLEST.0 as f32 - PANEL;
    for language in Language::ALL {
        oni_app::language::speak(Some(language));
        for (act, tab, title, name) in [(oni_app::Act::Framing, PanelTab::Crop, "Crop", "crop"), (oni_app::Act::Correction, PanelTab::Dedust, "De-dust", "dedust")] {
            ctl.act(act);
            rig.settle();
            assert_eq!(panel.get_tab(), tab, "{name} opens");
            println!("shot {}", rig.shot(&format!("header-{}-{name}", language.key())).display());
            let (english, title) = (title, tr(title));
            let found = titles(w, title, left);
            if language != Language::English && found.is_empty() && !titles(w, english, left).is_empty() {
                println!("{language:?}: the live interface reads English, judged on the app profile");
            } else {
                assert_eq!(found.len(), 1, "{language:?}: one title {title:?} in the panel, found {found:?}");
                assert!(whole(found[0]), "{language:?}: {title:?} laid {} wide, its words want {}", found[0].0.width, found[0].1);
            }
            ctl.act(act);
            rig.settle();
        }
    }
    oni_app::language::speak(Some(Language::English));
}

pub(crate) fn a_title_longer_than_the_panel_is_caught_cut() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let probe = HeaderProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    rig.window.set_size(PhysicalSize::new(PANEL as u32, 200));
    probe.show().unwrap();
    let title = tr("De-dust");
    probe.set_name(title.into());
    rig.settle();
    let found = titles(&probe, title, 0.0);
    assert!(found.len() == 1 && whole(found[0]), "the probe's header shows {title:?} whole: {found:?}");
    // Twin: a title wider than the panel is cut wherever the header lays it, and the reading sees it.
    let long = "De-dust ".repeat(8);
    let long = long.trim_end();
    probe.set_name(long.into());
    rig.settle();
    println!("shot {}", rig.shot("header-twin-long").display());
    let found = titles(&probe, long, 0.0);
    assert!(found.len() == 1 && !whole(found[0]), "twin: a title longer than the panel reads cut: {found:?}");
    probe.hide().unwrap();
}
