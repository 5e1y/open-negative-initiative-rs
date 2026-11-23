//! Tables baked from a stock's or a paper's file, each once per key and file content: an edited file bakes anew, and
//! the tables already lent to the kernels stay valid, as each lives as long as the app.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock};

pub struct Baked<K, T: 'static> {
    held: OnceLock<Mutex<HashMap<K, &'static T>>>,
}

impl<K: Hash + Eq, T: Sync + 'static> Baked<K, T> {
    pub const fn new() -> Self {
        Baked { held: OnceLock::new() }
    }

    /// The table of `key`, made by `make` at its first ask; the lock is free while it bakes, so another key never waits.
    pub fn get(&'static self, key: K, make: impl FnOnce() -> T) -> &'static T {
        let held = self.held.get_or_init(Default::default);
        if let Some(t) = held.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return t;
        }
        let made = make();
        match held.lock().unwrap_or_else(|e| e.into_inner()).entry(key) {
            Entry::Occupied(e) => e.get(),
            Entry::Vacant(v) => v.insert(Box::leak(Box::new(made))),
        }
    }
}
