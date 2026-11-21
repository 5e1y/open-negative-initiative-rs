//! The face the frames are seen on, and the Scanner face's words between the phone's link and the
//! settings' fields.

use super::*;

impl EditorStore {
    /// The face turned, a view and never a step: the step under way is recorded on its own face, then
    /// the open frame and those together show the new one, written as an opening writes it.
    pub(super) fn turn(&mut self, face: Face) -> FieldSet {
        self.settle_under_way();
        self.scanner_face = face == Face::Scanner;
        if self.open.is_none() || self.inert() {
            return FieldSet::EMPTY;
        }
        self.recorded.face = face;
        let changed = self.replace(PipelineSettings { face, ..self.settings.clone() });
        if !changed.is_empty() {
            self.last_change = Some(self.now());
        }
        let together = self.together().to_vec();
        self.modify(&together, |_, s| Some(PipelineSettings { face, ..s.clone() }));
        changed
    }
}

/// Whether the Scanner's screen asks the roll to be measured again: a film's reference kept no median,
/// which the machine's window is laid on; a digital photo is printed on its film's reference.
pub fn remeasure(s: &PipelineSettings) -> bool {
    s.mode.is_measured() && s.roll_reference.as_ref().is_some_and(|r| r.median.is_none())
}

pub fn channel(mode: ConversionMode) -> Channel {
    match mode {
        // The phone's three channels: a digital photo's negative moves as a negative does.
        ConversionMode::Negative | ConversionMode::Digital => Channel::Negative,
        ConversionMode::Positive => Channel::Positive,
        ConversionMode::Monochrome => Channel::Monochrome,
    }
}

pub fn machine_of(machine: Machine) -> ScannerMachine {
    match machine {
        Machine::Oni => ScannerMachine::Oni,
        Machine::Frontier => ScannerMachine::Frontier,
    }
}

pub fn machine_for(machine: ScannerMachine) -> Machine {
    match machine {
        ScannerMachine::Oni => Machine::Oni,
        ScannerMachine::Frontier => Machine::Frontier,
    }
}
