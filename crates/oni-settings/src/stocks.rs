//! The film stocks and papers ONI knows, each one file (`file`): the shipped ones built from this crate's `stocks` folder,
//! the person's own adding or standing in by key; their catalogue (`catalogue`) is swapped whole while the app runs.

pub mod file;
mod catalogue;
mod entries;
mod order;
#[cfg(test)]
mod tests;

pub use catalogue::*;
pub use entries::*;
pub use order::natural;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

mod shipped {
    include!(concat!(env!("OUT_DIR"), "/shipped_stocks.rs"));
}

/// The files built into the binary, by name: every stock's and paper's JSON and every canister's SVG.
pub fn shipped_files() -> &'static [(&'static str, &'static [u8])] {
    &shipped::SHIPPED
}

/// Where a file was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Shipped,
    /// The person's own file, in the support folder's `Stocks`.
    Own(PathBuf),
}

/// A file read and taken: its name, where it lies, what it holds and its bytes' hash, which a bake is keyed on.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub name: String,
    pub origin: Origin,
    pub file: file::File,
    pub hash: u64,
}

/// A file left out, with why.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Refusal {
    pub name: String,
    pub reason: String,
}

/// Two hashes as one.
pub(crate) fn mixed(a: u64, b: u64) -> u64 {
    (a ^ b.rotate_left(29)).wrapping_mul(0x0100_0000_01b3)
}

/// FNV-1a over `bytes`: a file's identity across reads, never a secret.
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// `text` kept for the app's life, one copy whatever reads it: a key or a name a file or a sidecar holds.
pub fn intern(text: &str) -> &'static str {
    static HELD: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut held = HELD.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = held.get(text) {
        return t;
    }
    let kept: &'static str = Box::leak(text.to_owned().into_boxed_str());
    held.insert(kept);
    kept
}

/// The shipped files read and their canisters; a shipped file that does not read is left out, which its check catches.
pub fn shipped() -> (Vec<Loaded>, Vec<(String, Origin, Arc<[u8]>)>) {
    let mut files = Vec::new();
    let mut canisters = Vec::new();
    for (name, bytes) in shipped_files() {
        if name.ends_with(".svg") {
            canisters.push((name.to_string(), Origin::Shipped, Arc::from(*bytes)));
        } else if let Ok(file) = file::read(bytes) {
            files.push(Loaded { name: name.to_string(), origin: Origin::Shipped, file, hash: hash(bytes) });
        }
    }
    (files, canisters)
}

fn slot() -> &'static RwLock<Arc<Catalogue>> {
    static CURRENT: OnceLock<RwLock<Arc<Catalogue>>> = OnceLock::new();
    CURRENT.get_or_init(|| {
        // A shipped file refused is left out, which the shipped catalogue's check catches.
        let (files, canisters) = shipped();
        RwLock::new(Arc::new(Catalogue::build(files, canisters).0))
    })
}

/// The catalogue in use: the shipped files' until the app reads the person's.
pub fn current() -> Arc<Catalogue> {
    slot().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `catalogue` in use from now on, for every stock and paper read.
pub fn install(catalogue: Catalogue) -> Arc<Catalogue> {
    let catalogue = Arc::new(catalogue);
    *slot().write().unwrap_or_else(|e| e.into_inner()) = catalogue.clone();
    catalogue
}

/// A key type a sidecar stores: the key as written, read through the catalogue so a retired key reads as its entry's,
/// copied freely and compared by its text. `$default`, where given, is the key a sidecar storing none reads.
macro_rules! keyed {
    ($(#[$doc:meta])* $name:ident, $space:ident, $default:literal) => {
        $crate::stocks::keyed!($(#[$doc])* $name, $space);

        impl Default for $name {
            fn default() -> Self {
                Self($default)
            }
        }
    };
    ($(#[$doc:meta])* $name:ident, $space:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(&'static str);

        impl $name {
            pub const fn of(key: &'static str) -> Self {
                Self(key)
            }

            pub fn key(self) -> &'static str {
                self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let key = <String as serde::Deserialize>::deserialize(d)?;
                Ok(Self($crate::stocks::current().canonical($crate::stocks::Space::$space, &key)))
            }
        }
    };
}
pub(crate) use keyed;
