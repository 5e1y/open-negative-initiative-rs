//! The editor's panel as Swift and Kotlin see it: the face, and each tab's sliders as the desktop
//! writes them, mirrored from `oni_link::panel`.

/// Pro's chain of sliders, or the Scanner's keypad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Face {
    Pro,
    Scanner,
}

/// `PanelTab` rather than `Tab`: Compose's `Tab` would shadow it in a Kotlin screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum PanelTab {
    Roll,
    Color,
    Details,
}

/// Mirrors `oni_link::Param` variant for variant, with the same names.
macro_rules! params {
    ($($name:ident),* $(,)?) => {
        /// One slider of the desktop's panel.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
        pub enum Param {
            $($name),*
        }

        impl From<Param> for oni_link::Param {
            fn from(param: Param) -> Self {
                match param {
                    $(Param::$name => oni_link::Param::$name),*
                }
            }
        }

        impl From<oni_link::Param> for Param {
            fn from(param: oni_link::Param) -> Self {
                match param {
                    $(oni_link::Param::$name => Param::$name),*
                }
            }
        }
    };
}

params!(
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
);

/// A slider as the desktop shows it now.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PanelRow {
    pub param: Param,
    pub tab: PanelTab,
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
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct Panel {
    pub face: Face,
    pub rows: Vec<PanelRow>,
}

impl From<Face> for oni_link::Face {
    fn from(face: Face) -> Self {
        match face {
            Face::Pro => oni_link::Face::Pro,
            Face::Scanner => oni_link::Face::Scanner,
        }
    }
}

impl From<oni_link::Face> for Face {
    fn from(face: oni_link::Face) -> Self {
        match face {
            oni_link::Face::Pro => Face::Pro,
            oni_link::Face::Scanner => Face::Scanner,
        }
    }
}

impl From<oni_link::Tab> for PanelTab {
    fn from(tab: oni_link::Tab) -> Self {
        match tab {
            oni_link::Tab::Roll => PanelTab::Roll,
            oni_link::Tab::Color => PanelTab::Color,
            oni_link::Tab::Details => PanelTab::Details,
        }
    }
}

impl From<&oni_link::Panel> for Panel {
    fn from(panel: &oni_link::Panel) -> Self {
        let rows = panel.rows.iter().map(|r| PanelRow {
            param: r.param.into(),
            tab: r.tab.into(),
            block: r.block.clone(),
            label: r.label.clone(),
            value: r.value.clone(),
            position: r.position,
            modified: r.modified,
        });
        Panel { face: panel.face.into(), rows: rows.collect() }
    }
}
