//! The three files of a frame exposed under red, green and blue light, found through the lead's
//! sidecar, so a gesture on a frame moves or trashes the three together.

use crate::paths;
use oni_settings::Date;
use oni_settings::frame::{PassOrder, RgbPasses};
use oni_settings::sidecar;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

/// `RGBPasses.readSide`: the long side a pass is decoded at to read its shares.
pub const READ_SIDE: f64 = 360.0;

/// `RGBPasses.shares`: a pass's light per channel over their sum, all a click on RGB Passes reads;
/// `None` for a black or non-finite pass.
pub fn shares(pixels: &[[f32; 4]]) -> Option<[f64; 3]> {
    if pixels.is_empty() {
        return None;
    }
    let mut sum = [0.0f64; 3];
    for p in pixels {
        for c in 0..3 {
            sum[c] += f64::from(p[c]);
        }
    }
    let rgb = sum.map(|v| v / pixels.len() as f64);
    let total = rgb[0] + rgb[1] + rgb[2];
    (total > 0.0 && total.is_finite()).then(|| rgb.map(|v| v / total))
}

/// One frame of a grouping: its lead's path and the triplet it names.
#[derive(Clone, Debug, PartialEq)]
pub struct Grouped {
    pub lead: String,
    pub passes: RgbPasses,
}

/// `RGBPasses.Grouping`: the frames, the files left over, and the leads whose passes lie further
/// apart than the frame lies from its neighbours.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grouping {
    pub frames: Vec<Grouped>,
    pub left_over: Vec<String>,
    pub stretched: HashSet<String>,
}

/// `RGBPasses.grouped`: three consecutive files per frame from `offset`, the first being the lead;
/// a triplet whose files are not in one folder is left out, its passes being found beside the lead.
pub fn grouped(files: &[(String, Option<Date>)], order: PassOrder, offset: usize) -> Grouping {
    let mut grouping = Grouping::default();
    let start = offset.min(files.len());
    grouping.left_over = files[..start].iter().map(|f| f.0.clone()).collect();
    let mut index = start;
    let mut chunks = Vec::new();
    while index + 3 <= files.len() {
        chunks.push(&files[index..index + 3]);
        index += 3;
    }
    grouping.left_over.extend(files[index..].iter().map(|f| f.0.clone()));
    let mut kept = Vec::new();
    for chunk in chunks {
        let folder = paths::parent(&chunk[0].0);
        if !chunk.iter().all(|f| paths::parent(&f.0) == folder) {
            grouping.left_over.extend(chunk.iter().map(|f| f.0.clone()));
            continue;
        }
        let mut names = [String::new(), String::new(), String::new()];
        for (file, channel) in chunk.iter().zip(order.channels()) {
            names[channel.index()] = paths::name(&file.0);
        }
        let [red, green, blue] = names;
        grouping.frames.push(Grouped { lead: chunk[0].0.clone(), passes: RgbPasses { red, green, blue, registration: None } });
        kept.push(chunk);
    }
    // A pause between two passes longer than the pause to a neighbouring frame says a pass is
    // missing or extra; it is said, never fixed.
    let seconds = |d: Date| d.time_interval_since_reference_date();
    for (i, chunk) in kept.iter().enumerate() {
        let (Some(first), Some(last)) = (chunk[0].1, chunk[2].1) else { continue };
        let span = seconds(last) - seconds(first);
        let mut gaps = Vec::new();
        if i > 0
            && let Some(previous) = kept[i - 1][2].1
        {
            gaps.push(seconds(first) - seconds(previous));
        }
        if let Some(next) = kept.get(i + 1).and_then(|c| c[0].1) {
            gaps.push(seconds(next) - seconds(last));
        }
        if gaps.into_iter().min_by(f64::total_cmp).is_some_and(|nearest| span > nearest) {
            grouping.stretched.insert(chunk[0].0.clone());
        }
    }
    grouping
}

/// Triplets a roll being prepared reads before its measure writes them: every decode of a lead
/// consults them first, and Prepare the Roll lets them go when it closes.
static HELD: Mutex<Option<HashMap<String, RgbPasses>>> = Mutex::new(None);

fn with_held<T>(f: impl FnOnce(&mut HashMap<String, RgbPasses>) -> T) -> T {
    let mut held = HELD.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(held.get_or_insert_with(HashMap::new))
}

/// `hold(_ frames:)`: the grouping's triplets, replacing any held before.
pub fn hold(frames: HashMap<String, RgbPasses>) {
    with_held(|h| *h = frames);
}

/// `hold(_:_:)`: one frame's triplet held as its measure registers it.
pub fn hold_one(lead: &str, passes: RgbPasses) {
    with_held(|h| {
        h.insert(lead.to_string(), passes);
    });
}

pub fn release() {
    hold(HashMap::new());
}

/// The triplet held for `lead`, whatever its sidecar says.
pub fn held(lead: &str) -> Option<RgbPasses> {
    with_held(|h| h.get(lead).cloned())
}

/// One key of a sidecar's settings, read only when its name is in the bytes: most frames carry none.
fn setting(path: &str, key: &str) -> Option<Value> {
    let data = std::fs::read(sidecar::url_for(Path::new(path))).ok()?;
    let quoted = format!("\"{key}\"");
    if !data.windows(quoted.len()).any(|w| w == quoted.as_bytes()) {
        return None;
    }
    let document: Value = serde_json::from_slice(&data).ok()?;
    document.get("settings")?.get(key).filter(|v| !v.is_null()).cloned()
}

/// `named(by:)`: the triplet held for a lead, else the one its sidecar names; `None` for a file that leads none.
pub fn named_by(lead: &str) -> Option<RgbPasses> {
    held(lead).or_else(|| serde_json::from_value(setting(lead, "rgbPasses")?).ok())
}

/// The passes' paths beside the lead, red, green then blue.
pub fn paths_beside(passes: &RgbPasses, lead: &str) -> Vec<String> {
    let folder = paths::parent(lead);
    [&passes.red, &passes.green, &passes.blue].iter().map(|n| paths::join(&folder, n)).collect()
}

/// The three files of the frame `path` belongs to, lead or pass; `None` for a file standing alone.
pub fn group(path: &str) -> Option<Vec<String>> {
    if let Some(passes) = named_by(path) {
        return Some(paths_beside(&passes, path));
    }
    let lead = setting(path, "passOf")?.as_str()?.to_owned();
    let lead = paths::join(&paths::parent(path), &lead);
    named_by(&lead).map(|passes| paths_beside(&passes, &lead))
}

/// `paths` with every frame made of passes spelt out as its three files, in order, once each.
pub fn expanded(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        for file in group(path).unwrap_or_else(|| vec![path.clone()]) {
            if seen.insert(file.clone()) {
                out.push(file);
            }
        }
    }
    out
}

/// `RGBPasses.framings`: each lead's two other passes on the lead's framing, so a split keeps the crop drawn.
pub fn framings(paths: &[String]) -> std::collections::HashMap<String, oni_settings::frame::Geometry> {
    let mut out = std::collections::HashMap::new();
    for path in paths {
        let Some(passes) = named_by(path) else { continue };
        let Ok(loaded) = sidecar::read(Path::new(path), false, false) else { continue };
        for pass in paths_beside(&passes, path) {
            if pass != *path {
                out.insert(pass, loaded.settings.geometry);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seven() -> Vec<(String, Option<Date>)> {
        let t0 = 800_000_000.0;
        [0.0, 2.0, 4.0, 30.0, 50.0, 70.0, 75.0].iter().enumerate().map(|(i, s)| (format!("/rgb/f{i}.ARW"), Some(Date::since_reference_date(t0 + s)))).collect()
    }

    /// `RGBPasses.selfCheck`'s grouping: seven files in capture order B G R.
    #[test]
    fn seven_files_make_two_frames_led_by_their_first_pass_and_a_stretched_one_is_said() {
        let files = seven();
        let g = grouped(&files, PassOrder::Bgr, 0);
        assert_eq!(g.frames.len(), 2);
        assert_eq!(g.left_over, ["/rgb/f6.ARW"]);
        assert_eq!(g.frames[0].lead, "/rgb/f0.ARW");
        assert_eq!((g.frames[0].passes.red.as_str(), g.frames[0].passes.green.as_str(), g.frames[0].passes.blue.as_str()), ("f2.ARW", "f1.ARW", "f0.ARW"));
        // 40 s between the second triplet's passes when the frames lie 26 s apart.
        assert_eq!(g.stretched, HashSet::from(["/rgb/f3.ARW".to_string()]));
        // Twin: shifted by one file, every triplet starts one file later.
        let later = grouped(&files, PassOrder::Bgr, 1);
        assert_eq!(later.frames.iter().map(|f| f.lead.as_str()).collect::<Vec<_>>(), ["/rgb/f1.ARW", "/rgb/f4.ARW"]);
        assert_eq!(later.left_over, ["/rgb/f0.ARW"]);
        assert_eq!(grouped(&files, PassOrder::Rgb, 0).frames[0].passes.red, "f0.ARW", "the declared order decides which file is which band");
        // A triplet straddling two folders is left out whole; no capture time says nothing stretched.
        let mut apart: Vec<(String, Option<Date>)> = files.iter().map(|f| (f.0.clone(), None)).collect();
        apart[4].0 = "/other/f4.ARW".into();
        let g = grouped(&apart, PassOrder::Bgr, 0);
        assert_eq!(g.frames.len(), 1);
        assert_eq!(g.left_over, ["/rgb/f6.ARW", "/rgb/f3.ARW", "/other/f4.ARW", "/rgb/f5.ARW"]);
        assert!(g.stretched.is_empty());
    }

    #[test]
    fn a_pass_s_shares_are_its_light_per_channel_over_the_sum() {
        let s = shares(&vec![[0.1, 0.3, 0.1, 1.0]; 12]).unwrap();
        assert!((s[0] - 0.2).abs() < 1e-6 && (s[1] - 0.6).abs() < 1e-6 && (s[2] - 0.2).abs() < 1e-6, "{s:?}");
        // Twin: a black pass reads no share at all.
        assert_eq!(shares(&[[0.0, 0.0, 0.0, 1.0]]), None);
    }

    #[test]
    fn a_held_triplet_names_its_lead_before_any_sidecar_and_is_gone_once_released() {
        let lead = "/nowhere/held-lead.ARW";
        let passes = RgbPasses { red: "r".into(), green: "g".into(), blue: "held-lead.ARW".into(), registration: None };
        assert_eq!(named_by(lead), None);
        hold_one(lead, passes.clone());
        assert_eq!(named_by(lead), Some(passes.clone()));
        assert_eq!(group(lead), Some(vec!["/nowhere/r".to_string(), "/nowhere/g".into(), lead.into()]));
        release();
        // Twin: released, the file stands alone again.
        assert_eq!(named_by(lead), None);
    }
}
