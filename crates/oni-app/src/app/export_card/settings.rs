//! The export's settings: format, space, size, metadata, and the sheets they open.

use super::*;

impl Ctl {
    pub(super) fn export_changed(self: &Rc<Self>, weigh: bool) {
        self.export.borrow_mut().save_settings();
        self.publish_export();
        self.publish_lines();
        if weigh {
            self.weigh_export();
        }
    }

    /// A new space reaches the canvas, the thumbnails to come and every preview, which render again.
    pub(super) fn space_changed(self: &Rc<Self>) {
        {
            let c = self.export.borrow();
            if let Ok(mut w) = c.wanted.lock() {
                *w = Proof { colour_only: c.colour_only, ..Proof::new(c.settings.space().clone()) };
            }
        }
        self.export_changed(true);
        self.reproof();
        let shown = self.export.borrow().shown;
        if shown {
            let targets = {
                let mut c = self.export.borrow_mut();
                c.previews = vec![Preview::Waiting; c.targets.len()];
                c.targets.clone()
            };
            self.publish_lines();
            let (generation, pass) = {
                let c = self.export.borrow();
                (c.pass.fetch_add(1, Ordering::Relaxed) + 1, c.pass.clone())
            };
            self.render_previews(targets, generation, pass);
        }
    }

    /// The canvas asked again under the new proof, its settings unchanged.
    pub(super) fn reproof(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.requested = u64::MAX;
            s.requested_live = None;
        }
        self.request_render();
    }

    pub(super) fn set_space(self: &Rc<Self>, space: Space) {
        {
            let mut c = self.export.borrow_mut();
            c.pending = false;
            if *c.settings.space() == space {
                drop(c);
                return self.publish_export();
            }
            c.settings.set_space(space);
        }
        self.space_changed();
    }

    /// `ExportSpaceField.apply`: Custom lands on the last profile, else the first usable, else waits.
    pub(super) fn space_tab(self: &Rc<Self>, tab: usize) {
        if let Some(space) = Space::BUILT_IN.get(tab) {
            return self.set_space(space.clone());
        }
        let chosen = {
            let mut c = self.export.borrow_mut();
            if !c.last_profile.is_empty() && OutputProfile::is_printer_at(Path::new(&c.last_profile)) {
                Some(OutputProfile::new(c.last_profile.clone()))
            } else if let Some(first) = c.installed.iter().find(|p| p.is_usable()).cloned() {
                c.last_profile = first.path.clone();
                let path = first.path.clone();
                c.save(LAST_PROFILE, Value::from(path));
                Some(first)
            } else {
                c.pending = true;
                None
            }
        };
        match chosen {
            Some(p) => self.set_space(Space::Profile(p)),
            None => self.publish_export(),
        }
    }

    pub fn choose_profile(self: &Rc<Self>, profile: OutputProfile) {
        {
            let mut c = self.export.borrow_mut();
            c.last_profile = profile.path.clone();
            c.save(LAST_PROFILE, Value::from(profile.path.clone()));
        }
        self.set_space(Space::Profile(profile));
    }

    pub(super) fn ask_paths(&self, what: &Ask) -> Vec<PathBuf> {
        let chooser = self.export.borrow().chooser.clone();
        if let Some(chooser) = chooser {
            return chooser(what);
        }
        match what {
            Ask::Save { name, extension } => crate::system::save_file("Export", name, &[extension.as_str()]).into_iter().collect(),
            Ask::Folder { count } => crate::system::pick_folder(&format!("Where should the {count} files be written?")).into_iter().collect(),
            Ask::Profiles => crate::system::pick_files("Choose ICC profiles, or the zip a paper maker or a lab ships them in.", &["icc", "icm", "zip"]),
        }
    }

    /// `loadProfile`: the files installed into the user's profiles, the first chosen, the rest reported.
    pub fn load_profile(self: &Rc<Self>) {
        let paths = self.ask_paths(&Ask::Profiles);
        if paths.is_empty() {
            return;
        }
        let (folder, scratch) = {
            let c = self.export.borrow();
            (c.profiles.clone(), c.support.join("Scratch"))
        };
        let (installed, refused) = OutputProfile::importing(&paths, &folder, &scratch);
        let list = OutputProfile::installed(&OutputProfile::roots(&folder), true);
        self.export.borrow_mut().installed = list;
        if let Some(first) = installed.first() {
            self.choose_profile(first.clone());
        }
        if let Some(notice) = export::import_report(installed.len(), refused) {
            self.export.borrow_mut().notice = Some(notice);
        }
        self.publish_export();
    }

    /// The profiles installed, walked off this thread at launch, as the field's task walks them.
    pub(in crate::app) fn list_profiles(&self) {
        let folder = self.export.borrow().profiles.clone();
        std::thread::spawn(move || {
            let list = OutputProfile::installed(&OutputProfile::roots(&folder), true);
            with_shell(move |c| {
                c.export.borrow_mut().installed = list;
                c.publish_export();
            });
        });
    }

    pub(super) fn set_colour_only(self: &Rc<Self>, on: bool) {
        {
            let mut c = self.export.borrow_mut();
            c.colour_only = on;
            c.save(PROOF_COLOUR_ONLY, Value::from(on));
        }
        self.space_changed();
    }
}
