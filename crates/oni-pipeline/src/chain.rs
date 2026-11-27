//! The frame's recipe from the scan to the output clip, on its legacy print where it lays one. A
//! digital photo's scene is shot on its virtual film and scanned first, on the scan's own grid; that scan, kept once
//! made, is read on as a scanned negative's is.

use super::*;

/// A request's source reduced, and what the chain reads off it: the scale to the full frame, the extent the dosed radii
/// are taken on, a measure's leaving off.
struct Input {
    image: Image,
    scale: f64,
    reference: f64,
    measuring: bool,
}

fn input(source: &Source, request: &Request) -> Input {
    let mut image = Image::new(source);
    if request.scale != 1.0 {
        image = image.lanczos(request.scale);
    }
    let e = image.extent();
    let full_width = request.full_width.unwrap_or(f64::from(source.extent().w));
    let scale = if full_width > 0.0 { f64::from(e.w) / full_width } else { 1.0 };
    // Taken before stage 0: the band is a fraction of the picture, the dosed radii export pixels.
    Input { image, scale, reference: f64::from(e.w.max(e.h)), measuring: request.measuring }
}

impl Pipeline {
    /// `apply` as a recipe on `source`: evaluate any region of it, materialise it, or read it whole.
    pub fn image(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let laid = scanner::on_legacy_print(settings);
        let s = laid.as_ref().unwrap_or(settings);
        let input = input(source, request);
        if s.mode.is_digital() {
            let scanned = self.shot(graph, &input, source, s, inputs)?;
            return self.read_scan(graph, scanned, s, &input);
        }
        let framed = self.framed(graph, input.image.clone(), s, input.scale, inputs)?;
        // A scanned slide is projected; a measure upstream of the print reads its scan through the windows as any frame's.
        if let Some(stock) = s.projector().filter(|_| !s.print.bypassed) {
            let shown = self.slide(graph, framed, s, stock)?;
            return self.finishing(graph, shown, s, input.scale, input.reference, input.measuring);
        }
        let copy = crate::paper::internegative::copy(s);
        let printed = stages::converted(framed, &conversion::stage47(copy.as_ref().unwrap_or(s)));
        self.printed(graph, printed, s, &input)
    }

    /// A digital photo's scan on its virtual film: `source`, its scene, shot and scanned on the scan's own grid, before
    /// any crop or turn, so the canvas keeps it while its film's settings hold and reads it as a scanned negative.
    pub fn scan(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let laid = scanner::on_legacy_print(settings);
        let s = laid.as_ref().unwrap_or(settings);
        let input = input(source, request);
        self.shot(graph, &input, source, s, inputs)
    }

    /// A digital photo's frame read off its scan, `scanned`, a source on the scan's grid at the full frame's width
    /// `request.full_width`, as a scanned negative's frame is: framed, then read and printed.
    pub fn on_scan(&mut self, graph: &mut Graph, scanned: &Source, settings: &PipelineSettings, request: &Request) -> Result<Image> {
        let laid = scanner::on_legacy_print(settings);
        let s = laid.as_ref().unwrap_or(settings);
        let input = input(scanned, request);
        let image = input.image.clone();
        self.read_scan(graph, image, s, &input)
    }

    /// The scene healed, seen by its film in its decode's light, shot on that film with its effects and grained: a scan.
    fn shot(&mut self, graph: &mut Graph, input: &Input, source: &Source, s: &PipelineSettings, inputs: &Inputs) -> Result<Image> {
        let scan = input.image.extent();
        let healed = self.healed(graph, input.image.clone(), s, input.scale, inputs)?;
        // A film's look reads the scene as its decode balanced it; any other film sees it against its own white.
        let has_look = s.film().is_some_and(|f| f.stock.look().is_some());
        let field_base = healed.clone();
        let lit = match conversion::scene_white(s).filter(|_| !has_look) {
            Some(m) => healed.then(stages::color_matrix(&m)),
            None => healed,
        };
        // A measure reads no effect.
        let effects = (!input.measuring).then_some(scan);
        // The grain lies on the developed film, its field drawn over the healed scene, which no film slider moves.
        let grain = effects.and_then(|scan| grain::grain(s, scan, input.scale)).map(|k| k.seeded(inputs.seed));
        let grained = |film: Image| match &grain {
            Some(k) => grain::grained_over(film, &field_base, k),
            None => film,
        };
        // The halation's glow is drawn on the film's plate, the whole frame at the preview's side lit as the scene is,
        // and read where each pixel lies on it: a full scan blurs no frame of its own size.
        let plate = match effects.filter(|_| halation::lays(s)) {
            Some(_) => {
                let whole = match inputs.plate {
                    Some(p) => Image::new(p),
                    None => {
                        let e = source.extent();
                        let long = f64::from(e.w.max(e.h));
                        if long > halation::PLATE_SIDE { Image::new(source).lanczos(halation::PLATE_SIDE / long) } else { Image::new(source) }
                    }
                };
                let full_width = f64::from(scan.w) / input.scale;
                let at = f64::from(whole.extent().w) / full_width;
                let healed = self.healed(graph, whole, s, at, inputs)?;
                let lit_plate = match conversion::scene_white(s).filter(|_| !has_look) {
                    Some(m) => healed.then(stages::color_matrix(&m)),
                    None => healed,
                };
                let to = (at / input.scale) as f32;
                (lit_plate, [[to, 0.0, 0.0], [0.0, to, 0.0]])
            }
            None => (lit.clone(), [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        };
        // A look holds no layer, so its effects lie on the scene it reads, and it gives a positive.
        if has_look {
            let glow = self.glow(graph, s, effects, plate.0.clone(), plate.1)?;
            let scene = effects.and_then(|scan| layers::laid(lit.clone(), s, scan, glow.as_ref())).unwrap_or(lit);
            let positive = self.looked(graph, scene, s)?.expect("a film of a look reads it");
            return Ok(grained(positive));
        }
        // An internegative's copy is shot on its own film; any other negative or slide on the frame's.
        let copy = crate::paper::internegative::copy(s);
        let read = copy.as_ref().filter(|_| s.projector().is_none()).unwrap_or(s);
        Ok(grained(self.filmed_on(graph, lit, read, effects, &plate)?))
    }

    /// A digital photo's scan framed and read: a slide projected, a look's positive shown or printed through its
    /// internegative, a negative read as any scanned negative is.
    fn read_scan(&mut self, graph: &mut Graph, scanned: Image, s: &PipelineSettings, input: &Input) -> Result<Image> {
        let e = scanned.extent();
        let lens = s.lens((f64::from(e.w) / input.scale, f64::from(e.h) / input.scale));
        let framed = geometry::apply(scanned, &s.geometry, lens.as_ref(), input.scale);
        let has_look = s.film().is_some_and(|f| f.stock.look().is_some());
        let copy = crate::paper::internegative::copy(s);
        let printed = match (has_look, s.film().and(s.projector())) {
            (true, _) => match &copy {
                Some(c) => stages::converted(self.filmed(graph, framed, c, None)?, &conversion::stage47(c)),
                None => framed,
            },
            (false, Some(stock)) => self.on_screen(graph, framed, s, stock)?,
            (false, None) => {
                let read = copy.as_ref().unwrap_or(s);
                stages::converted(framed, &conversion::stage47(read))
            }
        };
        self.printed(graph, printed, s, input)
    }

    /// A frame printed, its legacy print's look laid, then finished.
    fn printed(&mut self, graph: &mut Graph, printed: Image, s: &PipelineSettings, input: &Input) -> Result<Image> {
        // On a paper the legacy print lays its keys and TEMP alone: no look cube is laid over the paper's print.
        let looked = match s.legacy() {
            Some(l) if s.paper().is_some() => self.warmed_paper(graph, printed, &l)?,
            Some(l) => self.oni_finish(graph, printed, &l)?,
            None => printed,
        };
        self.finishing(graph, looked, s, input.scale, input.reference, input.measuring)
    }
}
