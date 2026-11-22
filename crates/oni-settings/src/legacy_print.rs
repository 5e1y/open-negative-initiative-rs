//! The print a photo edited on the former Scanner face keeps: its machine's look over the print, its keypad's steps in
//! the gains and TEMP's warmth, rendered as the face rendered them, for no photo to lose its edit.

use serde::{Deserialize, Serialize};

/// The machine whose look the Scanner face laid over ONI's print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LegacyMachine {
    #[default]
    Oni,
    Frontier,
}

/// The keypad's steps on D, C, M and Y and TEMP's, a step half the point the keypad's screen read, and the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyPrint {
    pub machine: LegacyMachine,
    pub d: i64,
    pub c: i64,
    pub m: i64,
    pub y: i64,
    pub w: i64,
    /// Off, the photo prints as its other settings say, the legacy print kept to be laid again.
    #[serde(skip_serializing_if = "is_on")]
    pub laid: bool,
}

impl Default for LegacyPrint {
    fn default() -> Self {
        LegacyPrint { machine: LegacyMachine::Oni, d: 0, c: 0, m: 0, y: 0, w: 0, laid: true }
    }
}

fn is_on(laid: &bool) -> bool {
    *laid
}

impl LegacyPrint {
    /// Steps in one point of the keypad's screen.
    pub const PER_POINT: i64 = 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legacy_print_writes_its_steps_and_off_alone() {
        let laid = LegacyPrint { machine: LegacyMachine::Frontier, d: 3, c: -1, m: 0, y: 4, w: -2, laid: true };
        let written = serde_json::to_value(laid).unwrap();
        assert_eq!(written, serde_json::json!({"machine": "frontier", "d": 3, "c": -1, "m": 0, "y": 4, "w": -2}));
        assert_eq!(serde_json::from_value::<LegacyPrint>(written).unwrap(), laid);
        let off = LegacyPrint { laid: false, ..laid };
        assert_eq!(serde_json::to_value(off).unwrap()["laid"], serde_json::json!(false));
        // Twin: a print read with no key is laid, on the ONI machine, its keys at rest.
        let bare: LegacyPrint = serde_json::from_str("{}").unwrap();
        assert_eq!(bare, LegacyPrint::default());
        assert!(bare.laid && bare.machine == LegacyMachine::Oni && bare != laid);
    }
}
