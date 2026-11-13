//! The world a film is shot in: a folder of its own made for the run, the photos copied in under
//! neutral names, its own support and caches, a navigator rooted in it, so no name of the machine shows.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use oni_app::app::Options;
use oni_i18n::Language;
use oni_library::navigator::Computer;
use oni_library::paths;
use oni_settings::PipelineSettings;
use oni_settings::sidecar::{self, WriteOptions};

/// The folders the navigator shows above the roll: the home's name, then the roll's path under it.
pub const HOME: &str = "Home";
pub const ROLL: [&str; 2] = ["Scans", "Roll 1"];

/// A world on the disk, removed with it; the originals it was made from are only read.
pub struct World {
    pub root: PathBuf,
    pub home: PathBuf,
    /// The folder the photos are copied into, which the app opens.
    pub roll: PathBuf,
    pub support: PathBuf,
    pub caches: PathBuf,
}

impl World {
    /// An empty world under the system's temporary folder, named `name` and the process.
    pub fn new(name: &str) -> Result<World, String> {
        let root = std::env::temp_dir().join(format!("oni-demos-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join(HOME);
        let roll = ROLL.iter().fold(home.clone(), |p, n| p.join(n));
        let (support, caches) = (root.join("Support"), root.join("Caches"));
        for dir in [&roll, &root.join("Volumes"), &support, &caches] {
            std::fs::create_dir_all(dir).map_err(|e| format!("the world's folders: {e}"))?;
        }
        // Read wherever the app asks the process's folders rather than its options: the patches, the decodes kept.
        // SAFETY: set before the shell or any of its threads starts; nothing else of the process reads them.
        unsafe {
            std::env::set_var("ONI_SUPPORT", &support);
            std::env::set_var("ONI_CACHES", &caches);
        }
        // The roll is the folder last opened, as the launch finds it; a notice a person answers once
        // is answered: the film shows the app as it is used.
        use oni_app::preferences::{NEUTRAL_DECODE_NOTICE_SHOWN, OPENED_FOLDER, Preferences};
        let mut preferences = Preferences::load(&support);
        let kept = [(OPENED_FOLDER, serde_json::Value::String(paths::canonical(&roll))), (NEUTRAL_DECODE_NOTICE_SHOWN, serde_json::Value::Bool(true))];
        for (key, value) in kept {
            preferences.set(key, value).map_err(|e| format!("the world's preferences: {e}"))?;
        }
        Ok(World { root, home, roll, support, caches })
    }

    /// `source` copied into the roll as `name`, its extension kept, with a sidecar of `settings` when
    /// given; the copy is the app's to write beside, the source never.
    pub fn add(&self, source: &Path, name: &str, settings: Option<&PipelineSettings>) -> Result<PathBuf, String> {
        let extension = source.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let copy = self.roll.join(format!("{name}.{extension}"));
        std::fs::copy(source, &copy).map_err(|e| format!("a photo copied into the world ({name}): {e}"))?;
        if let Some(s) = settings {
            sidecar::write(s, &copy, WriteOptions::default()).map_err(|e| format!("{name}'s sidecar: {e:?}"))?;
        }
        Ok(copy)
    }

    /// The language the app speaks, as Settings > General chooses it.
    pub fn speak(&self, language: Language) -> Result<(), String> {
        let mut preferences = oni_app::preferences::Preferences::load(&self.support);
        let key = serde_json::Value::String(language.key().into());
        preferences.set(oni_app::preferences::LANGUAGE, key).map_err(|e| format!("the world's preferences: {e}"))
    }

    /// The home and an empty volumes' folder: Computer lists this home alone.
    pub fn computer(&self) -> Computer {
        Computer { home: paths::canonical(&self.home), volumes: paths::canonical(&self.root.join("Volumes")) }
    }

    /// The app's own options on this world: the canvas texture shared with Slint, the window's colour
    /// space, no picker, no update feed, no phone, no volume named.
    pub fn options(&self) -> Options {
        Options {
            screen: oni_app::display::WINDOW,
            canvas: true,
            support: self.support.clone(),
            caches: self.caches.clone(),
            picker: Box::new(|| None),
            mask_picker: Box::new(|| None),
            show_folder: Box::new(|_| {}),
            computer: self.computer(),
            require_roll: true,
            updates: None,
            identify: Arc::new(|_| None),
            phone: false,
            ..Options::default()
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
