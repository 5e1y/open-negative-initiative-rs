//! `Pipeline.chain`: the frame's recipe from the scan to the output clip, on the face that renders it.

use super::*;

impl Pipeline {
    /// `apply` as a recipe on `source`: evaluate any region of it, materialise it, or read it whole.
    pub fn image(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let oni = scanner::on_oni_machine(settings);
        self.drawn(graph, source, settings, oni.as_ref(), request, inputs)
    }

    /// What a digital photo's levels receive on the face that renders: on the Scanner face, the machine's
    /// picture in front of its print and look; on Pro's, `image` itself.
    pub fn picture(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let entering = scanner::on_oni_machine(settings).map_or_else(|| settings.clone(), |m| m.unlit());
        self.drawn(graph, source, &entering, None, request, inputs)
    }

    fn drawn(
        &mut self,
        graph: &mut Graph,
        source: &Source,
        settings: &PipelineSettings,
        machine: Option<&PipelineSettings>,
        request: &Request,
        inputs: &Inputs,
    ) -> Result<Image> {
        let mut input = Image::new(source);
        if request.scale != 1.0 {
            input = input.lanczos(request.scale);
        }
        let e = input.extent();
        let full_width = request.full_width.unwrap_or(f64::from(source.extent().w));
        let scale = if full_width > 0.0 { f64::from(e.w) / full_width } else { 1.0 };
        // Taken before stage 0: the band is a fraction of the picture, the dosed radii export pixels.
        let reference = f64::from(e.w.max(e.h));
        self.chain(graph, input, settings, machine, (scale, reference), request.measuring, inputs)
    }

    /// `image` with the Scanner face's conversion given: `machine` is `on_oni_machine`'s settings for
    /// `frame`, `None` on the Pro face; `at` is the scale and the reference length.
    #[allow(clippy::too_many_arguments)]
    pub fn chain(
        &mut self,
        graph: &mut Graph,
        input: Image,
        frame: &PipelineSettings,
        machine: Option<&PipelineSettings>,
        at: (f64, f64),
        measuring: bool,
        inputs: &Inputs,
    ) -> Result<Image> {
        let ((scale, reference), s) = (at, machine.unwrap_or(frame));
        let scan = input.extent();
        let framed = self.framed(graph, input, s, scale, inputs)?;
        let balanced = match conversion::white(s) {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        // The film's own light, then its grain, lie on the scene before the print; a measure reads neither.
        // A digital photo's alone for now (owner's call), so a film mode never renders an effect it hides.
        let effects = !measuring && !s.mode.is_film();
        let haloed = if effects { halation::halated(balanced, s, scan) } else { balanced };
        // The grain is a pass of its own: a print notch starts from the grained scene the canvas's cache keeps.
        let grained = match grain::grain(s, scan, scale).filter(|_| effects) {
            Some(k) => haloed.then(stages::grain(&k)),
            None => haloed,
        };
        let picture = stages::stage47(&conversion::stage47(s));
        // A digital photo's Lighting, or the machine's print, follows its levels.
        let lit = conversion::lighting(s).map(|k| stages::print(&k));
        let levels = picture_levels(s);
        if machine.is_some() && !s.mode.is_film() {
            // On the machine a digital photo's levels come before its print, the face's curves after the look.
            let (leveled, printing) = match levels {
                Some(_) => (self.curves(graph, stages::laid(grained, vec![picture]), &CurveSet::default(), levels)?, lit.into_iter().collect()),
                None => (grained, [picture].into_iter().chain(lit).collect()),
            };
            let looked = self.oni_finish(graph, leveled, printing, s, frame)?;
            return self.finishing(graph, looked, s, None, None, true, scale, reference, measuring);
        }
        // Pro's Lighting is in the first pass after the levels' and curves' tables; with neither, in the picture's own.
        let (with_picture, after_tables) = if finishing::tabled(&s.manual_curves, levels) { (None, lit) } else { (lit, None) };
        let out = stages::laid(grained, [picture].into_iter().chain(with_picture).collect());
        let looked = if machine.is_some() { self.oni_finish(graph, out, Vec::new(), s, frame)? } else { out };
        self.finishing(graph, looked, s, levels, after_tables, true, scale, reference, measuring)
    }
}
