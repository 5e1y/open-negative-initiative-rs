//! `AutomaticKey`'s words and `AutoBalanceChoice`: what a press over several frames asks, and how
//! the two strategies name themselves, each carrying the count.

use oni_core::balance::Strategy;

pub const LABEL: &str = "Auto White Balance";
pub const HINT: &str = "The frame's light toward grey with Temperature and Tint, scaled by Balance strength";

pub const STRATEGIES: [Strategy; 2] = [Strategy::Each, Strategy::Average];

pub fn label(strategy: Strategy, count: usize) -> String {
    match strategy {
        Strategy::Each => format!("Balance Each of the {count}"),
        Strategy::Average => format!("Average the {count} Balances"),
    }
}

pub fn explanation(strategy: Strategy, count: usize) -> String {
    match strategy {
        Strategy::Each => format!(
            "Balance Each corrects the color of all {count} photos, each for itself: right when the light changed between frames."
        ),
        Strategy::Average => format!(
            "Average balances all {count}, then gives every one the mean of their corrections: right on a series shot in one light, which should match."
        ),
    }
}

/// `AutoLevels.Batch.label`: where a run stands.
pub fn progress(done: usize, total: usize) -> String {
    format!("Reading {done} of {total}\u{2026}")
}

/// Only the open frame alone balances in memory; anything else runs through its sidecars.
pub fn needs_run(target_count: usize, target_is_open_frame: bool) -> bool {
    !(target_count == 1 && target_is_open_frame)
}

pub fn asks_choice(target_count: usize) -> bool {
    target_count > 1
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wording {
    pub title: String,
    pub detail: String,
    pub options: Vec<(Strategy, String)>,
}

/// The alert a press over several frames raises: the two strategies, and what ⌘Z cannot take back.
pub fn wording(count: usize) -> Wording {
    Wording {
        title: format!("{LABEL} on {count} photos"),
        detail: format!(
            "{}\n\n{}\n\n\u{2318}Z only takes back the photo that is open. The others are written straight to their sidecars. Framing is never touched.",
            explanation(Strategy::Each, count),
            explanation(Strategy::Average, count)
        ),
        options: STRATEGIES.iter().map(|&s| (s, label(s, count))).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AutoBalanceChoice.selfCheck`.
    #[test]
    fn only_the_open_frame_alone_skips_the_run_and_every_wording_carries_the_count() {
        assert!(!needs_run(1, true) && needs_run(1, false) && needs_run(12, true) && needs_run(12, false));
        assert!(!asks_choice(1) && asks_choice(2));
        let asked = wording(12);
        assert_eq!(asked.options.iter().map(|o| o.0).collect::<Vec<_>>(), STRATEGIES.to_vec());
        let everywhere: Vec<&String> = [&asked.title, &asked.detail].into_iter().chain(asked.options.iter().map(|o| &o.1)).collect();
        assert!(everywhere.iter().all(|t| t.contains("12")));
        // Twin: the count follows the target.
        let other = wording(7);
        assert!(![&other.title, &other.detail].iter().any(|t| t.contains("12")) && other.options.iter().all(|o| !o.1.contains("12")));
        assert!(asked.detail.contains("\u{2318}Z") && asked.detail.to_lowercase().contains("sidecar"));
        let labels: Vec<String> = STRATEGIES.iter().map(|&s| label(s, 36)).collect();
        assert!(labels.iter().all(|l| l.contains("36")) && labels[0] != labels[1]);
        let running = progress(7, 36);
        assert!(running.contains('7') && running.contains("36"));
    }
}
