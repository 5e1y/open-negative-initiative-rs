//! The Frontier as a hand's own look: a standard `.cube` on ONI's inversion, display sRGB code in and
//! out, standing in for the measured table until the hand returns to it.

use oni_render::{ColorCube, cpu};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

static TABLE: RwLock<Option<(u64, Arc<ColorCube>)>> = RwLock::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The hand's table, or `None` for the measured one; each change takes a new generation, so a
/// render never mixes a table uploaded before it with one after.
pub fn set(cube: Option<ColorCube>) {
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut table) = TABLE.write() {
        *table = cube.map(|c| (generation, Arc::new(c)));
    }
}

/// The hand's table and its generation, while one stands in.
pub fn current() -> Option<(u64, Arc<ColorCube>)> {
    TABLE.read().ok().and_then(|t| t.clone())
}

/// Says what the file's axes are: an ordinary image LUT on ONI's inverted render.
const HEADER: &str = "# ONI Frontier look, applied to ONI's inversion (the Pro render).\n# Input and output: display sRGB code, 0 to 1. Any LUT tool edits it.\n";

/// The nodes a starting look is laid on.
pub const NODES: usize = 33;

/// A look to start from: ONI's own scanner look, read on display code rather than its square root.
pub fn starting() -> ColorCube {
    let oni = &*crate::oni_look::CUBE;
    let step = 1.0 / (NODES - 1) as f32;
    let mut cube_data = Vec::with_capacity(NODES * NODES * NODES);
    for b in 0..NODES {
        for g in 0..NODES {
            for r in 0..NODES {
                let code = [r, g, b].map(|i| (i as f32 * step).powf(1.0 / crate::oni_look::ROOT));
                cube_data.push(cpu::color_cube([code[0], code[1], code[2], 1.0], oni));
            }
        }
    }
    ColorCube { cube_dimension: NODES, cube_data }
}

/// The table as an Adobe `.cube`: red the fastest axis, as the table itself is laid.
pub fn write(cube: &ColorCube) -> String {
    let mut out = format!("{HEADER}TITLE \"ONI Frontier\"\nLUT_3D_SIZE {}\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n", cube.cube_dimension);
    for node in &cube.cube_data {
        out.push_str(&format!("{:.6} {:.6} {:.6}\n", node[0], node[1], node[2]));
    }
    out
}

/// A 3D `.cube` read back: its size, then its nodes in order; anything else is said, never guessed.
pub fn read(text: &str) -> Result<ColorCube, String> {
    let mut size = None;
    let mut nodes = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("TITLE") || line.starts_with("DOMAIN_") {
            continue;
        }
        if let Some(n) = line.strip_prefix("LUT_3D_SIZE") {
            size = Some(n.trim().parse::<usize>().map_err(|_| format!("line {}: the size reads not", number + 1))?);
            continue;
        }
        if line.starts_with("LUT_1D_SIZE") {
            return Err("a 1D table cannot stand in for the Frontier's 3D one".into());
        }
        let values: Vec<f32> = line.split_whitespace().map(str::parse).collect::<Result<_, _>>().map_err(|_| format!("line {}: not three numbers", number + 1))?;
        let [r, g, b] = values[..] else { return Err(format!("line {}: not three numbers", number + 1)) };
        if ![r, g, b].iter().all(|v| v.is_finite()) {
            return Err(format!("line {}: a value is not finite", number + 1));
        }
        nodes.push([r, g, b, 1.0]);
    }
    let n = size.ok_or("the file names no LUT_3D_SIZE")?;
    if !(2..=65).contains(&n) {
        return Err(format!("a size of {n} is outside 2 to 65"));
    }
    if nodes.len() != n * n * n {
        return Err(format!("{} nodes where a size of {n} needs {}", nodes.len(), n * n * n));
    }
    Ok(ColorCube { cube_dimension: n, cube_data: nodes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_measured_table_crosses_a_cube_file_and_comes_back() {
        let measured = &crate::scanner::MACHINE.render;
        let back = read(&write(measured)).unwrap();
        assert_eq!(back.cube_dimension, measured.cube_dimension);
        let worst = back.cube_data.iter().zip(&measured.cube_data).flat_map(|(a, b)| (0..3).map(move |c| (a[c] - b[c]).abs())).fold(0.0f32, f32::max);
        assert!(worst < 1e-6, "{worst}");
        // Twin: a table missing a node is refused, not padded.
        let text = write(measured);
        let short = text.rsplit_once('\n').map(|(head, _)| head).unwrap().rsplit_once('\n').unwrap().0;
        assert!(read(short).is_err());
        assert!(read("LUT_1D_SIZE 4\n0 0 0\n").is_err());
        assert!(read("LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\nnan 1 1\n").is_err());
    }

    #[test]
    fn the_starting_look_renders_as_onis_own() {
        let start = starting();
        let oni = &*crate::oni_look::CUBE;
        // A node of the start is ONI's look at that code, read through its square-root axes.
        for &(r, g, b) in &[(0, 0, 0), (16, 8, 24), (32, 32, 32), (5, 30, 12)] {
            let code = [r, g, b].map(|i| i as f32 / (NODES - 1) as f32);
            let expected = cpu::color_cube([code[0].sqrt(), code[1].sqrt(), code[2].sqrt(), 1.0], oni);
            let got = start.cube_data[(b * NODES + g) * NODES + r];
            assert!((0..3).all(|c| (got[c] - expected[c]).abs() < 1e-6), "{got:?} {expected:?}");
        }
        // Twin: read on the code itself, ONI's cube gives another colour in the mid-tones.
        let mid = [0.5f32, 0.4, 0.3];
        let wrong = cpu::color_cube([mid[0], mid[1], mid[2], 1.0], oni);
        let right = cpu::color_cube([mid[0].sqrt(), mid[1].sqrt(), mid[2].sqrt(), 1.0], oni);
        assert!((0..3).any(|c| (wrong[c] - right[c]).abs() > 0.02));
    }

    #[test]
    fn a_new_table_takes_a_new_generation_and_none_returns_to_the_measured() {
        let cube = read("LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n").unwrap();
        set(Some(cube.clone()));
        let first = current().unwrap().0;
        set(Some(cube));
        assert!(current().unwrap().0 > first, "an edit saved again is another table");
        set(None);
        assert!(current().is_none());
    }
}
