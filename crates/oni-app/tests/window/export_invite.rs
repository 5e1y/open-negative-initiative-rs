//! An export written ends on its notice with an invitation above it to post the photo with ONI's hashtag:
//! its keys show under its line, Don't Show Again hides it for good, Settings brings it back.

use oni_app::app::export_card::{self, job::End};
use oni_app::app::{Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{Act, AppSettings, Exporting, Told};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

const LONG: Duration = Duration::from_secs(300);

pub(crate) fn an_export_written_invites_to_share_until_told_not_to() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export's invitation is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-export-invite-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scan, out, support) = (root.join("Scans").join("A.tif"), root.join("Out"), root.join("Support"));
    std::fs::create_dir_all(&out).unwrap();
    common::linear_tiff(&scan, 96, 64, 0.5);
    let rig = common::rig();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { canvas: false, support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (e, told) = (w.global::<Exporting>(), w.global::<Told>());
    // Each export into a folder of its own, so none asks to replace the last one's file.
    let round = Rc::new(std::cell::Cell::new(0));
    let (answer, turn) = (out.clone(), round.clone());
    export_card::answer_with(&ctl, Rc::new(move |ask| {
        let folder = answer.join(turn.get().to_string());
        std::fs::create_dir_all(&folder).unwrap();
        match ask {
            export_card::Ask::Save { name, extension } => vec![folder.join(format!("{name}.{extension}"))],
            _ => vec![folder],
        }
    }));
    let export = || {
        round.set(round.get() + 1);
        ctl.enter_export(vec![oni_app::export::Target { path: scan.clone(), settings: Default::default() }]);
        rig.until("the line", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
        e.invoke_export();
        rig.until("the file written", LONG, || export_card::job::job(&ctl).is_some_and(|j| j.2.is_some()));
        assert_eq!(export_card::job::job(&ctl).and_then(|j| j.2), Some(End::Written));
        rig.settle();
    };
    let line = "Post your photos on Instagram with #madewithoni, we'd love to see them.";

    // The notice says the photo exported, the invitation above it, its keys under its line.
    export();
    assert!(told.get_shown() && told.get_invite(), "an export written invites to share");
    assert_eq!(told.get_tag(), "#madewithoni");
    assert_eq!(common::placed(w, Role::Text, line).len(), 1, "the line shows above the notice");
    for key in ["Copy", "Open Instagram", "Don't Show Again"] {
        let low = common::placed(w, Role::Button, key).into_iter().filter(|k| k.y > 400.0).count();
        assert_eq!(low, 1, "{key} shows in the notice");
    }
    rig.shot("export-invite");

    // Don't Show Again hides it at once and for good, written to the preferences.
    told.invoke_hide_invite();
    rig.settle();
    assert!(!told.get_invite() && common::placed(w, Role::Text, line).is_empty(), "hidden at once");
    assert!(!Preferences::load(&support).export_invite(), "the choice is kept");
    assert!(!w.global::<AppSettings>().get_share_invite(), "Settings shows it off");
    told.invoke_closed();
    export();
    assert!(told.get_shown() && !told.get_invite(), "twin: the next export tells its notice alone");

    // Settings brings it back.
    ctl.act(Act::ShareInvite);
    assert!(Preferences::load(&support).export_invite() && w.global::<AppSettings>().get_share_invite());
    told.invoke_closed();
    export();
    assert!(told.get_invite(), "back on, the next export invites again");
    let _ = std::fs::remove_dir_all(&root);
}
