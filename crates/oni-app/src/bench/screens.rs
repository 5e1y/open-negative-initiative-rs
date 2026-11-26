//! `ONI_BENCH=screens`: the main screens at the smallest window, each taken off the window into
//! `ONI_SHOTS` as `<language>-<screen>.jpg`: the editor's three panes, the export, Settings, the gallery
//! and Prepare the Roll, for the eye to judge a face or a language in the app's own build.

use super::*;

/// The window the checks of width hold the interface to, in points.
const SMALLEST: (f32, f32) = (1100.0, 680.0);
/// What a screen is given to lay out and settle before it is taken.
const SETTLE: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy)]
enum Shot {
    Colour,
    Roll,
    Details,
    Export,
    Settings,
    Gallery,
    Prepare,
}

const SHOTS: [Shot; 7] = [Shot::Colour, Shot::Roll, Shot::Details, Shot::Export, Shot::Settings, Shot::Gallery, Shot::Prepare];

impl Shot {
    fn name(self) -> &'static str {
        match self {
            Shot::Colour => "colour",
            Shot::Roll => "roll",
            Shot::Details => "details",
            Shot::Export => "export",
            Shot::Settings => "settings",
            Shot::Gallery => "gallery",
            Shot::Prepare => "prepare",
        }
    }

    /// The screen brought up from the one before it.
    fn show(self, ctl: &Rc<Ctl>, w: &crate::MainWindow) {
        let tab = |tab: crate::PanelTab| {
            let panel = w.global::<crate::PanelState>();
            panel.set_tab(tab);
            panel.invoke_tab_picked(tab);
        };
        match self {
            Shot::Colour => tab(crate::PanelTab::Color),
            Shot::Roll => tab(crate::PanelTab::Roll),
            Shot::Details => tab(crate::PanelTab::Details),
            Shot::Export => ctl.act(crate::Act::Export),
            Shot::Settings => {
                ctl.leave_export();
                ctl.act(crate::Act::Settings);
            }
            Shot::Gallery => {
                w.global::<crate::AppSettings>().set_shown(false);
                ctl.act(crate::Act::ToggleMode);
            }
            Shot::Prepare => ctl.act(crate::Act::PrepareRoll),
        }
    }
}

pub(super) fn screens(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let folder = std::env::var_os("ONI_SHOTS").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let language = oni_i18n::current().key();
    let timer = Rc::new(Timer::default());
    let next = Cell::new(None::<usize>);
    let since = Cell::new(Instant::now());
    let written = RefCell::new(Vec::<String>::new());
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let finish = |text: String| {
            held.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let Some(at) = next.get() else {
            if ctl.state.borrow().renders() >= 1 && ctl.state.borrow().placed().is_some() {
                w.window().set_size(slint::LogicalSize::new(SMALLEST.0, SMALLEST.1));
                SHOTS[0].show(&ctl, &w);
                next.set(Some(0));
                since.set(Instant::now());
            } else if started.elapsed() > LOADING {
                finish("screens: no frame rendered".into());
            }
            return;
        };
        if since.get().elapsed() < SETTLE {
            return;
        }
        let shot = SHOTS[at];
        let path = folder.join(format!("{language}-{}.jpg", shot.name()));
        let taken = w.window().take_snapshot().ok().and_then(|s| crate::thumbnails::jpeg(s.as_bytes(), s.width() as usize, s.height() as usize));
        match taken.map(|data| std::fs::write(&path, data)) {
            Some(Ok(())) => written.borrow_mut().push(shot.name().into()),
            _ => written.borrow_mut().push(format!("{} not taken", shot.name())),
        }
        if at + 1 == SHOTS.len() {
            return finish(format!("screens ({language}): {}", written.borrow().join(", ")));
        }
        SHOTS[at + 1].show(&ctl, &w);
        next.set(Some(at + 1));
        since.set(Instant::now());
    });
}
