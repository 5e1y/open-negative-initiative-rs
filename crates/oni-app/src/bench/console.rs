//! `ONI_BENCH=console`: the console window open on the session's log.

use super::*;

pub(super) fn console(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    ctl.act("console");
    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let shown = ctl.console_log();
        let report = match shown {
            Some(log) if log.contains("GPU:") => format!("console: the window shows the session's log, {} lines, no failure", log.lines().count()),
            _ if started.elapsed() > Duration::from_secs(10) => format!("console: the log never showed ({:?} characters)", shown.map(|l| l.len())),
            _ => return,
        };
        held.stop();
        if let Some(done) = done.borrow_mut().take() {
            done(report);
        }
    });
}
