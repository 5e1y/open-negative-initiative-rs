//! `ONI_BENCH=wrapped`: the year on film shown in the real window, each scene held until it is told,
//! then the share screen, every card saved and one copied; `ONI_WRAPPED_SHOTS` names a folder that takes
//! the window's picture at each scene and the cards.

use super::*;
use crate::Wrapped;

/// The film's advance and a scene's telling, with a margin: after it a scene is whole.
const TOLD: Duration = Duration::from_millis(3_400);
const SHOWN: Duration = Duration::from_secs(60);

pub(super) fn wrapped(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let shots = std::env::var_os("ONI_WRAPPED_SHOTS").map(std::path::PathBuf::from);
    let started = Instant::now();
    let step = Cell::new(None::<(usize, Instant)>);
    // What the cards' writer answered, once it has.
    let saved: std::sync::Arc<std::sync::Mutex<Option<String>>> = Default::default();
    let saving = Cell::new(false);
    let drawn = frame_clock(ctl);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    ctl.act(crate::Act::Wrapped);
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return held.stop() };
        let g = w.global::<Wrapped>();
        let finish = |report: String| {
            held.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(report);
            }
        };
        if saving.get() {
            let Some(saved) = saved.lock().ok().and_then(|mut s| s.take()) else { return };
            let count = g.get_scenes().row_count();
            let frames = frames_since(&drawn.borrow(), started);
            g.invoke_close();
            return finish(format!("wrapped: {count} scenes told in {:.1} s, {saved}; {frames}", started.elapsed().as_secs_f64()));
        }
        let Some((scene, since)) = step.get() else {
            if g.get_shown() {
                g.set_playing(false);
                step.set(Some((0, Instant::now())));
            } else if started.elapsed() > SHOWN {
                finish("wrapped: the summary never showed".into());
            }
            return;
        };
        if since.elapsed() < TOLD {
            return;
        }
        let count = g.get_scenes().row_count();
        if let Some(folder) = &shots
            && let Ok(shot) = w.window().take_snapshot()
            && let Some(data) = crate::wrapped::share::jpeg(shot.as_bytes(), shot.width() as usize, shot.height() as usize)
        {
            let _ = std::fs::create_dir_all(folder);
            let _ = std::fs::write(folder.join(format!("scene-{:02}.jpg", scene + 1)), data);
        }
        if scene < count {
            // Past the last scene the same step lands on the share screen, shot as one more.
            g.invoke_forward();
            return step.set(Some((scene + 1, Instant::now())));
        }
        ctl.copy_wrapped(0);
        saving.set(true);
        let answer = saved.clone();
        let tell = move |said: String| {
            if let Ok(mut answer) = answer.lock() {
                *answer = Some(said);
            }
        };
        match &shots {
            Some(folder) => ctl.save_wrapped_into(folder, None, move |written| {
                tell(match written {
                    Ok(cards) => format!("{} cards saved", cards.len()),
                    Err(e) => format!("no card saved: {e}"),
                })
            }),
            None => tell("no card asked".to_string()),
        }
    });
}
