//! Each dye set's profiles, from the files its stocks hold: the profile a stock's own set names, or the sources a mean
//! set such as the generic one averages.

use super::profile::Profile;
use oni_settings::DyeSet;
use oni_settings::stocks::DyeSource;

/// The colour profiles a mean set averages, the generic set's the nine still colour negatives most shot today.
pub fn sources(set: DyeSet) -> Vec<Profile> {
    match set.entry().source {
        DyeSource::Profile(json) => vec![profile(&json)],
        DyeSource::Mean(sets) => sets.into_iter().flat_map(sources).collect(),
    }
}

/// A profile of a file the catalogue took, which read when it was taken.
pub fn profile(json: &str) -> Profile {
    Profile::read(json.as_bytes()).expect("a profile reads once its file is taken")
}
