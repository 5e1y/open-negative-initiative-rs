//! `ONI_BENCH=folded`: a phone paired over loopback steps Exposure with the panel shown, then folded
//! away; each step is counted as rendered, and as drawn when a frame reaches the screen after it.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const STEPS: usize = 6;
const LOOK: Duration = Duration::from_millis(300);

/// The phone's side, on its own thread: pairs on `uri`, then presses one step every 700 ms, asking
/// for the fold before the second half.
fn phone(uri: String, pressed: Arc<AtomicUsize>, fold: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
        runtime.block_on(async move {
            let code = match oni_link::PairingCode::parse(&uri) {
                Ok(code) => code,
                Err(e) => return crate::timing::line(&format!("bench phone: the code reads not: {e:?}")),
            };
            let stream = match oni_link::Remote::dial(&code.addresses).await {
                Ok(stream) => stream,
                Err(e) => return crate::timing::line(&format!("bench phone: no dial: {e:?}")),
            };
            let remote = match oni_link::Remote::pair(stream, &oni_link::Identity::generate(), &code, "Bench").await {
                Ok(remote) => remote,
                Err(e) => return crate::timing::line(&format!("bench phone: no pairing: {e:?}")),
            };
            crate::timing::line("bench phone: paired");
            // `ONI_FOLDED=scanner` steps the Scanner face's keypad on the Frontier machine instead.
            let scanner = std::env::var("ONI_FOLDED").is_ok_and(|v| v == "scanner");
            if scanner {
                remote.press(oni_link::Command::Face(oni_link::Face::Scanner));
                remote.press(oni_link::Command::Machine(oni_link::Machine::Frontier));
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
            let key = if scanner { oni_link::Command::Step { letter: oni_link::Letter::C, by: 1 } } else { oni_link::Command::StepParam { param: oni_link::Param::Exposure, by: 3 } };
            for step in 0..STEPS {
                if step == STEPS / 2 {
                    fold.store(true, Ordering::Relaxed);
                    tokio::time::sleep(Duration::from_millis(700)).await;
                }
                remote.press(key);
                pressed.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
        });
    });
}

pub(super) fn folded(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let frames = Rc::new(Cell::new(0u64));
    if let Some(w) = ctl.window() {
        let counted = frames.clone();
        let _ = w.window().set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                counted.set(counted.get() + 1);
            }
        });
    }
    let (pressed, fold) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let paired = Cell::new(false);
    let seen = Cell::new(0usize);
    let watching: Cell<Option<(u64, usize, bool)>> = Cell::new(None);
    let landed: Cell<Option<(u64, Instant, bool)>> = Cell::new(None);
    let lines = RefCell::new(Vec::new());
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(10), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        if ctl.state.borrow().renders() < 1 {
            return;
        }
        if !paired.get() {
            let uri = w.global::<crate::Pairing>().get_uri().to_string();
            let port = ctl.state.borrow().phone().and_then(|p| p.bound()).map(|a| a.port());
            match (uri.is_empty(), port) {
                (false, Some(port)) => {
                    paired.set(true);
                    phone(format!("{uri},127.0.0.1:{port}"), pressed.clone(), fold.clone());
                }
                _ if !w.global::<crate::Pairing>().get_shown() => ctl.act("pair-phone"),
                _ => {}
            }
            return;
        }
        if pressed.load(Ordering::Relaxed) > 0 && w.global::<crate::Pairing>().get_shown() {
            ctl.act("close-pairing");
        }
        let hidden = w.global::<crate::App>().get_panel_hidden();
        if fold.load(Ordering::Relaxed) && !hidden {
            ctl.act("hide-panel");
        }
        if pressed.load(Ordering::Relaxed) > seen.get() && watching.get().is_none() && landed.get().is_none() {
            seen.set(seen.get() + 1);
            let s = ctl.state.borrow();
            watching.set(Some((s.store().revision(), s.renders(), hidden)));
        }
        if let Some((revision, renders, folded)) = watching.get() {
            let s = ctl.state.borrow();
            if s.store().revision() > revision && s.rendered() > revision && s.renders() > renders {
                watching.set(None);
                landed.set(Some((frames.get(), Instant::now(), folded)));
            } else if started.elapsed() > Duration::from_secs(40) {
                watching.set(None);
                lines.borrow_mut().push(format!("step {}: panel {}, NOT rendered (revision {} → {})", seen.get(), if folded { "folded" } else { "shown" }, revision, s.store().revision()));
            }
        }
        if let Some((before, at, folded)) = landed.get()
            && at.elapsed() >= LOOK
        {
            landed.set(None);
            let drawn = frames.get() > before;
            lines.borrow_mut().push(format!("step {}: panel {}, rendered, {}", seen.get(), if folded { "folded" } else { "shown" }, if drawn { "drawn" } else { "NOT drawn" }));
        }
        if lines.borrow().len() >= STEPS || started.elapsed() > Duration::from_secs(45) {
            held.stop();
            ctl.act("hide-panel");
            ctl.flush();
            if let Some(done) = done.borrow_mut().take() {
                done(format!("folded:\n{}", lines.borrow().join("\n")));
            }
        }
    });
}
