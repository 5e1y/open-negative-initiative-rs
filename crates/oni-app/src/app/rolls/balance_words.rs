//! What Auto White Balance over several frames asks and shows, in the reader's language: each
//! strategy carrying the count, and what ⌘Z cannot take back.

use oni_core::balance::Strategy;
use oni_i18n::tr;

/// A strategy's key.
pub(super) fn label(strategy: Strategy, count: usize) -> String {
    match strategy {
        Strategy::Each => tr!("Balance Each of the {count}", count = count),
        Strategy::Average => tr!("Average the {count} Balances", count = count),
    }
}

/// The question's title and its words.
pub(super) fn asked(count: usize) -> (String, String) {
    let title = tr!("Auto White Balance on {count} photos", count = count);
    let detail = tr!(
        "Balance Each corrects the colour of all {count} photos, each for itself: right when the light changed between frames.\n\nAverage balances all {count}, then gives every one the mean of their corrections: right on a series shot in one light, which should match.\n\n\u{2318}Z only takes back the photo that is open; the others keep their new balance. The crop is never touched.",
        count = count
    );
    (title, detail)
}

/// Where a run stands, on the key.
pub(super) fn progress(done: usize, total: usize) -> String {
    tr!("Reading {done} of {total}\u{2026}", done = done, total = total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AutoBalanceChoice.selfCheck`: every word carries the count, the two keys apart.
    #[test]
    fn every_wording_carries_the_count() {
        let (title, detail) = asked(12);
        let keys = [label(Strategy::Each, 12), label(Strategy::Average, 12)];
        assert!([&title, &detail].into_iter().chain(&keys).all(|t| t.contains("12")) && keys[0] != keys[1]);
        assert!(detail.contains("\u{2318}Z") && detail.contains("crop"));
        // Twin: the count follows the target.
        let (title, detail) = asked(7);
        assert!(![title, detail, label(Strategy::Each, 7), label(Strategy::Average, 7)].iter().any(|t| t.contains("12")));
        let running = progress(7, 36);
        assert!(running.contains('7') && running.contains("36"));
    }
}
