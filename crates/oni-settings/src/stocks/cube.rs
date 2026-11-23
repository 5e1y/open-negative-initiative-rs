//! A 3D LUT as a `.cube` file holds it: a digital photo's look read in display sRGB code and written back in it, its
//! nodes red fastest, as the format lays them.

use std::sync::Arc;

/// The nodes a look is read through: `size` per channel, red fastest, then green, then blue.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub size: usize,
    pub nodes: Arc<[[f32; 4]]>,
}

/// The sides a cube may have: two nodes are a cube, past 128 a table outgrows a texture's side.
const SIZES: std::ops::RangeInclusive<usize> = 2..=128;

/// The title and the nodes `bytes` hold, or why they are refused: no size, a 1D table, a domain other than 0 to 1, a
/// node missing, extra or not a number.
pub fn read(bytes: &[u8]) -> Result<(Option<String>, Look), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not text".to_string())?;
    let (mut title, mut size, mut nodes) = (None, None, Vec::new());
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "TITLE" => title = Some(rest.trim_matches('"').to_string()).filter(|t| !t.is_empty()),
            "LUT_3D_SIZE" => size = Some(rest.parse::<usize>().ok().filter(|n| SIZES.contains(n)).ok_or_else(|| format!("its size \"{rest}\" is not 2 to 128"))?),
            "LUT_1D_SIZE" => return Err("it is a 1D table, not a 3D one".into()),
            "DOMAIN_MIN" | "DOMAIN_MAX" => {
                let end = if word == "DOMAIN_MIN" { 0.0 } else { 1.0 };
                if numbers(rest).ok().is_none_or(|v| v.len() != 3 || v.iter().any(|&x| x != end)) {
                    return Err(format!("its {word} is not {end} {end} {end}"));
                }
            }
            "LUT_3D_INPUT_RANGE" => return Err("its input range is not 0 to 1".into()),
            _ => {
                let v = numbers(line).map_err(|_| format!("the line \"{line}\" is neither a keyword nor a node"))?;
                let [r, g, b] = v[..] else { return Err(format!("the node \"{line}\" holds {} numbers, not 3", v.len())) };
                nodes.push([r, g, b, 1.0]);
            }
        }
    }
    let size = size.ok_or("it gives no LUT_3D_SIZE")?;
    if nodes.len() != size.pow(3) {
        return Err(format!("it holds {} nodes where a size of {size} needs {}", nodes.len(), size.pow(3)));
    }
    Ok((title, Look { size, nodes: nodes.into() }))
}

fn numbers(text: &str) -> Result<Vec<f32>, std::num::ParseFloatError> {
    text.split_whitespace().map(|w| w.parse::<f32>().and_then(|x| if x.is_finite() { Ok(x) } else { "x".parse() })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(size: usize, header: &str) -> String {
        let mut out = format!("# a comment\n{header}LUT_3D_SIZE {size}\n");
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let at = |v: usize| v as f32 / (size - 1) as f32;
                    out += &format!("{} {} {}\n", at(r), at(g), at(b));
                }
            }
        }
        out
    }

    #[test]
    fn a_cube_reads_its_title_and_nodes_red_fastest() {
        let (title, look) = read(cube(3, "TITLE \"Portra\"\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1.0 1.0 1.0\n").as_bytes()).unwrap();
        assert_eq!((title.as_deref(), look.size, look.nodes.len()), (Some("Portra"), 3, 27));
        assert_eq!((look.nodes[1], look.nodes[3], look.nodes[9]), ([0.5, 0.0, 0.0, 1.0], [0.0, 0.5, 0.0, 1.0], [0.0, 0.0, 0.5, 1.0]));
        assert_eq!(read(cube(2, "").as_bytes()).unwrap().0, None);
    }

    #[test]
    fn a_cube_that_cannot_be_read_whole_is_refused_with_why() {
        let refused = |text: &str| read(text.as_bytes()).unwrap_err();
        assert!(refused(&cube(3, "")[..cube(3, "").len() - 6]).contains("26 nodes"));
        assert!(refused(&(cube(2, "") + "1 1 1\n")).contains("9 nodes"));
        assert!(refused(&cube(2, "DOMAIN_MAX 2 2 2\n")).contains("DOMAIN_MAX"));
        assert!(refused("LUT_1D_SIZE 4\n0\n").contains("1D"));
        assert!(refused("0 0 0\n").contains("LUT_3D_SIZE"));
        assert!(refused(&cube(2, "").replace("0 0 0\n", "0 nan 0\n")).contains("neither"));
        assert!(refused("LUT_3D_SIZE 1\n0 0 0\n").contains("2 to 128"));
    }
}
