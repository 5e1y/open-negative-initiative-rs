//! ISO 5-3's Status M densitometer (ANSI/ISO 5-3-1995, table 3), the reading a colour negative's data sheet prints: what
//! a virtual negative's bare film is read as, channel by channel.

/// Each channel's product, red, green and blue: the first wavelength, the log10 products every 10 nm to a 5.000 peak, and
/// the log slopes past either end.
const STATUS_M: [(f64, &[f64], f64, f64); 3] = [
    (620.0, &[2.109, 4.479, 5.000, 4.899, 4.578, 4.252, 3.875, 3.491, 3.099, 2.687, 2.269, 1.859, 1.449, 1.054, 0.654, 0.254], 0.260, -0.040),
    (470.0, &[1.152, 2.207, 3.156, 3.804, 4.272, 4.626, 4.872, 5.000, 4.995, 4.818, 4.458, 3.915, 3.172, 2.239, 1.070], 0.106, -0.120),
    (410.0, &[2.103, 4.111, 4.632, 4.871, 5.000, 4.955, 4.743, 4.343, 3.743, 2.990, 1.852], 0.250, -0.220),
];

/// Channel `c`'s product at `nm`, linear in log between the table's points, its peak one.
fn product(c: usize, nm: f64) -> f64 {
    let (first, logs, before, after) = STATUS_M[c];
    let last = first + 10.0 * (logs.len() - 1) as f64;
    let log = if nm < first {
        logs[0] + before * (nm - first)
    } else if nm >= last {
        logs[logs.len() - 1] + after * (nm - last)
    } else {
        let k = ((nm - first) / 10.0) as usize;
        let t = (nm - first) / 10.0 - k as f64;
        logs[k] + t * (logs[k + 1] - logs[k])
    };
    10f64.powf(log - 5.0)
}

/// The three densities Status M reads of a spectral density on the profile's grid, on the wavelengths it is known.
pub fn read(spectral: &[Option<f64>]) -> [f64; 3] {
    std::array::from_fn(|c| {
        let known = spectral.iter().enumerate().filter_map(|(l, d)| d.map(|d| (super::cie::FIRST_NM + super::cie::STEP_NM * l as f64, d)));
        let (seen, total) = known.fold((0.0, 0.0), |(s, t), (nm, d)| (s + product(c, nm) * 10f64.powf(-d), t + product(c, nm)));
        -(seen / total).log10()
    })
}
