//! Reading the targets: each photo's line, its EXIF and its preview.

use super::*;

impl Ctl {
    /// Every line at once from the sidecars, then each frame decoded once for its size and preview,
    /// in order, off this thread; a newer pass or a space change stops this one.
    pub(super) fn read_targets(self: &Rc<Self>) {
        let (targets, generation, pass, artist, copyright, rolls) = {
            let c = self.export.borrow();
            let s = self.state.borrow();
            let rolls: Vec<Option<oni_settings::Film>> = c
                .targets
                .iter()
                .map(|t| {
                    let path = t.path.to_string_lossy();
                    s.archive.as_ref().and_then(|a| a.entries().into_iter().find(|e| e.path == path).and_then(|e| e.film.clone()))
                })
                .collect();
            let generation = c.pass.fetch_add(1, Ordering::Relaxed) + 1;
            (c.targets.clone(), generation, c.pass.clone(), c.artist.clone(), c.copyright.clone(), rolls)
        };
        let rows: Vec<Row> = targets.iter().zip(&rolls).map(|(t, roll)| export::opening_row(&t.path, roll.as_ref(), &artist, &copyright)).collect();
        let low = targets.iter().filter(|t| oni_decode::is_low_precision(&t.path)).count();
        {
            let mut c = self.export.borrow_mut();
            // A line typed in before it lands keeps the typing.
            for (i, row) in rows.into_iter().enumerate() {
                if c.rows[i].is_none() {
                    c.rows[i] = Some(row);
                }
            }
            c.low_precision = low;
        }
        self.publish_lines();
        self.publish_export();
        self.render_previews(targets, generation, pass);
    }

    pub(super) fn render_previews(self: &Rc<Self>, targets: Vec<Target>, generation: u64, pass: Arc<AtomicU64>) {
        let (engine, previews, screen, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.screen, s.negative.clone())
        };
        let live = move || pass.load(Ordering::Relaxed) == generation;
        std::thread::Builder::new()
            .name("export previews".into())
            .spawn(move || {
                for (i, target) in targets.into_iter().enumerate() {
                    if !live() {
                        return;
                    }
                    let path = target.path.to_string_lossy().into_owned();
                    let known = open.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()).or_else(|| previews.lock().ok()?.get(&path));
                    let negative = match known {
                        Some(n) => Ok(n),
                        None => prepared(&engine, &path, crate::render::Reading::of(&target.settings)),
                    };
                    let size = negative.as_ref().ok().map(|n| {
                        let (w, h) = n.size();
                        export::cropped_size((n.full_width, n.full_width * f64::from(h) / f64::from(w.max(1))), &target.settings)
                    });
                    // One exact render per photo in the export space, as the canvas shows it: proofed.
                    let picture = negative.and_then(|n| {
                        let linear = engine.lock()?.render(&n, &target.settings, Some(PREVIEW_SIDE as usize))?;
                        let k = Params::new(screen, linear.width as u32, linear.height as u32, i as u32);
                        Ok((crate::render::encoded(&linear, &k, 1), linear.width, linear.height))
                    });
                    with_shell(move |c| {
                        if c.export.borrow().pass.load(Ordering::Relaxed) != generation {
                            return;
                        }
                        {
                            let mut card = c.export.borrow_mut();
                            card.sizes[i] = size;
                            card.previews[i] = match picture {
                                Ok((bytes, w, h)) => Preview::Ready(image(&bytes, w, h)),
                                Err(_) => Preview::Failed,
                            };
                        }
                        c.publish_line(i);
                        if i == 0 {
                            c.publish_export();
                            c.weigh_export();
                        }
                    });
                }
            })
            .expect("export preview thread");
    }

    /// `refreshEstimate`: the first target weighed on its 500 px sample, multiplied by the count.
    pub(super) fn weigh_export(self: &Rc<Self>) {
        let (target, full, export, generation, weighing) = {
            let mut c = self.export.borrow_mut();
            c.estimate = None;
            let generation = c.weighing.fetch_add(1, Ordering::Relaxed) + 1;
            let (Some(target), Some(full)) = (c.targets.first().cloned(), c.full()) else { return };
            (target, full, c.settings.clone(), generation, c.weighing.clone())
        };
        let (engine, previews, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.negative.clone())
        };
        std::thread::spawn(move || {
            if weighing.load(Ordering::Relaxed) != generation {
                return;
            }
            let path = target.path.to_string_lossy().into_owned();
            let known = open.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()).or_else(|| previews.lock().ok()?.get(&path));
            let negative = known.or_else(|| prepared(&engine, &path, crate::render::Reading::of(&target.settings)).ok());
            let weighed = negative.and_then(|n| export::weigh(&engine, &n, full, &target.settings, &export));
            with_shell(move |c| {
                if c.export.borrow().weighing.load(Ordering::Relaxed) != generation {
                    return;
                }
                c.export.borrow_mut().estimate = weighed;
                c.publish_export();
            });
        });
    }
}
