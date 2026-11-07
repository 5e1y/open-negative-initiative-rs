//! `AutoBalanceChoice`'s rule: when a press over several frames asks for a strategy, and when it
//! runs through the sidecars; the words it shows are the app's, translated there.

use oni_core::balance::Strategy;

pub const STRATEGIES: [Strategy; 2] = [Strategy::Each, Strategy::Average];

/// Only the open frame alone balances in memory; anything else runs through its sidecars.
pub fn needs_run(target_count: usize, target_is_open_frame: bool) -> bool {
    !(target_count == 1 && target_is_open_frame)
}

pub fn asks_choice(target_count: usize) -> bool {
    target_count > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AutoBalanceChoice.selfCheck`'s rule.
    #[test]
    fn only_the_open_frame_alone_skips_the_run_and_only_several_ask() {
        assert!(!needs_run(1, true) && needs_run(1, false) && needs_run(12, true) && needs_run(12, false));
        // Twin: one frame asks nothing, two ask.
        assert!(!asks_choice(1) && asks_choice(2));
    }
}
