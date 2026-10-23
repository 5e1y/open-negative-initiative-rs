//! `ONI_BENCH=sheet`: six frames picked and New Contact Sheet made from the bar.

use super::*;

/// The gallery's first six frames picked by the hand's clicks, then New Contact Sheet from the bar.
pub(super) fn sheet(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let asked: Cell<Option<(Instant, usize)>> = Cell::new(None);
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let finish = |text: String| {
            held.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(format!("bench, sheet: {text}"));
            }
        };
        let advice = w.global::<crate::Advice>();
        match asked.get() {
            None => {
                if ctl.state.borrow().renders() < 1 {
                    return;
                }
                ctl.act(crate::Act::ToggleMode);
                let lib = w.global::<crate::Library>();
                let mut tiles = crate::app::tiles(&w);
                tiles.sort_by_key(|t| t.index);
                for (i, t) in tiles.iter().filter(|t| !t.caption.contains("contact sheet")).take(6).enumerate() {
                    lib.invoke_tile_clicked(t.index, i > 0, false);
                }
                let title = w.global::<crate::Menus>().get_titles().sheet.to_string();
                w.global::<crate::Bar>().invoke_act(crate::Act::ContactSheet);
                asked.set(Some((Instant::now(), ctl.state.borrow().renders())));
                eprintln!("bench, sheet: {title} asked");
            }
            Some((at, renders)) => {
                if advice.get_shown() {
                    return finish(format!("{}: {}", advice.get_title(), advice.get_message()));
                }
                let open = ctl.state.borrow().open_path().map(str::to_string);
                // Done once the sheet is open and the canvas has drawn since the ask.
                if let Some(path) = open.filter(|p| p.contains("contact sheet")) && ctl.state.borrow().renders() > renders {
                    return finish(format!("{path} made, opened and drawn in {:.1} s, no failure", at.elapsed().as_secs_f64()));
                }
                if started.elapsed() > LOADING {
                    finish("no sheet opened".into());
                }
            }
        }
    });
}
