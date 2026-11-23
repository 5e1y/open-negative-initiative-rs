//! The person's stock and paper files in the support folder's `Stocks`, read before any photo renders and then watched:
//! a burst of changes is read off the interface thread and its catalogue put in use on it, menus, heroes and canvas following.

use super::*;
use notify::{RecursiveMode, Watcher};
use oni_pipeline::stocks::{Loader, Reloaded, Told};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};

/// The folder's name inside the support folder.
pub const FOLDER: &str = "Stocks";

/// How long the folder stays quiet before it is read again: a file saved in several writes is read once, whole.
const QUIET: Duration = Duration::from_millis(250);

/// FSEvents on the Mac; polling on Windows, where a watch holds its folder open and the folder can then not be renamed.
#[cfg(target_os = "macos")]
type Backend = notify::FsEventWatcher;
#[cfg(windows)]
type Backend = notify::PollWatcher;
#[cfg(not(any(target_os = "macos", windows)))]
type Backend = notify::RecommendedWatcher;

/// Windows' polling interval: a saved file is seen within it.
const POLL: Duration = Duration::from_secs(1);

enum Order {
    Changed,
    Stop,
}

/// The watching thread's handle; dropping it ends the thread and the watch.
pub(in crate::app) struct Watching {
    orders: Sender<Order>,
}

impl Drop for Watching {
    fn drop(&mut self) {
        let _ = self.orders.send(Order::Stop);
    }
}

/// The folder read and its catalogue put in use, then watched; what it refuses is told once the shell stands.
pub(in crate::app) fn start(support: &Path) -> Watching {
    let folder = support.join(FOLDER);
    let _ = std::fs::create_dir_all(&folder);
    let mut loader = Loader::new(folder.clone());
    if let Some(first) = loader.reload() {
        crate::timing::line(&format!("stock files read: {}", counted(&first)));
        oni_settings::stocks::install(first.catalogue);
        if !first.refused.is_empty() {
            let refused = first.refused;
            with_shell(move |c| c.stocks_refused(&refused));
        }
    }
    let (orders, inbox) = mpsc::channel::<Order>();
    let events = orders.clone();
    let _ = std::thread::Builder::new().name("stock files".into()).spawn(move || {
        let mut watcher = Backend::new(
            move |_: notify::Result<notify::Event>| {
                let _ = events.send(Order::Changed);
            },
            notify::Config::default().with_poll_interval(POLL),
        )
        .ok();
        if let Some(w) = watcher.as_mut()
            && let Err(e) = w.watch(&folder, RecursiveMode::NonRecursive)
        {
            crate::timing::line(&format!("stock files: the folder is not watched ({e})"));
        }
        loop {
            match inbox.recv() {
                Ok(Order::Changed) => {}
                Ok(Order::Stop) | Err(_) => return,
            }
            loop {
                match inbox.recv_timeout(QUIET) {
                    Ok(Order::Changed) => {}
                    Ok(Order::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => break,
                }
            }
            if let Some(reloaded) = loader.reload() {
                crate::timing::line(&format!("stock files read again: {}", counted(&reloaded)));
                with_shell(move |c| c.stocks_changed(reloaded));
            }
        }
    });
    Watching { orders }
}

/// The catalogue's counts, for the log: no file is named there.
fn counted(r: &Reloaded) -> String {
    format!("{} stocks, {} papers", r.catalogue.films().len(), r.catalogue.papers().len())
}

impl Ctl {
    /// A catalogue read from the person's files put in use: the menus laid again on it, any lit stock let go, the open
    /// photo, its heroes and the roll's shown again on it, every refusal told.
    pub(in crate::app) fn stocks_changed(self: &Rc<Self>, reloaded: Reloaded) {
        oni_settings::stocks::install(reloaded.catalogue);
        {
            let mut s = self.state.borrow_mut();
            s.lit = None;
            s.render_anew();
        }
        self.lay_stock_menu();
        self.lay_paper_menu();
        self.stock_picker_again();
        self.publish_roll_tab();
        self.publish_info();
        self.request_render();
        self.stocks_refused(&reloaded.refused);
    }

    /// Each file refused, told by its name, which the log masks.
    pub(in crate::app) fn stocks_refused(&self, refused: &[Told]) {
        for told in refused {
            let (name, why) = (&told.refusal.name, &told.refusal.reason);
            let message = if told.earlier_kept {
                tr!("{file} is not used as it stands, its last version that read stays in use: {why}", file = oni_i18n::named(name), why = why)
            } else {
                tr!("{file} is not used: {why}", file = oni_i18n::named(name), why = why)
            };
            self.told(tr("Stock files"), &message);
        }
    }
}
