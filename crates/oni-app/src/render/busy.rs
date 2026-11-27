//! What the render thread did while a job waited for it, written past `WAIT` so a stall the hand feels names its
//! cause; nothing when the thread was free.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A job waiting longer than this for the render thread is a stall a hand on the canvas feels.
pub const WAIT: Duration = Duration::from_millis(50);

/// The render thread's last tasks, each named with when it ran.
#[derive(Default)]
pub struct Busy(VecDeque<(&'static str, Instant, Instant)>);

impl Busy {
    /// `work` run on the render thread and kept under `name`.
    pub fn run<T>(&mut self, name: &'static str, work: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let out = work();
        self.0.push_back((name, start, Instant::now()));
        if self.0.len() > 32 {
            self.0.pop_front();
        }
        out
    }

    /// The line a job of `kind` asked at `asked` and started at `started` owes the log: each task that ran
    /// meanwhile and its share of the wait, then what none accounts for; none within `WAIT`.
    pub fn behind(&self, kind: &str, asked: Instant, started: Instant) -> Option<String> {
        let waited = started.saturating_duration_since(asked);
        if waited <= WAIT {
            return None;
        }
        let mut by: Vec<(&str, Duration)> = Vec::new();
        for (name, from, to) in &self.0 {
            let share = (*to).min(started).saturating_duration_since((*from).max(asked));
            if share.is_zero() {
                continue;
            }
            match by.iter_mut().find(|(n, _)| n == name) {
                Some(seen) => seen.1 += share,
                None => by.push((name, share)),
            }
        }
        let told: Duration = by.iter().map(|(_, d)| *d).sum();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut parts: Vec<String> = by.iter().map(|(n, d)| format!("{n} {:.0} ms", ms(*d))).collect();
        let rest = waited.saturating_sub(told);
        if rest >= Duration::from_millis(1) || parts.is_empty() {
            parts.push(format!("no task of its own {:.0} ms", ms(rest)));
        }
        Some(format!("render: a {kind} waited {:.0} ms behind {}", ms(waited), parts.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A job asked while a scan ahead ran waits behind it, and the line says so with its share; the twin, a job
    /// asked once the thread was free, owes the log nothing.
    #[test]
    fn a_long_wait_names_what_the_render_thread_ran() {
        let mut busy = Busy::default();
        let t0 = Instant::now();
        busy.0.push_back(("a scan ahead", t0, t0 + Duration::from_millis(150)));
        busy.0.push_back(("the GPU finishing a render", t0 + Duration::from_millis(150), t0 + Duration::from_millis(160)));
        let line = busy.behind("view", t0 + Duration::from_millis(20), t0 + Duration::from_millis(170)).expect("a stall");
        assert_eq!(line, "render: a view waited 150 ms behind a scan ahead 130 ms, the GPU finishing a render 10 ms, no task of its own 10 ms");
        assert_eq!(busy.behind("view", t0 + Duration::from_millis(165), t0 + Duration::from_millis(170)), None, "twin: no wait");
        assert_eq!(busy.run("a count", || 7), 7);
        assert_eq!(busy.0.back().map(|t| t.0), Some("a count"));
    }
}
