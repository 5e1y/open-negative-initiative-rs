//! A disk of photos pulled out and plugged back, a disk image standing for it: the open photo says its disk is away and
//! keeps its edits, written as soon as the disk is back; the gallery says the disk is not connected; a photo opened
//! while away says so, never that its disk is here, and opens by itself once the disk is back.

use oni_app::{Absent, Canvas, Library};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::common;

/// A disk image of its own, attached under `/Volumes`, detached however the check ends.
struct Disk {
    image: PathBuf,
    volume: String,
}

impl Disk {
    fn new(root: &Path) -> Option<Disk> {
        let volume = format!("ONIUNPLUG{}", std::process::id());
        let image = root.join("photos.dmg");
        let made = Command::new("hdiutil").args(["create", "-quiet", "-size", "40m", "-fs", "APFS", "-volname", &volume]).arg(&image).status().ok()?;
        let disk = Disk { image, volume };
        (made.success() && disk.plug()).then_some(disk)
    }

    fn at(&self) -> PathBuf {
        Path::new("/Volumes").join(&self.volume)
    }

    fn plug(&self) -> bool {
        Command::new("hdiutil").args(["attach", "-quiet", "-nobrowse"]).arg(&self.image).status().is_ok_and(|s| s.success()) && self.at().is_dir()
    }

    fn pull(&self) {
        let pulled = Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(self.at()).status();
        assert!(pulled.is_ok_and(|s| s.success()) && !self.at().exists(), "the disk image detaches");
    }
}

impl Drop for Disk {
    fn drop(&mut self) {
        if self.at().exists() {
            let _ = Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(self.at()).status();
        }
    }
}

/// The exposure a sidecar beside `photo` holds, none while it cannot be read.
fn stored_exposure(photo: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(format!("{}.json", photo.display())).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value["settings"]["print"]["exposure"].as_f64()
}

pub(crate) fn a_photo_disk_pulled_out_is_said_away_and_its_edits_land_once_it_is_back() {
    if !cfg!(target_os = "macos") {
        return println!("SKIPPED a_photo_disk_pulled_out_is_said_away_and_its_edits_land_once_it_is_back: hdiutil is macOS's");
    }
    let root = common::Root::new("unplugged").canonical();
    let Some(disk) = Disk::new(&root) else {
        return println!("SKIPPED a_photo_disk_pulled_out_is_said_away_and_its_edits_land_once_it_is_back: no disk image attaches");
    };
    let folder = disk.at().join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6)] {
        common::graded_tiff(&folder.join(name), 600, 400, tone);
    }
    let (a, b) = (folder.join("A.tif"), folder.join("B.tif"));
    let home = root.join("home").to_string_lossy().into_owned();
    let computer = oni_library::navigator::Computer { home, volumes: "/Volumes".into() };
    let Some(app) = common::launch(root).with(|o| o.computer = computer).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (canvas, absent, lib) = (w.global::<Canvas>(), w.global::<Absent>(), w.global::<Library>());
    let status = || canvas.get_status().to_string();
    let away = format!("Its disk “{}” is not connected", disk.volume);
    app.read(&folder);
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(60), || ctl.state.borrow().renders() >= 1);
    assert!(!status().contains(&away), "twin: the disk there, nothing says it is away");

    // Pulled under the editor: said at once, the photo still shown and its edit kept.
    disk.pull();
    rig.until("the disk said away", Duration::from_secs(10), || status().starts_with(&away));
    let step = w.global::<oni_app::Store>();
    step.invoke_step(oni_app::fields::Field::Exposure.index(), 5);
    rig.settle();
    assert!(status().starts_with(&away), "an edit with the disk away says why it waits: {}", status());
    assert!(!status().contains("may be disconnected") && !status().contains("Nothing was saved"), "{}", status());
    assert_eq!(stored_exposure(&a), None, "nothing written while the disk is away");

    // Plugged back on the same photo: its edit written with no other gesture.
    assert!(disk.plug(), "the disk image attaches again");
    rig.until("A's edit written", Duration::from_secs(10), || stored_exposure(&a).is_some_and(|e| (e - 0.05).abs() < 1e-6));
    assert_eq!(status(), "The kept edits are saved.");

    // Pulled again: the gallery says the disk is not connected, and B opened says so too.
    disk.pull();
    ctl.act(oni_app::Act::ToggleMode);
    rig.until("the gallery's banner", Duration::from_secs(10), || lib.get_banner_shown());
    assert_eq!(lib.get_banner_title().as_str(), format!("{} Not Connected", disk.volume));
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    rig.until("B opened", Duration::from_secs(10), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("B.tif")));
    rig.settle();
    assert_eq!(absent.get_title().as_str(), "“B.tif” cannot be found");
    assert!(absent.get_detail().starts_with(&away), "{}", absent.get_detail());
    assert!(!absent.get_detail().contains("its disk is here"), "twin: never the words of a file gone from a disk there");

    // Back: B opens by itself, its card gone, and renders.
    let renders = ctl.state.borrow().renders();
    assert!(disk.plug(), "the disk image attaches again");
    rig.until("B's card gone", Duration::from_secs(10), || absent.get_title().is_empty());
    rig.until("B rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert!(b.is_file());
}
