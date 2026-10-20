//! The editor's panel as the phone sees it: the face the open photo is edited on, and the sliders
//! the Roll, Color and Details tabs show on that face, each written as the desktop writes it.

use serde::{Deserialize, Serialize};

/// Pro's chain of sliders, or the Scanner's keypad; the desktop stores it in the photo's sidecar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Face {
    #[default]
    Pro,
    Scanner,
}

/// The phone's tabs, the desktop panel's without Crop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Roll,
    Color,
    Details,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Roll, Tab::Color, Tab::Details];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Roll => "Roll",
            Tab::Color => "Color",
            Tab::Details => "Details",
        }
    }
}

/// One slider of the panel. The wire carries a variant's index, so new ones are only appended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Param {
    Temperature,
    Tint,
    ColorDensity,
    Exposure,
    Highlights,
    Shadows,
    WhitePoint,
    BlackPoint,
    Contrast,
    Vignette,
    Chrome,
    BalanceStrength,
    DensityCeiling,
    DyeSeparation,
    SharpenAmount,
    SharpenRadius,
    SharpenHalo,
    ChromaAmount,
    ChromaRadius,
    Texture,
    PaperRed,
    PaperGreen,
    PaperBlue,
    GainRed,
    GainGreen,
    GainBlue,
    FlatAmount,
    FlatTemperature,
    FlatTint,
}

impl Param {
    pub const ALL: [Param; 29] = [
        Param::Temperature,
        Param::Tint,
        Param::ColorDensity,
        Param::Exposure,
        Param::Highlights,
        Param::Shadows,
        Param::WhitePoint,
        Param::BlackPoint,
        Param::Contrast,
        Param::Vignette,
        Param::Chrome,
        Param::BalanceStrength,
        Param::DensityCeiling,
        Param::DyeSeparation,
        Param::SharpenAmount,
        Param::SharpenRadius,
        Param::SharpenHalo,
        Param::ChromaAmount,
        Param::ChromaRadius,
        Param::Texture,
        Param::PaperRed,
        Param::PaperGreen,
        Param::PaperBlue,
        Param::GainRed,
        Param::GainGreen,
        Param::GainBlue,
        Param::FlatAmount,
        Param::FlatTemperature,
        Param::FlatTint,
    ];
}

/// A slider as the desktop shows it now: where it stands, under which block and words.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub param: Param,
    pub tab: Tab,
    /// The block's title, such as "Lighting".
    pub block: String,
    pub label: String,
    /// The readout, such as "+0.25 EV".
    pub value: String,
    /// The knob's place on its track, 0 at the left end and 1 at the right.
    pub position: f32,
    /// Off the value a double click on the slider returns it to.
    pub modified: bool,
}

/// The face and every slider the desktop's panel shows on it, in the panel's order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub face: Face,
    pub rows: Vec<Row>,
}

impl Panel {
    pub fn tab(&self, tab: Tab) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(move |row| row.tab == tab)
    }
}
