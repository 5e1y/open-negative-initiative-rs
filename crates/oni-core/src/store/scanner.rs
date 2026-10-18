//! The Scanner face's words between the phone's link and the settings' fields.

use super::*;

pub fn channel(mode: ConversionMode) -> Channel {
    match mode {
        ConversionMode::Negative => Channel::Negative,
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
