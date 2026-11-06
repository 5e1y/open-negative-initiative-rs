//! The one way the app writes a value on screen: a true minus, a plus where the value is signed, no
//! sign on what rounds to zero, and its unit after it, `%`, `°` and `×` set against the digits.

use oni_i18n::tr;

/// What a readout's number counts, written after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Bare,
    Percent,
    Stops,
    Degrees,
    Times,
    Pixels,
}

impl Unit {
    fn symbol(self) -> &'static str {
        match self {
            Unit::Bare => "",
            Unit::Percent => "%",
            Unit::Stops => " EV",
            Unit::Degrees => "°",
            Unit::Times => "×",
            Unit::Pixels => " px",
        }
    }
}

/// `value` at `places` decimals in `unit`: U+2212 before a negative, `+` before a positive when
/// `signed`, nothing before what rounds to zero, so a value at rest never reads "−0".
pub fn number(value: f64, places: usize, signed: bool, unit: Unit) -> String {
    let digits = format!("{:.*}", places, value.abs());
    let sign = if digits.bytes().all(|b| b == b'0' || b == b'.') {
        ""
    } else if value < 0.0 {
        "\u{2212}"
    } else if signed {
        "+"
    } else {
        ""
    };
    format!("{sign}{digits}{}", unit.symbol())
}

/// The unit a readout ends on, all it holds past its last digit, set in a column of its own so the
/// digits of every row end at one edge: " EV", "%", "°", nothing for a bare number or a word.
pub fn unit_of(readout: &str) -> &str {
    readout.rfind(|c: char| c.is_ascii_digit()).map_or("", |at| &readout[at + 1..])
}

/// The colour axis a slider's track runs along: its ends say which way a value leans, which the
/// number's own unit, stops or percent, leaves a photographer to guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Blue to yellow, cool to warm.
    Warmth,
    /// Green to magenta.
    Tint,
}

impl Axis {
    /// `text`, a signed readout, after the end its sign leans to; unsigned, at rest, it stands alone.
    /// The word goes first, so the number holds its place at the readout's right edge.
    pub fn named(self, text: String) -> String {
        let (low, high) = match self {
            Axis::Warmth => (tr("Cool"), tr("Warm")),
            Axis::Tint => (tr("Green"), tr("Magenta")),
        };
        match text.chars().next() {
            Some('+') => format!("{high} {text}"),
            Some('\u{2212}') => format!("{low} {text}"),
            _ => text,
        }
    }
}

/// How a slider writes its stored value, each through `number`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Readout {
    /// Stops, signed, two places.
    Stops,
    /// The value times 100, in percent, signed.
    SignedPercent,
    /// The value times 100, in percent.
    Percent,
    /// Three places.
    Ceiling,
    /// Degrees, signed, two places.
    Degrees,
    /// A factor, two places.
    Times,
    /// A factor, signed, two places.
    SignedTimes,
    /// Pixels, one place.
    Pixels,
}

impl Readout {
    /// The number `format` writes for `v`, before its rounding: a percent's is a hundred times it.
    pub fn scaled(self, v: f32) -> f64 {
        let v = f64::from(v);
        match self {
            Readout::SignedPercent | Readout::Percent => v * 100.0,
            _ => v,
        }
    }

    /// `format`, after the end of `axis` the value leans to.
    pub fn along(self, axis: Option<Axis>, v: f32) -> String {
        let text = self.format(v);
        match axis {
            Some(axis) => axis.named(text),
            None => text,
        }
    }

    pub fn format(self, v: f32) -> String {
        let n = self.scaled(v);
        match self {
            Readout::Stops => number(n, 2, true, Unit::Stops),
            Readout::SignedPercent => number(n, 0, true, Unit::Percent),
            Readout::Percent => number(n, 0, false, Unit::Percent),
            Readout::Ceiling => number(n, 3, false, Unit::Bare),
            Readout::Degrees => number(n, 2, true, Unit::Degrees),
            Readout::Times => number(n, 2, false, Unit::Times),
            Readout::SignedTimes => number(n, 2, true, Unit::Times),
            Readout::Pixels => number(n, 1, false, Unit::Pixels),
        }
    }
}
