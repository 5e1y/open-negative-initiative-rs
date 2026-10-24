//! A batch's destinations before anything is written: what each would land on, and the names the
//! Finder would give the ones that cannot land.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::file::{self, Ledger, Standing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clash {
    None,
    /// An earlier export of ONI's, which a confirmed Replace may write over.
    Export,
    /// An original, or any file ONI did not export, or a frame's own source: never written over.
    Original,
    /// A name the batch gives twice, which would write over its own earlier file.
    Twice,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub named: Vec<PathBuf>,
    pub clashes: Vec<Clash>,
}

/// A path as a volume that ignores case holds it, so two names it takes for one file count as one.
fn folded(path: &Path) -> String {
    oni_colour::profile::standardized(path).to_lowercase()
}

/// Each destination against the disk, the ledger, the frames' sources and the batch's own names.
pub fn plan(named: &[PathBuf], sources: &[PathBuf], ledger: &Ledger) -> Plan {
    let own: HashSet<PathBuf> = sources.iter().map(|s| file::canonical(s)).collect();
    let mut seen = HashSet::new();
    let clashes = named
        .iter()
        .map(|path| {
            if !seen.insert(folded(path)) {
                return Clash::Twice;
            }
            if own.contains(&file::canonical(path)) {
                return Clash::Original;
            }
            match file::standing(path, ledger) {
                Standing::Free => Clash::None,
                Standing::Export => Clash::Export,
                Standing::Original => Clash::Original,
            }
        })
        .collect();
    Plan { named: named.to_vec(), clashes }
}

impl Plan {
    /// The clashes a Replace may not write over.
    pub fn kept(&self) -> usize {
        self.clashes.iter().filter(|c| matches!(c, Clash::Original | Clash::Twice)).count()
    }

    pub fn exports(&self) -> usize {
        self.clashes.iter().filter(|c| **c == Clash::Export).count()
    }

    /// Every clash named the Finder way, `P1 2.jpg`, at the first number free on disk and in the batch.
    pub fn renamed(&self) -> Vec<PathBuf> {
        let mut taken: HashSet<String> = self.named.iter().zip(&self.clashes).filter(|(_, c)| **c == Clash::None).map(|(p, _)| folded(p)).collect();
        self.named
            .iter()
            .zip(&self.clashes)
            .map(|(path, clash)| {
                if *clash == Clash::None {
                    return path.clone();
                }
                let free = (2..).map(|n| file::numbered(path, n)).find(|p| !taken.contains(&folded(p)) && std::fs::symlink_metadata(p).is_err());
                let free = free.expect("an unbounded count reaches a free name");
                taken.insert(folded(&free));
                free
            })
            .collect()
    }
}
