//! The De-dust tab's reminders: each correction's dust as the frame shows it unpatched, kept until the
//! frame's look moves, rendered off the interface thread.

use super::*;
use oni_settings::PipelineSettings;
use oni_settings::id::Id;

/// The look the reminders were rendered on, each one by its correction, and whether a render is out.
#[derive(Default)]
struct Kept {
    path: String,
    look: Option<PipelineSettings>,
    images: HashMap<Id, Image>,
    asking: bool,
}

thread_local! {
    static KEPT: RefCell<Kept> = RefCell::new(Kept::default());
}

/// The frame's settings with no correction: what a reminder shows, whatever was healed since.
fn look_of(settings: &PipelineSettings) -> PipelineSettings {
    let mut bare = settings.clone();
    bare.corrections.clear();
    bare
}

/// The reminder kept for a correction, else an empty image until it lands.
pub(super) fn original(id: Id) -> Image {
    KEPT.with(|k| k.borrow().images.get(&id).cloned().unwrap_or_default())
}

impl Ctl {
    /// Renders the reminders the tab lacks, every one again once the frame's look has moved.
    pub(in crate::app) fn ask_originals(&self) {
        if !self.painting() {
            return;
        }
        let (path, negative, settings, screen) = {
            let s = self.state.borrow();
            let Some((path, negative)) = s.negative.clone() else { return };
            if s.open_path() != Some(path.as_str()) || s.hands.load(Ordering::Relaxed) > 0 {
                return;
            }
            (path, negative, s.store.settings().clone(), s.screen)
        };
        let look = look_of(&settings);
        let lacking: Vec<(Id, Vec<(f64, f64)>, f64)> = KEPT.with(|k| {
            let mut k = k.borrow_mut();
            if k.path != path || k.look.as_ref() != Some(&look) {
                *k = Kept { path: path.clone(), look: Some(look.clone()), images: HashMap::new(), asking: k.asking };
            }
            if k.asking {
                return Vec::new();
            }
            let lacking: Vec<_> = settings
                .corrections
                .iter()
                .filter(|c| !c.stroke.is_empty() && !k.images.contains_key(&c.id))
                .map(|c| (c.id, c.stroke.iter().map(|p| (p.x, p.y)).collect(), c.radius))
                .collect();
            k.asking = !lacking.is_empty();
            lacking
        });
        if lacking.is_empty() {
            return;
        }
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let strokes: Vec<(Vec<(f64, f64)>, f64)> = lacking.iter().map(|(_, s, r)| (s.clone(), *r)).collect();
            let rendered = engine.lock().and_then(|mut e| e.originals(&negative, &settings, &strokes));
            let encoded: Vec<(Id, Vec<u8>, usize, usize)> = match rendered {
                Ok(list) => lacking
                    .iter()
                    .zip(list)
                    .map(|((id, _, _), l)| (*id, render::encoded(&l, &Params::new(screen, l.width as u32, l.height as u32, 0), 1), l.width, l.height))
                    .collect(),
                Err(e) => {
                    timing::line(&format!("dust reminders not drawn: {e}"));
                    Vec::new()
                }
            };
            with_shell(move |c| {
                let landed = KEPT.with(|k| {
                    let mut k = k.borrow_mut();
                    k.asking = false;
                    // A look moved meanwhile asks again rather than showing the former one.
                    let current = k.path == path && k.look.as_ref() == Some(&look);
                    if current {
                        for (id, rgba, w, h) in encoded {
                            k.images.insert(id, image(&rgba, w, h));
                        }
                    }
                    current
                });
                if landed {
                    c.publish_dedust();
                } else {
                    c.ask_originals();
                }
            });
        });
    }
}
