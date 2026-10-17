//! The open frame's undo stack, for the session only: snapshots of its settings with a cursor, so
//! an undo is a plain assignment, and the other frames a step wrote with it.

use oni_settings::PipelineSettings;
use std::collections::BTreeMap;

/// Below this interval, two changes to the same setting coalesce into one step.
pub const COALESCE_WINDOW: f64 = 0.4;
pub const LIMIT: usize = 200;

/// The other frames a step wrote, by path: what an undo lays back and a redo lays again.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Companion {
    pub before: BTreeMap<String, PipelineSettings>,
    pub after: BTreeMap<String, PipelineSettings>,
}

/// What changed between two states at the grain of a setting; `Several` never coalesces, and a
/// mode flip has no class, so it always falls there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Geometry,
    Chroma,
    Gains,
    Levels,
    Print,
    Saturation,
    Sharpen,
    Correction,
    Several,
}

impl Class {
    pub fn between(from: &PipelineSettings, to: &PipelineSettings) -> Class {
        let notes = [
            (Class::Geometry, from.geometry != to.geometry),
            (Class::Chroma, from.chroma != to.chroma),
            (Class::Gains, from.gains != to.gains),
            (Class::Levels, from.levels != to.levels),
            (Class::Print, from.print != to.print),
            (Class::Saturation, from.saturation != to.saturation),
            (Class::Sharpen, from.sharpen != to.sharpen),
            (Class::Correction, from.corrections != to.corrections),
        ];
        let mut found = None;
        for (class, changed) in notes {
            if changed {
                found = Some(if found.is_none() { class } else { Class::Several });
            }
        }
        found.unwrap_or(Class::Several)
    }
}

#[derive(Debug, Clone)]
pub struct History {
    entries: Vec<PipelineSettings>,
    cursor: usize,
    companions: Vec<Option<Companion>>,
    /// Each entry's serial survives coalescing, and a companion finds its step by it.
    serials: Vec<u64>,
    next_serial: u64,
    last: Option<(Class, f64)>,
}

impl Default for History {
    fn default() -> Self {
        History::new(PipelineSettings::default())
    }
}

impl History {
    pub fn new(initial: PipelineSettings) -> History {
        History { entries: vec![initial], cursor: 0, companions: vec![None], serials: vec![0], next_serial: 1, last: None }
    }

    pub fn current(&self) -> &PipelineSettings {
        &self.entries[self.cursor]
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len() - 1
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Records a state, merged into the previous one when it continues the same gesture; the serial
    /// of the step it lands in, `None` when nothing changed.
    pub fn record(&mut self, next: PipelineSettings, now: f64) -> Option<u64> {
        if next == *self.current() {
            return None;
        }
        let class = Class::between(self.current(), &next);
        // Merging only at the top keeps a gesture after an undo from rewriting a past state.
        let merges = self.cursor == self.entries.len() - 1
            && class != Class::Several
            && self.last.is_some_and(|(last, time)| last == class && now - time < COALESCE_WINDOW);
        if merges {
            self.entries[self.cursor] = next;
        } else {
            self.push(next, None);
        }
        self.last = Some((class, now));
        Some(self.serials[self.cursor])
    }

    /// A step of the other frames alone, the open one unchanged.
    pub fn lay(&mut self, companion: Companion) {
        let current = self.current().clone();
        self.push(current, Some(companion));
        self.last = None;
    }

    fn push(&mut self, state: PipelineSettings, companion: Option<Companion>) {
        self.entries.truncate(self.cursor + 1);
        self.companions.truncate(self.cursor + 1);
        self.serials.truncate(self.cursor + 1);
        self.entries.push(state);
        self.companions.push(companion);
        self.serials.push(self.next_serial);
        self.next_serial += 1;
        if self.entries.len() > LIMIT {
            let excess = self.entries.len() - LIMIT;
            self.entries.drain(..excess);
            self.companions.drain(..excess);
            self.serials.drain(..excess);
        }
        self.cursor = self.entries.len() - 1;
    }

    /// Lays other frames' writes on step `serial`, merged with any there: the first before is
    /// kept, the last after taken.
    pub fn attach(&mut self, companion: Companion, serial: u64) {
        let Some(index) = self.serials.iter().position(|s| *s == serial) else { return };
        match &mut self.companions[index] {
            None => self.companions[index] = Some(companion),
            Some(held) => {
                for (path, before) in companion.before {
                    held.before.entry(path).or_insert(before);
                }
                held.after.extend(companion.after);
            }
        }
    }

    pub fn undo_companion(&self) -> Option<&Companion> {
        self.companions[self.cursor].as_ref()
    }

    pub fn redo_companion(&self) -> Option<&Companion> {
        if self.can_redo() { self.companions[self.cursor + 1].as_ref() } else { None }
    }

    pub fn undo(&mut self) -> Option<PipelineSettings> {
        if !self.can_undo() {
            return None;
        }
        self.cursor -= 1;
        // Cuts coalescing, so the next gesture cannot merge with the state just left.
        self.last = None;
        Some(self.current().clone())
    }

    pub fn redo(&mut self) -> Option<PipelineSettings> {
        if !self.can_redo() {
            return None;
        }
        self.cursor += 1;
        self.last = None;
        Some(self.current().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exposed(by: f32) -> PipelineSettings {
        let mut s = PipelineSettings::default();
        s.print.exposure += by;
        s
    }

    #[test]
    fn a_drag_within_the_window_is_one_step() {
        let mut history = History::default();
        for (i, at) in [0.0, 0.1, 0.2, 0.35].into_iter().enumerate() {
            history.record(exposed(0.1 * (i + 1) as f32), at);
        }
        assert_eq!(history.len(), 2);
        assert_eq!(history.undo(), Some(PipelineSettings::default()));
        // Twin: the same notches spaced past the window are four steps.
        let mut spaced = History::default();
        for (i, at) in [0.0, 0.5, 1.0, 1.5].into_iter().enumerate() {
            spaced.record(exposed(0.1 * (i + 1) as f32), at);
        }
        assert_eq!(spaced.len(), 5);
    }

    #[test]
    fn two_settings_at_once_never_coalesce() {
        let mut history = History::default();
        let mut both = exposed(0.2);
        both.sharpen.amount = 1.0;
        history.record(both.clone(), 0.0);
        let mut again = both.clone();
        again.print.exposure += 0.1;
        again.sharpen.amount = 1.2;
        history.record(again, 0.1);
        assert_eq!(history.len(), 3);
        // Twin: one setting twice in the window merges.
        let mut one = History::default();
        one.record(exposed(0.2), 0.0);
        one.record(exposed(0.3), 0.1);
        assert_eq!(one.len(), 2);
    }

    #[test]
    fn a_gesture_after_an_undo_drops_the_redo_branch_and_never_merges_into_the_past() {
        let mut history = History::default();
        history.record(exposed(0.1), 0.0);
        history.record(exposed(0.2), 1.0);
        history.undo();
        history.record(exposed(0.5), 1.1);
        assert!(!history.can_redo());
        assert_eq!(history.current(), &exposed(0.5));
        assert_eq!(history.undo(), Some(exposed(0.1)));
        // Twin: without the undo the same notch within the window merges into the top.
        let mut kept = History::default();
        kept.record(exposed(0.1), 0.0);
        kept.record(exposed(0.2), 1.0);
        kept.record(exposed(0.5), 1.1);
        assert_eq!(kept.len(), 3);
    }

    #[test]
    fn a_companion_survives_coalescing_and_keeps_its_first_before() {
        let mut history = History::default();
        let serial = history.record(exposed(0.1), 0.0).unwrap();
        let a = |by| BTreeMap::from([("a".to_string(), exposed(by))]);
        history.attach(Companion { before: a(0.0), after: a(0.1) }, serial);
        assert_eq!(history.record(exposed(0.2), 0.1), Some(serial));
        history.attach(Companion { before: a(0.1), after: a(0.2) }, serial);
        let held = history.undo_companion().unwrap();
        assert_eq!((held.before["a"].clone(), held.after["a"].clone()), (exposed(0.0), exposed(0.2)));
        // Twin: a serial no step carries attaches nothing.
        history.attach(Companion { before: a(9.0), after: a(9.0) }, serial + 40);
        assert_eq!(history.undo_companion().unwrap().after["a"], exposed(0.2));
    }

    #[test]
    fn the_stack_holds_its_limit() {
        let mut history = History::default();
        for i in 0..(LIMIT + 50) {
            history.record(exposed(i as f32 + 1.0), i as f64);
        }
        assert_eq!(history.len(), LIMIT);
        // Twin: one under the limit keeps every state.
        let mut short = History::default();
        for i in 0..(LIMIT - 2) {
            short.record(exposed(i as f32 + 1.0), i as f64);
        }
        assert_eq!(short.len(), LIMIT - 1);
    }
}
