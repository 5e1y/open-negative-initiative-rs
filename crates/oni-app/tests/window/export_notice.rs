//! The export's notice carries its invitation in a card of its own above it: the hashtag, its line and its keys at the
//! notice's size in one row, the result's Dismiss and Show in Finder at the foot of its own card; the invitation leaves
//! by itself first, the result staying until its own time.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::Told;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common::{self, Placed};
use crate::panel_bindings::opened;

/// The notice's key of that name, the lowest of its name: the bar above may hold one too, as Copy.
fn key(w: &oni_app::MainWindow, text: &str) -> Option<Placed> {
    common::placed(w, Role::Button, text).into_iter().max_by(|a, b| a.y.total_cmp(&b.y))
}

/// `SheetStyle.linger`: the result's time; the invitation leaves at half of it.
const LINGER: Duration = Duration::from_secs(8);

pub(crate) fn the_export_notice_carries_its_invitation_in_a_card_that_leaves_first() {
    let Some(app) = opened("export-notice") else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.settle();
    let told = w.global::<Told>();
    rig.freeze();
    told.set_title("Export".into());
    told.set_message("1 photo exported".into());
    told.set_action("Show in Finder".into());
    told.set_invite(true);
    told.set_serial(told.get_serial() + 1);
    told.set_shown(true);
    rig.advance(Duration::from_millis(400));
    rig.pump();
    let (dismiss, finder) = (key(w, "Dismiss").expect("Dismiss"), key(w, "Show in Finder").expect("Show in Finder"));
    let invite: Vec<Placed> = ["Copy", "Open Instagram", "Don't Show Again"].iter().map(|t| key(w, t).unwrap_or_else(|| panic!("{t}"))).collect();
    let tag = common::placed(w, Role::Text, "#madewithoni");
    println!("Dismiss {dismiss:?}, Show in Finder {finder:?}, the invitation's {invite:?}, its tag {tag:?}");
    assert!(invite.iter().chain([&finder]).all(|k| (k.height - dismiss.height).abs() < 0.5), "every key at one size");
    assert!(invite.iter().all(|k| (k.y - invite[0].y).abs() < 0.5) && (dismiss.y - finder.y).abs() < 0.5, "each card's keys in one row");
    assert!(tag.len() == 1 && tag[0].y < invite[0].y && invite[0].y + invite[0].height < dismiss.y, "the invitation above the result");
    assert!(finder.x + finder.width <= 1280.0 && finder.y + finder.height <= 800.0, "inside the window");
    println!("shot {}", rig.shot("export-notice").display());
    // The invitation leaves by itself at half the result's time; the result stays, its keys where they were.
    rig.advance(LINGER / 2 - Duration::from_millis(500));
    rig.pump();
    assert!(key(w, "Open Instagram").is_some(), "twin: the invitation still shows a moment before its time");
    rig.advance(Duration::from_millis(600));
    rig.pump();
    assert!(key(w, "Open Instagram").is_none() && common::placed(w, Role::Text, "#madewithoni").is_empty(), "the invitation left");
    let stayed = key(w, "Dismiss").expect("the result stays");
    assert!((stayed.y - dismiss.y).abs() < 0.5, "the result's keys unmoved: {stayed:?} against {dismiss:?}");
    rig.advance(LINGER);
    rig.pump();
    rig.advance(Duration::from_secs(1));
    rig.pump();
    assert!(!told.get_shown() || key(w, "Dismiss").is_none(), "the result left at its own time");
    rig.thaw();
}
