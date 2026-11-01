//! A batch written as one contact sheet: its frames laid as Edit Together lays them, numbered by
//! their place on their rolls, in one file.

use super::*;
use super::super::gallery_view::together::{self, Captions};
use oni_export::Failure;

impl Ctl {
    /// The sheet off this thread on an engine apart, the stop honoured between frames; it carries the
    /// first line's artist and copyright, and the first frame's date and paper correction.
    pub(super) fn write_sheet(self: &Rc<Self>, destinations: Vec<PathBuf>, replacing: bool) {
        let Some(destination) = destinations.into_iter().next() else { return };
        let Some(lettering) = self.lettering() else { return };
        let (targets, sizes, export, stop, metadata) = {
            let mut c = self.export.borrow_mut();
            c.start_writing();
            let first = c.rows.first().cloned().flatten().map(|r| r.metadata).unwrap_or_default();
            let metadata = ExifMetadata { artist: first.artist, copyright: first.copyright, ..ExifMetadata::default() };
            // The sizes the panel laid its sheet on, so the file is the size it said.
            let sizes = c.sizes.iter().copied().collect::<Option<Vec<_>>>();
            (c.targets.clone(), sizes, c.settings.clone(), c.stop.clone(), metadata)
        };
        let paths: Vec<String> = targets.iter().map(|t| oni_library::paths::normal(&t.path.to_string_lossy()).into_owned()).collect();
        let (folders, heading) = (self.roll_folders(&paths), self.heading_of(&paths));
        self.job_started(std::slice::from_ref(&destination));
        let engine: Shared = self.export_engine();
        let support = self.export.borrow().support.clone();
        let journal = crate::app::year::journal_in(&support);
        std::thread::Builder::new()
            .name("export sheet".into())
            .spawn(move || {
                let mut ledger = oni_export::file::Ledger::in_support(&support);
                let written = (|| {
                    for t in &targets {
                        oni_export::lands(&t.path, &destination, &ledger, replacing)?;
                    }
                    let numbers = together::ranks(&paths, &folders, oni_library::roll_membership::is_sheet);
                    let captions = Captions { numbers, heading, lettering };
                    let Some(sheet) = together::printed(&engine, &targets, sizes, &captions, &stop)? else { return Ok(false) };
                    let first = &targets[0];
                    let bytes = oni_export::file_bytes(&first.path, sheet, &first.settings.paper_correction, &export, &metadata, None)?;
                    oni_export::land(oni_export::Landing { path: &destination, ledger: &mut ledger, replacing }, &bytes)?;
                    Ok::<_, Failure>(true)
                })();
                match written {
                    Ok(true) => {
                        journal.note(Gesture::Exported, None, 0, 0, None, oni_settings::Date::now());
                        with_shell(|c| {
                            c.export.borrow_mut().progress = 1;
                            c.export_ended(job::End::Written);
                        });
                    }
                    Ok(false) => with_shell(|c| c.export_ended(job::End::Stopped)),
                    Err(e) => {
                        let name = destination.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        let message = format!("{name}: {e}");
                        with_shell(move |c| c.export_ended(job::End::Failed(message)));
                    }
                }
            })
            .expect("export sheet thread");
    }
}
