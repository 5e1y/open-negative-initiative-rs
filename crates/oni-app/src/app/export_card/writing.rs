//! Writing: the batch off this thread, its clashes, its progress and its journal.

use super::*;

impl Ctl {
    /// `runExport`: one file asks for its name, a batch for a folder; then any clash asks, Replace
    /// offered for ONI's earlier exports alone, Rename for anything else.
    pub fn run_export(self: &Rc<Self>) {
        self.rows_now();
        let (targets, rows, format, support) = {
            let c = self.export.borrow();
            (c.targets.clone(), c.rows.clone(), c.settings.format(), c.support.clone())
        };
        if targets.is_empty() || self.export.borrow().exporting {
            return;
        }
        let single = targets.len() == 1;
        let destinations = if single {
            let typed = rows[0].as_ref().map_or("", |r| r.name.as_str());
            let name = oni_export::name(&targets[0].path, typed, format);
            let Some(destination) = self.ask_paths(&Ask::Save { name, extension: format.file_extension().into() }).into_iter().next() else { return };
            vec![destination]
        } else {
            let Some(folder) = self.ask_paths(&Ask::Folder { count: targets.len() }).into_iter().next() else { return };
            export::clash::destinations(&folder, &targets, &rows, format)
        };
        let sources: Vec<PathBuf> = targets.iter().map(|t| t.path.clone()).collect();
        let plan = oni_export::landing::plan(&destinations, &sources, &oni_export::file::Ledger::in_support(&support));
        match export::clash::notice(&plan) {
            // The save panel has asked already before an earlier export is replaced.
            Some(notice) if single && notice.replaces => self.write_export(notice.destinations, true),
            Some(notice) => {
                {
                    let mut c = self.export.borrow_mut();
                    c.confirm = Some((notice.destinations, notice.replaces));
                    c.notice = None;
                }
                if let Some(w) = self.window() {
                    let e = w.global::<Exporting>();
                    e.set_confirm_title(notice.title.into());
                    e.set_confirm_message(notice.message.into());
                    e.set_confirm_replaces(notice.replaces);
                }
                self.publish_export()
            }
            None => self.write_export(destinations, false),
        }
    }

    pub(super) fn confirmed(self: &Rc<Self>, go: bool) {
        let chosen = self.export.borrow_mut().confirm.take();
        self.publish_export();
        if go && let Some((destinations, replacing)) = chosen {
            self.write_export(destinations, replacing);
        }
    }

    /// `writeExport`: in order off this thread on an engine apart, the stop honoured between files, an
    /// earlier export replaced only when `replacing`; the first line's artist and copyright are remembered.
    pub(super) fn write_export(self: &Rc<Self>, destinations: Vec<PathBuf>, replacing: bool) {
        let started = destinations.clone();
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
        self.job_started(&started);
        let engine: Shared = self.export_engine();
        let support = self.export.borrow().support.clone();
        let journal = crate::app::year::journal_in(&support);
        std::thread::Builder::new()
            .name("export".into())
            .spawn(move || {
                let mut ledger = oni_export::file::Ledger::in_support(&support);
                let jobs: Vec<export::batch::Job> = plan.into_iter().map(|(target, destination, metadata)| export::batch::Job { target, destination, metadata }).collect();
                let (end, _) = export::batch::write_batch(&engine, &jobs, &export, &mut ledger, replacing, &stop, |i| {
                    let subject = oni_library::paths::canonical(&jobs[i].target.path);
                    journal.note(Gesture::Exported, Some(&subject), 0, 0, None, oni_settings::Date::now());
                    with_shell(|c| {
                        c.export.borrow_mut().progress += 1;
                        c.publish_export();
                    });
                });
                match end {
                    export::batch::End::Done => {}
                    export::batch::End::Stopped => return with_shell(|c| c.export_ended(job::End::Stopped)),
                    export::batch::End::Failed(i, e) => {
                        let name = jobs[i].target.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let message = format!("{name}: {e}");
                        return with_shell(move |c| c.export_ended(job::End::Failed(message)));
                    }
                }
                with_shell(|c| c.export_ended(job::End::Written));
            })
            .expect("export thread");
    }
}
