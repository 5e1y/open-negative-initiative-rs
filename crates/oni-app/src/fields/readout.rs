//! The one way the app writes a value on screen: a true minus, a plus where the value is signed, no
//! sign on what rounds to zero, and its unit after it, `%`, `°` and `×` set against the digits.

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
    pub fn format(self, v: f32) -> String {
        let v = f64::from(v);
        match self {
            Readout::Stops => number(v, 2, true, Unit::Stops),
            Readout::SignedPercent => number(v * 100.0, 0, true, Unit::Percent),
            Readout::Percent => number(v * 100.0, 0, false, Unit::Percent),
            Readout::Ceiling => number(v, 3, false, Unit::Bare),
            Readout::Degrees => number(v, 2, true, Unit::Degrees),
            Readout::Times => number(v, 2, false, Unit::Times),
            Readout::SignedTimes => number(v, 2, true, Unit::Times),
            Readout::Pixels => number(v, 1, false, Unit::Pixels),
        }
    }
}
