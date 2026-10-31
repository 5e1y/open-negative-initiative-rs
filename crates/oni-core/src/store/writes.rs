//! The sidecars a gesture writes beside the open frame: each read, laid and written side by side.

use super::*;

impl EditorStore {
    /// Reads each frame's sidecar, lays `transform` and writes what moved; a sidecar from a newer
    /// version is left alone. The before and after of every frame written.
    pub(super) fn modify(
        &mut self,
        paths: &[String],
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        self.modify_with(paths, false, transform)
    }

    /// `modify`, keeping each sidecar's modification date when `preserving` so the thumbnail key holds.
    pub(super) fn modify_with(
        &mut self,
        paths: &[String],
        preserving: bool,
        transform: impl Fn(&str, &PipelineSettings) -> Option<PipelineSettings>,
    ) -> (Settings, Settings) {
        // One thread per file: a path twice would have two threads write one sidecar.
        let mut seen = std::collections::HashSet::new();
        let paths: Vec<String> = paths.iter().filter(|p| seen.insert(p.as_str())).cloned().collect();
        let paths = &paths[..];
        let mosaics: Vec<bool> = paths.iter().map(|p| self.mosaic(p)).collect();
        // A missing original reads as no photo, never as a fresh one its absent sidecar would suggest.
        let loaded = in_parallel(paths, |i, path| {
            sidecar::original_present(Path::new(path)).and_then(|()| sidecar::read(Path::new(path), mosaics[i], true))
        });
        let mut gone = 0;
        let mut writes = Vec::new();
        for (path, loaded) in paths.iter().zip(loaded) {
            let loaded = match loaded {
                Ok(loaded) if loaded.writable => loaded,
                Ok(_) => continue,
                Err(sidecar::Error::Unreachable) => {
                    gone += 1;
                    continue;
                }
                Err(e) => {
                    self.failure = Some(refusal::said(&e));
                    continue;
                }
            };
            let Some(next) = transform(path, &loaded.settings) else { continue };
            if next != loaded.settings {
                writes.push((path.clone(), loaded.settings, next));
            }
        }
        let options = WriteOptions { preserving_revision: preserving, ..WriteOptions::default() };
        let written = in_parallel(&writes, |_, (path, _, next)| sidecar::write(next, Path::new(path), options.clone()));
        let mut before = Settings::new();
        let mut after = Settings::new();
        for ((path, was, next), result) in writes.into_iter().zip(written) {
            match result {
                Ok(()) => {
                    before.insert(path.clone(), was);
                    after.insert(path, next);
                }
                Err(sidecar::Error::Unreachable) => gone += 1,
                Err(e) => self.failure = Some(refusal::said(&e)),
            }
        }
        if gone > 0 {
            self.failure = Some(refusal::left_out(gone));
        }
        // A frame that left or joined a roll has both rolls found again on the next ask.
        let moved: Vec<RollKey> = before
            .iter()
            .filter_map(|(p, was)| Some((RollKey::of(p, was), RollKey::of(p, after.get(p)?))))
            .filter(|(then, now)| then != now)
            .flat_map(|(then, now)| [then, now])
            .flatten()
            .collect();
        moved.iter().for_each(|k| _ = self.measured.remove(k));
        self.together.landed(&after);
        (before, after)
    }
}

/// `work` on each item, a few threads at once, the results in the items' order: a roll's sidecars
/// read and written side by side, each file its own, as Swift's batch moves them off the main thread.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    const THREADS: usize = 8;
    if items.len() < 2 {
        return items.iter().enumerate().map(|(i, t)| work(i, t)).collect();
    }
    let chunk = items.len().div_ceil(THREADS);
    std::thread::scope(|s| {
        let work = &work;
        let handles: Vec<_> = items
            .chunks(chunk)
            .enumerate()
            .map(|(c, part)| s.spawn(move || part.iter().enumerate().map(|(j, t)| work(c * chunk + j, t)).collect::<Vec<R>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a sidecar thread")).collect()
    })
}
