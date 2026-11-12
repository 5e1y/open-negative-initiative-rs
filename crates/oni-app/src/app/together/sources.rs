//! Edit Together's sources: each frame decoded at a tile's side a few at once, its thumbnail shown
//! in its tile meanwhile.

use super::*;

impl Ctl {
    /// Each frame's source at a tile's side, a few at once off this thread: a frame the editor holds
    /// is reduced from its preview at once, the others decoded, those held first.
    pub(super) fn decode_together(&self, id: u64, mut paths: Vec<(usize, String)>) {
        let previews = self.state.borrow().previews.clone();
        let held: Vec<bool> = paths.iter().map(|(_, p)| previews.lock().expect("previews").holds(p)).collect();
        let mut order: Vec<usize> = (0..paths.len()).collect();
        order.sort_by_key(|&k| !held[k]);
        paths = order.into_iter().map(|k| paths[k].clone()).collect();
        let next = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(paths);
        for _ in 0..DECODERS.min(paths.len()) {
            let (engine, next, paths, previews) = (self.engine.clone(), next.clone(), paths.clone(), previews.clone());
            std::thread::spawn(move || {
                // A group left while a decode waits for its place gives that place to the next group's.
                crate::decode_gate::only_while(move || GROUPS.load(Ordering::Relaxed) == id);
                loop {
                    let Some((i, path)) = paths.get(next.fetch_add(1, Ordering::Relaxed)).cloned() else { return };
                    if GROUPS.load(Ordering::Relaxed) != id {
                        return;
                    }
                    let settings = drawing::settings_of(&path);
                    let reading = settings.as_ref().map(render::Reading::of).unwrap_or_default();
                    let preview = previews.lock().expect("previews").get(&path, &reading);
                    let made = match (preview, settings) {
                        (Some(p), Some(s)) => engine.lock().and_then(|mut e| e.tile_of(&p, source_side(&s, (p.size().0 as usize, p.size().1 as usize)))).map(Arc::new),
                        _ => tile_source(&engine, &path, id),
                    };
                    let Ok(negative) = made else { continue };
                    with_shell(move |c| c.tile_decoded(id, i, negative));
                }
            });
        }
    }

    /// Each lacking frame's thumbnail, read from disk and laid on the GPU off this thread: a tile shows
    /// it at once until its frame decodes, unless it waits to be drawn again, a look the frame left.
    pub(super) fn stand_in(&self, id: u64, lacking: &[(usize, String)]) {
        let (support, held): (PathBuf, Vec<(usize, String)>) = {
            let s = self.state.borrow();
            let paint = s.painter.0.lock().expect("painter");
            let current = lacking.iter().filter(|(_, p)| !paint.stale(p));
            (s.support.clone(), current.filter_map(|(i, p)| s.fingerprint(p).map(|f| (*i, f.to_string()))).collect())
        };
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            for (i, fingerprint) in held {
                if GROUPS.load(Ordering::Relaxed) != id {
                    return;
                }
                let Some((rgba, w, h)) = crate::thumbnails::load(&support, &fingerprint, Screen::Srgb) else { continue };
                let mut pixels: Vec<[f32; 4]> = rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]].map(|v| f32::from(v) / 255.0)).collect();
                oni_colour::source::Source::of(None, None, false).linearise(&mut pixels, false);
                let Ok(picture) = engine.lock().and_then(|mut e| e.picture(w as u32, h as u32, &pixels)) else { continue };
                with_shell(move |c| c.tile_stood_in(id, i, picture));
            }
        });
    }

    fn tile_stood_in(self: &Rc<Self>, id: u64, i: usize, picture: oni_graph::Source) {
        let landed = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id && g.negatives[i].is_none() => {
                g.stand_ins[i] = Some(picture);
                true
            }
            _ => false,
        };
        if landed {
            self.request_render();
        }
    }

    fn tile_decoded(self: &Rc<Self>, id: u64, i: usize, negative: Arc<Negative>) {
        let landed = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id => {
                g.negatives[i] = Some(negative);
                g.stand_ins[i] = None;
                true
            }
            _ => false,
        };
        if landed {
            self.request_render();
            self.balance_ahead();
        }
    }
}
