//! A batch written as one contact sheet: its frames laid as Edit Together lays them, numbered by
//! their place on their rolls, in one file.

use super::*;
use super::super::gallery_view::together::{self, Captions};
use oni_colour::PaperCorrection;
use oni_export::Failure;

impl Ctl {
    /// The sheet off this thread on an engine apart, the stop honoured between frames; it carries the
    /// first line's artist, copyright and date, each frame's paper correction laid on its own tile.
    pub(super) fn write_sheet(self: &Rc<Self>, destinations: Vec<PathBuf>, replacing: bool) {
        let Some(destination) = destinations.into_iter().next() else { return };
        let Some(inks) = self.inks() else { return };
        let (targets, sizes, export, stop, metadata) = {
            let mut c = self.export.borrow_mut();
            c.start_writing();
            let first = c.rows.first().cloned().flatten().map(|r| r.metadata).unwrap_or_default();
            let metadata = sheet_metadata(first);
            // The sizes the panel laid its sheet on, so the file is the size it said.
            let sizes = c.sizes.iter().copied().collect::<Option<Vec<_>>>();
            (c.targets.clone(), sizes, c.settings.clone(), c.writer.flag(), metadata)
        };
        let paths: Vec<String> = targets.iter().map(|t| oni_library::paths::normal(&t.path.to_string_lossy()).into_owned()).collect();
        let folders = self.roll_folders(&paths);
        self.job_started(std::slice::from_ref(&destination));
        let engine: Shared = self.export_engine();
        let support = self.export.borrow().support.clone();
        let journal = crate::app::year::journal_in(&support);
        self.export.borrow_mut().writer.spawn("export sheet", move || {
            let mut ledger = oni_export::file::Ledger::in_support(&support);
            let written = (|| {
                for t in &targets {
                    oni_export::lands(&t.path, &destination, &ledger, replacing)?;
                }
                let numbers = together::ranks(&paths, &folders);
                let captions = Captions { numbers, inks };
                let Some(sheet) = together::printed(&engine, &targets, sizes, &captions, export.space(), &stop)? else { return Ok(false) };
                let bytes = oni_export::file_bytes(sheet, &PaperCorrection::default(), &export, &metadata)?;
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
                    let message = format!("{}: {e}", oni_i18n::named(&name));
                    with_shell(move |c| c.export_ended(job::End::Failed(message)));
                }
            }
        });
    }
}

/// What a sheet states of its frames: the first line's artist, copyright and date, as shown; never a
/// camera, lens, ISO or film, which are each frame's own.
fn sheet_metadata(first: ExifMetadata) -> ExifMetadata {
    ExifMetadata { artist: first.artist, copyright: first.copyright, date: first.date, ..ExifMetadata::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_states_the_first_lines_artist_copyright_and_date_alone() {
        let line = |date: &str| ExifMetadata {
            camera: "Nikon Z f".into(),
            lens: "Nikkor 40".into(),
            film: "Portra 400".into(),
            iso: "400".into(),
            date: date.into(),
            artist: "A. Tester".into(),
            copyright: "© A. Tester".into(),
        };
        let stated = sheet_metadata(line("2026:09:14 16:46:19"));
        assert_eq!(stated, ExifMetadata { artist: "A. Tester".into(), copyright: "© A. Tester".into(), date: "2026:09:14 16:46:19".into(), ..ExifMetadata::default() });
        // Twin: a first line cleared of its date gives the sheet none.
        assert_eq!(sheet_metadata(line("")).date, "");
    }
}
