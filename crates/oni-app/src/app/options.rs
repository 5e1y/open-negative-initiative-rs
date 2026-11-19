//! The window's options: what the app hands the shell, and what a check hands in its place.

use super::*;

pub struct Options {
    pub screen: Screen,
    /// Slint draws the window on the engine's device and takes the canvas texture as it is; on its
    /// software renderer the canvas is read back.
    pub canvas: bool,
    /// `AppFolders.support`: the phone link's keys, the preferences, the thumbnails.
    pub support: PathBuf,
    /// `AppFolders.caches`: the index, which the system may purge.
    pub caches: PathBuf,
    pub picker: Picker,
    pub discard: Discard,
    pub reporter: Reporter,
    /// `FlatFieldBlock.load`'s open panel and the Folder key's Finder.
    pub mask_picker: Picker,
    pub show_folder: Shower,
    /// The home folder and the volumes' folder the navigator walks; a check hands a disk of its own.
    pub computer: oni_library::navigator::Computer,
    /// `requireRoll`: a frame never measured opens on Prepare the Roll; off as under Swift's probes,
    /// so a check opens bare frames by hand, and on in the app itself.
    pub require_roll: bool,
    /// The update feed's reader; none looks nowhere, as the checks run.
    pub updates: Option<Updater>,
    /// `Propagation.board`: the general pasteboard in the app, a board of the process's own for a check.
    pub pasteboard: Box<dyn crate::pasteboard::Board>,
    /// Names the volume a path lies on: the system's in the app, a check's own for its folders.
    pub identify: oni_library::places::Identify,
    /// The phone link offered at all (`phone::offered`): off, no entry to it shows and nothing listens.
    pub phone: bool,
    /// What the launch's `migration::support` left: imports due, never written this run, and its notice.
    pub brought: crate::migration::Brought,
    /// Opens a web page: the person's browser in the app, a check's own record of the address.
    pub browse: Box<dyn Fn(&str)>,
    /// The Lens DB the Kit talks to; none, as every check and bench runs unless it hands a simulated one.
    pub tldb: TldbSite,
}

/// Which The Lens DB the Kit asks.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum TldbSite {
    /// No token read and nothing asked: Connect opens nothing.
    #[default]
    None,
    /// thelensdb.com, its token in the system's keychain.
    Real,
    /// A simulated service at this loopback address, its worthless token in the support folder.
    Simulated(String),
}

impl TldbSite {
    /// The app's own: `ONI_TLDB=fake` starts the simulated service for the run, keeping its accounts in the
    /// support folder and taking its `tldb-catalogue.json` when present; a loopback address names one running.
    pub fn launch(support: &std::path::Path) -> TldbSite {
        static FAKE: std::sync::OnceLock<Option<oni_tldb::fake::Fake>> = std::sync::OnceLock::new();
        match std::env::var("ONI_TLDB") {
            Ok(v) if v == "fake" => {
                let fake = FAKE.get_or_init(|| {
                    let file = support.join("tldb-catalogue.json");
                    let catalogue = file.exists().then(|| oni_tldb::fake::read_catalogue(&file)).and_then(|read| {
                        read.map_err(|e| crate::timing::line(&format!("tldb: the simulated catalogue is unreadable: {e}"))).ok()
                    });
                    let setup = oni_tldb::fake::Setup { catalogue, kept: Some(support.join("tldb-fake.json")), ..Default::default() };
                    oni_tldb::fake::start_with(setup).map_err(|e| crate::timing::line(&format!("tldb: the simulated service does not start: {:?}", e.kind()))).ok()
                });
                fake.as_ref().map_or(TldbSite::None, |f| TldbSite::Simulated(f.site.clone()))
            }
            Ok(v) if v.starts_with("http://127.0.0.1:") => TldbSite::Simulated(v),
            _ => TldbSite::Real,
        }
    }
}

/// `EditorScene.open`'s panel message.
pub const PICKER_MESSAGE: &str = oni_i18n::marked("Choose a folder: every photo under it shows in the archive. Nothing is copied or moved.");

impl Options {
    /// The bug reporter the app ships with, posting to the site.
    pub fn online_reporter() -> help::Reporter {
        std::sync::Arc::new(help::post)
    }
}

impl Default for Options {
    fn default() -> Self {
        Options {
            screen: Screen::Srgb,
            canvas: false,
            // A check's own folder and no reporter: it never writes the owner's journal nor opens an issue.
            support: std::env::temp_dir().join(format!("oni-support-{}", std::process::id())),
            caches: std::env::temp_dir().join(format!("oni-caches-{}", std::process::id())),
            picker: Box::new(|| system::pick_folder(tr(PICKER_MESSAGE))),
            discard: Box::new(system::trash),
            reporter: std::sync::Arc::new(|_: &str, _: Option<String>| Err("no reporter in a check".to_string())),
            mask_picker: Box::new(|| system::pick_file(masks::LOAD_MESSAGE, oni_library::library::IMAGE_EXTENSIONS)),
            show_folder: Box::new(system::show_folder),
            // The app hands this machine's; a check gets none unless it hands its own.
            computer: oni_library::navigator::Computer::none(),
            require_roll: false,
            updates: None,
            pasteboard: Box::new(crate::pasteboard::Memory::default()),
            identify: oni_library::places::system(),
            phone: phone::offered(),
            brought: Default::default(),
            browse: Box::new(system::open_url),
            tldb: TldbSite::None,
        }
    }
}
