//! `ONI_BENCH=loupe`: the Details loupe built, then Sharpening notches until it shows each.

use super::*;

/// Sharpening notches while the Details loupe shows the frame at its own scale.
pub(super) const LOUPE_NOTCHES: usize = 10;

/// The Details tab on the open frame: its full resolution built and the loupe's first render, then
/// each Sharpening notch until the loupe shows it, the badge's 200 %, all undone.
pub(super) fn loupe(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let asked = Cell::new(None::<(Instant, u64)>);
    let times = RefCell::new(Vec::<f64>::new());
    let lines = RefCell::new(Vec::<String>::new());
    let start = Cell::new(0.0f32);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (panel, band, g) = (w.global::<crate::PanelState>(), w.global::<crate::DetailLoupe>(), w.global::<StoreGlobal>());
        let loupes = ctl.state.borrow().loupes();
        let n = step.get();
        let landed = |since: Option<(Instant, u64)>| since.filter(|(_, before)| loupes > *before).map(|(at, _)| at.elapsed().as_secs_f64() * 1000.0);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 {
                    return;
                }
                start.set(ctl.state.borrow().store().get(Field::SharpenAmount));
                panel.set_tab(3);
                panel.invoke_tab_picked(3);
                asked.set(Some((Instant::now(), loupes)));
                step.set(1);
            }
            1 => {
                let Some(ms) = landed(asked.get()) else { return };
                let size = band.get_source().size();
                lines.borrow_mut().push(format!("first loupe, full resolution built: {ms:.0} ms, {} × {} source px at {}", size.width, size.height, band.get_ratio()));
                step.set(2);
            }
            k if k >= 2 && k < 2 + 2 * LOUPE_NOTCHES => {
                if k % 2 == 0 {
                    g.invoke_step(Field::SharpenAmount.index(), 1);
                    asked.set(Some((Instant::now(), loupes)));
                    step.set(k + 1);
                } else if let Some(ms) = landed(asked.get()) {
                    times.borrow_mut().push(ms);
                    step.set(k + 1);
                }
            }
            k if k == 2 + 2 * LOUPE_NOTCHES => {
                band.invoke_toggled();
                asked.set(Some((Instant::now(), loupes)));
                step.set(k + 1);
            }
            k if k == 3 + 2 * LOUPE_NOTCHES => {
                let Some(ms) = landed(asked.get()) else { return };
                let size = band.get_source().size();
                lines.borrow_mut().push(format!("200 %: {ms:.0} ms, {} × {} source px at {}", size.width, size.height, band.get_ratio()));
                band.invoke_toggled();
                for _ in 0..LOUPE_NOTCHES {
                    if ctl.state.borrow().store().get(Field::SharpenAmount) == start.get() {
                        break;
                    }
                    ctl.act("undo");
                }
                let back = ctl.state.borrow().store().get(Field::SharpenAmount) == start.get();
                lines.borrow_mut().push(format!("undone to the amount it started from: {back}"));
                timer.stop();
                panel.set_tab(2);
                panel.invoke_tab_picked(2);
                let mut t = times.borrow().clone();
                t.sort_by(f64::total_cmp);
                let report = format!(
                    "bench, loupe:\n{}\na Sharpening notch reaches the loupe in {:.0} ms median, {:.0} ms worst, over {} notches",
                    lines.borrow().join("\n"),
                    t.get(t.len() / 2).copied().unwrap_or(f64::NAN),
                    t.last().copied().unwrap_or(f64::NAN),
                    t.len()
                );
                if let Some(done) = done.borrow_mut().take() {
                    done(report);
                }
            }
            _ => {}
        }
    });
}
