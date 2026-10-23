//! A preset's tint: the hue its card wears, read off what the preset does.

/// A fixed set of card colours: recognition across a shelf needs distinct hues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tint {
    #[default]
    Slate,
    Rust,
    Amber,
    Moss,
    Teal,
    Indigo,
    Plum,
    Rose,
}

/// Rec. 709 weights on linearised sRGB, for display legibility.
pub fn luminance(c: [f64; 3]) -> f64 {
    let linear = |v: f64| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
}

impl Tint {
    pub const ALL: [Tint; 8] =
        [Tint::Slate, Tint::Rust, Tint::Amber, Tint::Moss, Tint::Teal, Tint::Indigo, Tint::Plum, Tint::Rose];

    pub fn raw_value(self) -> &'static str {
        ["slate", "rust", "amber", "moss", "teal", "indigo", "plum", "rose"][self as usize]
    }

    pub fn from_raw(raw: &str) -> Option<Tint> {
        Tint::ALL.into_iter().find(|t| t.raw_value() == raw)
    }

    /// The swatch's name, the raw value capitalised, in the language spoken.
    pub fn label(self) -> String {
        use oni_i18n::marked;
        let names = [marked("Slate"), marked("Rust"), marked("Amber"), marked("Moss"), marked("Teal"), marked("Indigo"), marked("Plum"), marked("Rose")];
        oni_i18n::tr(names[self as usize]).to_string()
    }

    /// Components rather than a colour, so the ink is derived.
    pub fn components(self) -> [f64; 3] {
        match self {
            Tint::Slate => [0.38, 0.42, 0.48],
            Tint::Rust => [0.72, 0.34, 0.22],
            Tint::Amber => [0.87, 0.67, 0.20],
            Tint::Moss => [0.45, 0.58, 0.31],
            Tint::Teal => [0.20, 0.56, 0.56],
            Tint::Indigo => [0.35, 0.38, 0.72],
            Tint::Plum => [0.53, 0.31, 0.56],
            Tint::Rose => [0.83, 0.48, 0.55],
        }
    }

    /// Whether black reads better than white on the card, comparing the two contrasts.
    pub fn wants_dark_ink(self) -> bool {
        self.contrast_against(0.0) >= self.contrast_against(1.0)
    }

    pub(super) fn contrast_against(self, text: f64) -> f64 {
        let fill = luminance(self.components());
        let (high, low) = if fill > text { (fill, text) } else { (text, fill) };
        (high + 0.05) / (low + 0.05)
    }

    /// The contrast of the ink the card writes in.
    pub fn contrast(self) -> f64 {
        self.contrast_against(0.0).max(self.contrast_against(1.0))
    }
}
