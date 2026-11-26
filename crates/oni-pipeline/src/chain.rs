//! `Pipeline.chain`: the frame's recipe from the scan to the output clip, on its legacy print where it lays one.

use super::*;

impl Pipeline {
    /// `apply` as a recipe on `source`: evaluate any region of it, materialise it, or read it whole.
    pub fn image(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let mut input = Image::new(source);
        if request.scale != 1.0 {
            input = input.lanczos(request.scale);
        }
        let e = input.extent();
        let full_width = request.full_width.unwrap_or(f64::from(source.extent().w));
        let scale = if full_width > 0.0 { f64::from(e.w) / full_width } else { 1.0 };
        // Taken before stage 0: the band is a fraction of the picture, the dosed radii export pixels.
        let reference = f64::from(e.w.max(e.h));
        let measuring = request.measuring;
        let laid = scanner::on_legacy_print(settings);
        let s = laid.as_ref().unwrap_or(settings);
        let scan = input.extent();
        let framed = self.framed(graph, input, s, scale, inputs)?;
        // A scanned slide is projected; a measure upstream of the print reads its scan through the windows as any frame's.
        if let Some(stock) = s.projector().filter(|_| !s.mode.is_digital() && !s.print.bypassed) {
            let shown = self.slide(graph, framed, s, stock)?;
            return self.finishing(graph, shown, s, scale, reference, measuring);
        }
        // A digital photo's scene, lit at its film's white, takes its film's own light. A film's look reads the scene as
        // it was shot, its own white.
        let has_look = s.film().is_some_and(|f| f.stock.look().is_some());
        // The grain's field lies on the framed scan, which no film or grain slider moves.
        let field_base = framed.clone();
        let lit = match conversion::scene_white(s).filter(|_| !has_look) {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        // A measure reads no effect, and a film mode renders none (owner's call).
        let effects = (!measuring && s.mode.is_digital()).then_some(scan);
        // The grain lies on the developed film, a pass of its own: a print notch starts from the grained film the
        // canvas's cache keeps.
        let grain = effects.and_then(|scan| grain::grain(s, scan, scale)).map(|k| k.seeded(inputs.seed));
        let grained = |film: Image| match &grain {
            Some(k) => grain::grained_over(film, &field_base, k),
            None => film,
        };
        // The virtual film takes that scene as a pass of its own too, the halation on its layers before their curve: its
        // negative is read as any negative is. A look holds no layer, so its effects lie on the scene it reads; it
        // gives a positive, printed on a paper through the internegative or shown as it is; a slide is projected.
        // The film's plate, the photo's whole frame at the preview's side healed and lit as the scene is: its halation's
        // glow is drawn there once and read at every scale and place.
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
                let at = f64::from(whole.extent().w) / full_width;
                let healed = self.healed(graph, whole, s, at, inputs)?;
                let lit_plate = match conversion::scene_white(s).filter(|_| !has_look) {
                    Some(m) => healed.then(stages::color_matrix(&m)),
                    None => healed,
                };
                let map = geometry::scan_from(&s.geometry, scan, scale).map(|r| r.map(|v| (v * at) as f32));
                (lit_plate, map)
            }
            None => (lit.clone(), [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        };
        let copy = crate::paper::internegative::copy(s);
        let read = copy.as_ref().unwrap_or(s);
        let looked_glow = match has_look {
            true => self.glow(graph, s, effects, plate.0.clone(), plate.1)?,
            false => None,
        };
        let scene = effects.filter(|_| has_look).and_then(|scan| layers::laid(lit.clone(), s, scan, looked_glow.as_ref())).unwrap_or_else(|| lit.clone());
        let printed = match self.looked(graph, scene, s)? {
            Some(positive) => match &copy {
                Some(c) => stages::converted(self.filmed(graph, grained(positive), c, None)?, &conversion::stage47(c)),
                None => grained(positive),
            },
            None => match s.film().and(s.projector()) {
                Some(stock) => {
                    let slide = grained(self.filmed_on(graph, lit, s, effects, &plate)?);
                    self.on_screen(graph, slide, s, stock)?
                }
                None => stages::converted(grained(self.filmed_on(graph, lit, read, effects, &plate)?), &conversion::stage47(read)),
            },
        };
        // On a paper the legacy print lays its keys and TEMP alone: no look cube is laid over the paper's print.
        let looked = match s.legacy() {
            Some(l) if s.paper().is_some() => self.warmed_paper(graph, printed, &l)?,
            Some(l) => self.oni_finish(graph, printed, &l)?,
            None => printed,
        };
        self.finishing(graph, looked, s, scale, reference, measuring)
    }
}
