//! The background threads: the thumbnails painter and the disk reader, and the render helpers they share.

use super::*;

/// The tiles Slint was last given, read back by the checks.
pub fn tiles(window: &MainWindow) -> Vec<Tile> {
    let model = window.global::<Library>().get_tiles();
    (0..model.row_count()).filter_map(|i| model.row_data(i)).collect()
}

/// The display kernel's block with the screen gamut's stripes when they are on, laid by where the
/// texture sits on the canvas, so they keep one size on screen whatever the zoom.
pub(super) fn zebra(s: &State, k: Params, placed: Option<Placed>, texels: u32, canvas: Option<(f64, f64)>) -> Params {
    if !s.screen_gamut {
        return k;
    }
    let started = *CLOCK.get_or_init(Instant::now);
    let height = canvas.map_or(f64::from(k.height), |c| c.1) as f32;
    let (origin, stretch) = placed.map_or(((0.0, 0.0), 1.0), |p| ((p.x as f32, p.y as f32), (p.width / f64::from(texels.max(1))) as f32));
    let phase = oni_colour::gamut::phase(started.elapsed().as_secs_f64());
    k.striped(display::Zebra { origin, stretch, canvas_height: height.max(1.0), phase })
}

/// The stripes' clock, as Swift reads `CACurrentMediaTime`.
static CLOCK: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// A frame decoded on the calling thread, the CPU's part, then laid on the GPU as its preview.
pub(super) fn prepared(engine: &Shared, path: &str, reading: render::Reading) -> Result<Arc<Negative>, String> {
    prepared_keeping(engine, path, reading).map(|(negative, _)| negative)
}

/// `prepared`, with the full resolution when the decode read it whole.
pub(super) fn prepared_keeping(engine: &Shared, path: &str, reading: render::Reading) -> Result<(Arc<Negative>, Option<oni_graph::Source>), String> {
    let decoded = render::decode(Path::new(path), reading)?;
    let (negative, full) = engine.prepare_keeping(decoded)?;
    Ok((Arc::new(negative), full))
}

/// The renderer's thread for thumbnails: an edit's redraw first, then the pass's next photo, each
/// waiting while a hand holds a control, so a drag never shares the GPU with the gallery.
pub(super) fn paint(painter: Painter, engine: Shared, previews: Arc<Mutex<Previews>>, hands: Arc<AtomicUsize>, generation: Arc<AtomicU64>, support: PathBuf, screen: Screen) {
    enum Next {
        Redraw(String, Option<String>),
        Pass(u64, Vec<(String, String)>),
        Fill(u64, String, String),
    }
    std::thread::Builder::new()
        .name("thumbnails".into())
        .spawn(move || {
            crate::decode_gate::in_background();
            let mut current: Option<(u64, VecDeque<(String, String)>)> = None;
            let mut seed = 0u32;
            loop {
                let next = {
                    let (lock, wake) = &*painter;
                    let mut paint = lock.lock().expect("painter");
                    loop {
                        if let Some((path, known)) = paint.redraws.pop_front() {
                            break Next::Redraw(path, known);
                        }
                        if let Some((g, rows)) = paint.pass.take() {
                            break Next::Pass(g, rows);
                        }
                        if let Some((g, queue)) = current.as_mut()
                            && let Some((path, fingerprint)) = queue.pop_front()
                        {
                            break Next::Fill(*g, path, fingerprint);
                        }
                        paint = wake.wait(paint).expect("painter");
                    }
                };
                let (pass, path, fingerprint) = match next {
                    Next::Pass(g, rows) => {
                        let (held, absent): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(_, f)| thumbnails::is_held(&support, f));
                        let counts = (held.len(), absent.len());
                        current = Some((g, absent.into()));
                        with_shell(move |c| c.pass_counted(g, counts.0, counts.1));
                        continue;
                    }
                    Next::Fill(g, path, fingerprint) => {
                        if generation.load(Ordering::Relaxed) != g {
                            current = None;
                            continue;
                        }
                        (Some(g), path, fingerprint)
                    }
                    Next::Redraw(path, known) => {
                        let Some(fingerprint) = known.or_else(|| oni_library::relink::fingerprint(Path::new(&path))) else {
                            let name = stem(&path);
                            with_shell(move |c| c.thumb_landed(None, String::new(), Err("its file is unreachable".into()), name));
                            continue;
                        };
                        (None, path, fingerprint)
                    }
                };
                while hands.load(Ordering::Relaxed) > 0 {
                    std::thread::sleep(Duration::from_millis(20));
                }
                seed = seed.wrapping_add(1);
                let drawn = drawn(&engine, &previews, &path, screen, seed).and_then(|(jpeg, srgb)| {
                    thumbnails::persist(&support, &fingerprint, &jpeg).map_err(|e| e.to_string())?;
                    Ok(srgb)
                });
                let name = stem(&path);
                with_shell(move |c| c.thumb_landed(pass, fingerprint, drawn, name));
            }
        })
        .expect("thumbnail thread");
}

/// `Thumbnails.render`: the photo's own sidecar through the per-pixel pipeline at `SIDE`, as an sRGB
/// JPEG, and the tile decoded back from it so a tile drawn and a tile read are the same pixels.
pub(super) fn drawn(engine: &Shared, previews: &Arc<Mutex<Previews>>, path: &str, screen: Screen, seed: u32) -> Result<(Vec<u8>, (Vec<u8>, usize, usize)), String> {
    let asked = Instant::now();
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let settings = oni_settings::sidecar::read(Path::new(path), mosaic, true).map_err(|e| e.to_string())?.settings;
    // `Thumbnails.produce`: the scan as the file holds it until the frame is converted.
    if !crate::scans::is_converted(&settings)
        && let Some(jpeg) = crate::scans::scan(Path::new(path))
    {
        let tile = thumbnails::decoded(&jpeg, screen).ok_or("the scan cannot be read back")?;
        timing::render(Kind::Thumbnail, &crate::anonymity::token(path), Duration::ZERO, asked.elapsed());
        return Ok((jpeg, tile));
    }
    let known = previews.lock().expect("previews").get(path);
    let negative = match known {
        Some(n) => n,
        None => {
            // A RAW read at half size, a third of the second: kept for the editor only when it was read whole.
            let decoded = render::decode_for(Path::new(path), render::Reading::of(&settings), render::PREVIEW_SIDE)?;
            let halved = mosaic && decoded.full_width > decoded.linear.width as f64;
            let n = Arc::new(engine.prepare(decoded)?);
            if !halved {
                previews.lock().expect("previews").insert(path.to_string(), n.clone());
            }
            n
        }
    };
    let mut guard = engine.lock().map_err(|_| "the render engine failed earlier")?;
    let started = Instant::now();
    let linear = guard.render(&negative, &settings.per_pixel_only(), Some(thumbnails::SIDE))?;
    let pipeline = started.elapsed();
    drop(guard);
    let (w, h) = (linear.width, linear.height);
    let srgb = render::encoded(&linear, &Params::new(Screen::Srgb, w as u32, h as u32, seed), 1);
    let jpeg = thumbnails::jpeg(&srgb, w, h).ok_or("the thumbnail cannot be encoded")?;
    let tile = thumbnails::decoded(&jpeg, screen).ok_or("the thumbnail cannot be read back")?;
    timing::render(Kind::Thumbnail, &crate::anonymity::token(path), pipeline, asked.elapsed());
    Ok((jpeg, tile))
}

/// `Thumbnails.drain`: the folder's files read off this thread, the latest asked first, so a
/// scroll shows the cells under the eye before those it left.
pub(super) fn read_disk(support: PathBuf, screen: Screen) -> mpsc::Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("thumbnail files".into())
        .spawn(move || {
            let mut stack: Vec<String> = Vec::new();
            loop {
                if stack.is_empty() {
                    match rx.recv() {
                        Ok(f) => stack.push(f),
                        Err(_) => return,
                    }
                }
                stack.extend(rx.try_iter());
                let Some(fingerprint) = stack.pop() else { continue };
                let read = thumbnails::load(&support, &fingerprint, screen);
                with_shell(move |c| c.disk_landed(fingerprint, read));
            }
        })
        .expect("thumbnail files thread");
    tx
}
