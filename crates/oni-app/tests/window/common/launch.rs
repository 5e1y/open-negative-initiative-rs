//! A check's app: its own temporary root and the shell launched on it, with the options it changes.

use super::{Rig, rig};
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A check's own folder under the system's temporary one, emptied at its start and removed when it
/// drops; its scans, support and caches folders are named here.
pub struct Root(PathBuf, String);

impl Root {
    pub fn new(name: &str) -> Root {
        let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        Root(root, name.to_string())
    }

    /// The root made and spelt as the app keeps paths: Windows' temporary folder is a short `RUNNER~1`
    /// name and uses `\`.
    pub fn canonical(mut self) -> Root {
        std::fs::create_dir_all(&self.0).unwrap();
        self.0 = PathBuf::from(oni_library::paths::canonical(&self.0));
        self
    }

    pub fn scans(&self) -> PathBuf {
        self.0.join("Scans")
    }

    pub fn support(&self) -> PathBuf {
        self.0.join("Support")
    }

    pub fn caches(&self) -> PathBuf {
        self.0.join("Caches")
    }
}

impl std::ops::Deref for Root {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Root {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A launched app: its fields drop in order, the shell and its workers before the root that holds their
/// files and the GPU they ran on; a check's clones of `ctl`, made later, drop first.
pub struct App {
    pub shell: oni_app::app::Shell,
    pub rig: Rig,
    pub root: Root,
    gpu: oni_app::render::Device,
}

/// The app a check runs: the shell on an empty archive and its root's support and caches, no folder
/// picker, at 1280 x 800 points and scale 1 unless the check says otherwise.
pub struct Launch {
    root: Root,
    options: oni_app::app::Options,
    size: (u32, u32),
    scale: f32,
}

pub fn launch(root: Root) -> Launch {
    let options = oni_app::app::Options { support: root.support(), caches: root.caches(), picker: Box::new(|| None), ..Default::default() };
    Launch { root, options, size: (1280, 800), scale: 1.0 }
}

impl Launch {
    /// The options changed from the check's defaults.
    pub fn with(mut self, change: impl FnOnce(&mut oni_app::app::Options)) -> Launch {
        change(&mut self.options);
        self
    }

    /// The window's size in points.
    pub fn size(mut self, width: u32, height: u32) -> Launch {
        self.size = (width, height);
        self
    }

    pub fn scale(mut self, scale: f32) -> Launch {
        self.scale = scale;
        self
    }

    /// The window shown; none without a GPU, the check then not exercised.
    pub fn show(self) -> Option<App> {
        use slint::ComponentHandle;
        let gpu = match oni_app::render::Device::open() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("{e}: {} is not exercised", self.root.1);
                return None;
            }
        };
        let rig = rig();
        let store = oni_core::EditorStore::new(oni_core::Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let shell = oni_app::app::Shell::new(store, oni_app::render::Engine::shared(&gpu), self.options).unwrap();
        rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: self.scale });
        let (width, height) = self.size;
        rig.window.set_size(slint::PhysicalSize::new((width as f32 * self.scale) as u32, (height as f32 * self.scale) as u32));
        shell.window.show().unwrap();
        Some(App { shell, rig, root: self.root, gpu })
    }
}

impl App {
    /// The folder opened in the gallery and read.
    pub fn read(&self, folder: &Path) {
        let ctl = &self.shell.ctl;
        self.shell.open_folder(folder);
        self.rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    }

    /// The scans folder read and its first frame double-clicked into the editor, rendered once.
    pub fn edit_first(&self) {
        use slint::ComponentHandle;
        self.read(&self.root.scans());
        let lib = self.shell.window.global::<oni_app::Library>();
        lib.invoke_tile_clicked(0, false, false);
        lib.invoke_tile_clicked(0, false, false);
        let ctl = &self.shell.ctl;
        self.rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    }
}
