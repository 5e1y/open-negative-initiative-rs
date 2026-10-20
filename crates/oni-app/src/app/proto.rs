//! The Machine tabs: ONI and Frontier sent as the phone's own command, Proto written whole, as the
//! phone's protocol names two machines.

use crate::app::Ctl;
use oni_core::Command;
use oni_link::Command as Link;
use oni_settings::{PipelineSettings, ScannerMachine};
use std::rc::Rc;

/// The third tab.
pub const PROTO: i32 = 2;

impl Ctl {
    pub(in crate::app) fn machine_picked(self: &Rc<Self>, machine: i32) {
        if machine == PROTO {
            let next = PipelineSettings { scanner_machine: ScannerMachine::Oni, scanner_proto: true, ..self.state.borrow().store.settings().clone() };
            return self.write(Command::Write(Box::new(next)));
        }
        if let Some(machine) = super::at(&super::MACHINES, machine) {
            self.press(Link::Machine(machine));
        }
    }
}
