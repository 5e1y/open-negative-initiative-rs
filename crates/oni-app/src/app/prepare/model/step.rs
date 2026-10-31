//! The surface's steps: their order, titles and details, and which ones a roll walks.

use super::*;

/// `RollStep`, in the order the panel walks them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Film,
    FlatField,
    Base,
    Flags,
    Window,
}

impl Step {
    pub const ALL: [Step; 5] = [Step::Film, Step::FlatField, Step::Base, Step::Flags, Step::Window];

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn title(self) -> &'static str {
        match self {
            Step::Film => tr("Select the Film Type"),
            Step::FlatField => tr("Set Up the Flat-Field"),
            Step::Base => tr("Pick the Film Base"),
            Step::Flags => tr("Flag the Frames Left Out"),
            Step::Window => tr("Place the Measuring Frame"),
        }
    }

    pub fn detail(self, film: ConversionMode) -> &'static str {
        match self {
            Step::Window if film == ConversionMode::Positive => tr("Draw the frame inside the picture, clear of the holder and the film's edges, and use the slider to check it on every scan. The roll's gains are read on what this frame holds in every unflagged slide."),
            Step::Flags if film == ConversionMode::Positive => tr("Flag slides whose colours stand apart: a sunset, a lit stage, one colour filling the frame. They still take the roll's gains, but left unflagged, one of them can tint the whole roll."),
            Step::Film => tr("Choose the film and what you digitised it with. The wrong film type converts every frame of the roll wrong."),
            Step::FlatField => tr("Click the shot of the empty light panel, or pick one you loaded before. Skip it if your light is even; otherwise uneven light darkens or tints the corners of every frame."),
            Step::Base => tr("Choose a scan, then click on bare film: the border or the gap between two frames. A click on the picture instead shifts the blacks and colours of the whole roll."),
            Step::Flags => tr("Flag light leaks, the cut first frame and extreme shots: they are still converted, but left unflagged, one of them can wash out or tint the whole roll."),
            Step::Window => tr("Draw the frame inside the picture, clear of the holder and the film's edges, and use the slider to check it on every scan. An edge caught inside skews the whole roll."),
        }
    }

    /// The name a draft stores, the Swift case's, so a step added later moves no stored one.
    pub fn name(self) -> &'static str {
        match self {
            Step::Film => "film",
            Step::FlatField => "flatField",
            Step::Base => "base",
            Step::Flags => "flags",
            Step::Window => "window",
        }
    }

    pub fn named(name: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|s| s.name() == name)
    }

    /// A scanner lights its film evenly: it needs no flat-field. A slide has no film base to pick,
    /// and its flags keep an odd slide out of the roll's gains (owner's call).
    pub fn skipped(self, capture: Capture, film: ConversionMode) -> bool {
        match self {
            Step::FlatField => capture == Capture::Scanner,
            Step::Base => film == ConversionMode::Positive,
            Step::Film | Step::Flags | Step::Window => false,
        }
    }
}

/// `RollPanel.neighbour`: one step either way past the skipped ones, held at either end.
pub fn neighbour(step: Step, way: i32, capture: Capture, film: ConversionMode) -> Step {
    let at = |i: i32| usize::try_from(i).ok().and_then(|i| Step::ALL.get(i).copied());
    let mut next = at(step.index() as i32 + way).unwrap_or(step);
    while next.skipped(capture, film)
        && let Some(further) = at(next.index() as i32 + way)
    {
        next = further;
    }
    if next.skipped(capture, film) { step } else { next }
}

/// `RollPanel.stepFilled`: NEXT once the step holds its value, SKIP before.
pub fn filled(step: Step, flat: &FlatField, base: bool) -> bool {
    match step {
        Step::FlatField => !flat.is_neutral(),
        Step::Base => base,
        _ => true,
    }
}
