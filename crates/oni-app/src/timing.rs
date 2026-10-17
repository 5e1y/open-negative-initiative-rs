//! What the app says about its own speed: the GPU it runs on, each render's job and times, and the
//! notch renders' centiles, on stderr and in the log file beside the support folder's other files.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `AppFolders.name`: the folder every platform's support directory holds for this app.
pub const FOLDER: &str = "Open Negative Initiative";
/// The notch renders between two printed reports.
pub const REPORT_EVERY: usize = 120;
/// One refresh at 120 Hz: a notch shown later than this behind the previous one missed a frame.
pub const FRAME_BUDGET: Duration = Duration::from_micros(8_333);
/// Beyond this a pause between two notch frames is the hand resting, not a slow frame.
const IDLE: Duration = Duration::from_secs(1);

/// The app's support folder: `ONI_SUPPORT`, else the platform's application-support directory.
pub fn support() -> PathBuf {
    if let Some(dir) = std::env::var_os("ONI_SUPPORT") {
        return PathBuf::from(dir);
    }
    let home = || std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let base = if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home)
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
    };
    base.join(FOLDER)
}

/// `<support>/Logs/oni.log`, appended to by every run.
pub fn log_path(support: &Path) -> PathBuf {
    support.join("Logs").join("oni.log")
}

struct Sink {
    start: Instant,
    file: Mutex<Option<File>>,
}

static SINK: OnceLock<Sink> = OnceLock::new();

fn sink() -> &'static Sink {
    SINK.get_or_init(|| Sink { start: Instant::now(), file: Mutex::new(None) })
}

/// Opens the log file for the rest of the run; without it, lines go to stderr alone.
pub fn open_file(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    *sink().file.lock().map_err(|_| "the log failed earlier")? = Some(file);
    Ok(())
}

/// One line, stamped with the seconds since the first line, to stderr and to the file.
pub fn line(text: &str) {
    let s = sink();
    let stamped = format!("[{:9.3}] {text}", s.start.elapsed().as_secs_f64());
    eprintln!("{stamped}");
    if let Ok(mut file) = s.file.lock()
        && let Some(f) = file.as_mut()
    {
        let _ = writeln!(f, "{stamped}");
    }
}

/// The three jobs a render serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A setting moved on the open frame.
    Notch,
    Thumbnail,
    /// The first render of a frame just opened, its decode included.
    Open,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Notch => "notch",
            Kind::Thumbnail => "thumbnail",
            Kind::Open => "open",
        }
    }
}

/// One render's line: its job, its frame, the pipeline's time and the time from the ask to the screen.
pub fn render(kind: Kind, frame: &str, pipeline: Duration, end_to_end: Duration) {
    line(&format!("render {} {frame}: pipeline {:.1} ms, end to end {:.1} ms", kind.word(), ms(pipeline), ms(end_to_end)));
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The notch renders shown: each one's time from the store write to the screen, and the gaps
/// between two shown while the hand moves.
#[derive(Default)]
pub struct Notches {
    latencies: Vec<Duration>,
    /// Each gap with the index of the render that ended it.
    gaps: Vec<(usize, Duration)>,
    last_shown: Option<Instant>,
    reported: usize,
}

impl Notches {
    pub fn record(&mut self, end_to_end: Duration, shown: Instant) {
        self.latencies.push(end_to_end);
        if let Some(last) = self.last_shown {
            let gap = shown.saturating_duration_since(last);
            if gap < IDLE {
                self.gaps.push((self.latencies.len() - 1, gap));
            }
        }
        self.last_shown = Some(shown);
    }

    pub fn count(&self) -> usize {
        self.latencies.len()
    }

    /// The renders since the last report, every `REPORT_EVERY`; `None` before that.
    pub fn due(&mut self) -> Option<String> {
        if self.latencies.len() < self.reported + REPORT_EVERY {
            return None;
        }
        let from = self.reported;
        self.reported = self.latencies.len();
        Some(self.summary(from, "last notches"))
    }

    /// The centiles of the notch renders recorded since `from`, the index `count` gave.
    pub fn summary(&self, from: usize, title: &str) -> String {
        let latencies = &self.latencies[from.min(self.latencies.len())..];
        let gaps: Vec<Duration> = self.gaps.iter().filter(|(i, _)| *i > from).map(|(_, g)| *g).collect();
        let mut out = format!("{title}: {} notch renders", latencies.len());
        if latencies.is_empty() {
            return out;
        }
        let l = Centiles::of(latencies);
        let _ = write!(out, "; end to end p50 {:.1} ms, p99 {:.1} ms, worst {:.1} ms", ms(l.p50), ms(l.p99), ms(l.worst));
        if gaps.is_empty() {
            out.push_str("; no gap between two notch frames");
        } else {
            let g = Centiles::of(&gaps);
            let late = gaps.iter().filter(|g| **g > FRAME_BUDGET).count();
            let _ = write!(
                out,
                "; gap p50 {:.1} ms, p99 {:.1} ms, worst gap {:.1} ms, {late} of {} past {:.1} ms",
                ms(g.p50),
                ms(g.p99),
                ms(g.worst),
                gaps.len(),
                ms(FRAME_BUDGET)
            );
        }
        out
    }
}

/// `FrameRate.describe`'s centile: the sample at `count × q`, never interpolated.
struct Centiles {
    p50: Duration,
    p99: Duration,
    worst: Duration,
}

impl Centiles {
    fn of(samples: &[Duration]) -> Centiles {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let at = |q: f64| sorted[((sorted.len() as f64 * q) as usize).min(sorted.len() - 1)];
        Centiles { p50: at(0.5), p99: at(0.99), worst: *sorted.last().expect("a sample") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notches(latencies_ms: &[u64], gap_ms: u64) -> Notches {
        let mut n = Notches::default();
        let start = Instant::now();
        for (i, l) in latencies_ms.iter().enumerate() {
            n.record(Duration::from_millis(*l), start + Duration::from_millis(gap_ms * i as u64));
        }
        n
    }

    #[test]
    fn the_centiles_read_the_slow_tail_and_the_gaps_between_frames() {
        // A hundred notches at 5 ms and two at 40 ms: the median stays, the 99th centile sees them.
        let mut latencies = vec![5u64; 100];
        latencies.extend([40, 40]);
        let n = notches(&latencies, 10);
        let text = n.summary(0, "drag");
        assert!(text.starts_with("drag: 102 notch renders"), "{text}");
        assert!(text.contains("p50 5.0 ms, p99 40.0 ms, worst 40.0 ms"), "{text}");
        assert!(text.contains("worst gap 10.0 ms, 101 of 101 past 8.3 ms"), "{text}");
        // Twin: at 8 ms apart no frame is past the budget.
        let quick = notches(&latencies, 8);
        assert!(quick.summary(0, "drag").contains("0 of 101 past 8.3 ms"));
    }

    #[test]
    fn a_rest_between_two_drags_is_no_gap_and_a_report_covers_its_own_window() {
        let mut n = Notches::default();
        let start = Instant::now();
        n.record(Duration::from_millis(3), start);
        n.record(Duration::from_millis(3), start + Duration::from_secs(5));
        assert!(n.summary(0, "x").contains("no gap"), "a five-second rest is not a frame");
        // Twin: the same two renders 20 ms apart are a gap.
        let mut m = Notches::default();
        m.record(Duration::from_millis(3), start);
        m.record(Duration::from_millis(3), start + Duration::from_millis(20));
        assert!(m.summary(0, "x").contains("worst gap 20.0 ms"));

        let mut r = notches(&vec![2; REPORT_EVERY - 1], 5);
        assert!(r.due().is_none());
        r.record(Duration::from_millis(9), Instant::now() + Duration::from_secs(9));
        let report = r.due().expect("a report at the hundred-and-twentieth notch");
        assert!(report.contains(&format!("{REPORT_EVERY} notch renders")), "{report}");
        assert!(r.due().is_none(), "the next one waits for another window");
        let since = r.count();
        r.record(Duration::from_millis(7), Instant::now() + Duration::from_secs(20));
        assert!(r.summary(since, "bench").contains("1 notch renders; end to end p50 7.0 ms"));
    }
}
