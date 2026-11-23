//! `Pipeline.chain`: the frame's recipe from the scan to the output clip, on its legacy print where it lays one.

use super::*;

impl Pipeline {
    /// `apply` as a recipe on `source`: evaluate any region of it, materialise it, or read it whole.
    pub fn image(&mut self, graph: &mut Graph, source: &Source, settings: &PipelineSettings, request: &Request, inputs: &Inputs) -> Result<Image> {
        let legacy = scanner::on_legacy_print(settings);
        self.drawn(graph, source, settings, legacy.as_ref(), request, inputs)
    }

    fn drawn(
        &mut self,
        graph: &mut Graph,
        source: &Source,
        settings: &PipelineSettings,
        legacy: Option<&PipelineSettings>,
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
        self.chain(graph, input, settings, legacy, (scale, reference), request.measuring, inputs)
    }

    /// `image` with the legacy print's conversion given: `legacy` is `on_legacy_print`'s settings for `frame`, `None`
    /// where it lays none; `at` is the scale and the reference length.
    #[allow(clippy::too_many_arguments)]
    pub fn chain(
        &mut self,
        graph: &mut Graph,
        input: Image,
        frame: &PipelineSettings,
        legacy: Option<&PipelineSettings>,
        at: (f64, f64),
        measuring: bool,
        inputs: &Inputs,
    ) -> Result<Image> {
        let ((scale, reference), s) = (at, legacy.unwrap_or(frame));
        let scan = input.extent();
        let framed = self.framed(graph, input, s, scale, inputs)?;
        // A digital photo's scene, lit at its film's white, takes its film's own light, then its grain, as the
        // film does; a measure reads neither. A film mode renders neither effect (owner's call). A slide on a paper
        // exposes its internegative with its light. A film's look reads the scene as it was shot, its own white.
        let copy = crate::paper::internegative::copy(s);
        let has_look = s.film().is_some_and(|f| f.stock.look().is_some());
        let light = if has_look { None } else { copy.as_ref().map(|c| c.light).or_else(|| conversion::scene_white(s)) };
        let lit = match light {
            Some(m) => framed.then(stages::color_matrix(&m)),
            None => framed,
        };
        let effects = !measuring && s.mode.is_digital();
        let haloed = if effects { halation::halated(lit, s, scan) } else { lit };
        // The grain is a pass of its own: a print notch starts from the grained scene the canvas's cache keeps.
        let grained = match effects.then(|| grain::grain(s, scan, scale)).flatten() {
            Some(k) => grain::grained(haloed, &k.seeded(inputs.seed), grain::dissolve(s, scan)),
            None => haloed,
        };
        // The virtual film takes that scene as a pass of its own too: its negative is read as any negative is. A look
        // gives a positive: printed on a paper through the internegative, as a slide is, else shown as it is.
        let read = copy.as_ref().map_or(s, |c| &c.settings);
        let printed = match self.looked(graph, grained.clone(), s)? {
            Some(positive) => match &copy {
                Some(c) => {
                    let negative = self.filmed(graph, positive.then(stages::color_matrix(&c.light)), read)?;
                    stages::converted(negative, &conversion::stage47(read))
                }
                None => positive,
            },
            None => {
                let negative = self.filmed(graph, grained, read)?;
                stages::converted(negative, &conversion::stage47(read))
            }
        };
        // On a paper the legacy print lays its keys and TEMP alone: no look cube is laid over the paper's print.
        let looked = match legacy.and_then(PipelineSettings::legacy) {
            Some(l) if s.paper().is_some() => self.warmed_paper(graph, printed, &l)?,
            Some(l) => self.oni_finish(graph, printed, &l)?,
            None => printed,
        };
        self.finishing(graph, looked, s, scale, reference, measuring)
    }
}
