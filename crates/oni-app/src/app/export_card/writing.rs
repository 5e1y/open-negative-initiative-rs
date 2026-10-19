//! Writing: the batch off this thread, its clashes, its progress and its journal.

use super::*;

impl Ctl {
    /// `runExport`: one file asks for its name, a batch for a folder and then for any clash.
    pub fn run_export(self: &Rc<Self>) {
        let (targets, rows, format) = {
            let c = self.export.borrow();
            (c.targets.clone(), c.rows.clone(), c.settings.format())
        };
        if targets.is_empty() || self.export.borrow().exporting {
            return;
        }
        if targets.len() == 1 {
            let typed = rows[0].as_ref().map_or("", |r| r.name.as_str());
            let name = oni_export::name(&targets[0].path, typed, format);
            let Some(destination) = self.ask_paths(&Ask::Save { name, extension: format.file_extension().into() }).into_iter().next() else { return };
            return self.write_export(vec![destination]);
        }
        let Some(folder) = self.ask_paths(&Ask::Folder { count: targets.len() }).into_iter().next() else { return };
        let destinations = export::destinations(&folder, &targets, &rows, format);
        // A folder picker does not ask before overwriting, so the batch checks its clashes itself.
        let clashes = oni_export::clashes(&destinations, |p| p.exists());
        if let Some((title, message)) = export::clash_notice(clashes.len(), destinations.len()) {
            {
                let mut c = self.export.borrow_mut();
                c.confirm = Some(destinations);
                c.notice = None;
            }
            if let Some(w) = self.window() {
                let e = w.global::<Exporting>();
                e.set_confirm_title(title.into());
                e.set_confirm_message(message.into());
            }
            return self.publish_export();
        }
        self.write_export(destinations);
    }

    pub(super) fn confirmed(self: &Rc<Self>, replace: bool) {
        let chosen = self.export.borrow_mut().confirm.take();
        self.publish_export();
        if replace && let Some(destinations) = chosen {
            self.write_export(destinations);
        }
    }

    /// `writeExport`: in order off this thread, the stop honoured between files; the first line's
    /// artist and copyright are remembered for the next photo's empty line.
    pub(super) fn write_export(self: &Rc<Self>, destinations: Vec<PathBuf>) {
        let (plan, export, stop) = {
            let mut c = self.export.borrow_mut();
            c.exporting = true;
            c.progress = 0;
            c.stopped = None;
            c.stop.store(false, Ordering::Relaxed);
            if let Some(Some(first)) = c.rows.first().cloned() {
                c.artist = first.metadata.artist.clone();
                c.copyright = first.metadata.copyright.clone();
                c.save(ARTIST, Value::from(first.metadata.artist));
                c.save(COPYRIGHT, Value::from(first.metadata.copyright));
            }
            let plan: Vec<(Target, PathBuf, ExifMetadata)> = c
                .targets
                .iter()
                .zip(destinations)
                .zip(&c.rows)
                .map(|((t, d), r)| (t.clone(), d, r.as_ref().map(|r| r.metadata.clone()).unwrap_or_default()))
                .collect();
            (plan, c.settings.clone(), c.stop.clone())
        };
        self.publish_export();
        let engine: Shared = self.engine.clone();
        let journal = crate::app::year::journal_in(&self.export.borrow().support);
        std::thread::Builder::new()
            .name("export".into())
            .spawn(move || {
                for (target, destination, metadata) in plan {
                    if stop.load(Ordering::Relaxed) {
                        return with_shell(|c| {
                            {
                                let mut card = c.export.borrow_mut();
                                card.exporting = false;
                                card.stopped = Some(card.progress);
                            }
                            c.publish_export();
                        });
                    }
                    match export::write_one(&engine, &target, &destination, &export, &metadata) {
                        Ok(()) => {
                            let subject = target.path.to_string_lossy().into_owned();
                            journal.note(Gesture::Exported, Some(&subject), 0, 0, None, oni_settings::Date::now());
                            with_shell(|c| {
                                c.export.borrow_mut().progress += 1;
                                c.publish_export();
                            });
                        }
                        Err(e) => {
                            let name = target.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            let message = format!("{name}: {e}");
                            return with_shell(move |c| {
                                {
                                    let mut card = c.export.borrow_mut();
                                    card.exporting = false;
                                    card.notice = Some(("Export Failed".into(), message));
                                }
                                c.publish_export();
                            });
                        }
                    }
                }
                with_shell(|c| {
                    c.export.borrow_mut().exporting = false;
                    c.leave_export();
                });
            })
            .expect("export thread");
    }
}
