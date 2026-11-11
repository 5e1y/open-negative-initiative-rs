//! What each node must produce for a requested region, back from the output to the sources.

use super::*;

impl<'a> Plan<'a> {
    /// Consumers before producers, each rectangle the union of what its readers need; a node the cache
    /// holds whole asks nothing of its inputs.
    pub(super) fn new(image: &'a Image, request: Rect, cache: Option<&Cache>) -> Result<Self, Error> {
        let mut nodes = Vec::new();
        let last = image.flatten(&mut nodes, &mut HashMap::new());
        let n = nodes.len();
        let mut rects: Vec<Option<Rect>> = vec![None; n];
        let mut hits: Vec<Option<(FloatTexture, Rect)>> = vec![None; n];
        let mut runs = vec![Run::Whole; n];
        let mut shown = HashMap::new();
        rects[last] = Some(request);
        for i in (0..n).rev() {
            let Some(need) = rects[i] else { continue };
            if matches!(nodes[i].kind, Kind::Source(_)) {
                continue;
            }
            if let Some(hit) = cache.and_then(|c| c.hit(nodes[i].key, need)) {
                hits[i] = Some(hit);
                continue;
            }
            let mut ask = |j: usize, region: Rect| -> Result<(), Error> {
                if region.is_empty() || nodes[j].extent.is_empty() {
                    return Err(Error::EmptyInput { stage: i.saturating_sub(1) });
                }
                // A request wholly past its input (a straightened frame's clear corner) reads the nearest edge, as `load` clamps.
                let region = region.nearest_in(&nodes[j].extent);
                rects[j] = Some(rects[j].map_or(region, |r| r.union(&region)));
                Ok(())
            };
            let (stage, input, aux) = match &nodes[i].kind {
                Kind::Stage { stage, input, aux } => (stage, input, aux),
                // Its input over the whole rectangle, the members it shows laid onto it.
                Kind::Laid { laid, input } => {
                    ask(*input, need)?;
                    let members = laid.shown(need);
                    runs[i] = if members.is_empty() { Run::Through } else { Run::Laid };
                    shown.insert(i, members);
                    continue;
                }
                Kind::Source(_) => continue,
            };
            // The last node writes the render's own output, so it runs whole.
            runs[i] = match stage.area().filter(|_| i != last) {
                None => Run::Whole,
                Some(area) => match need.intersect(&area) {
                    hit if hit.is_empty() => Run::Through,
                    hit => Run::Within(hit),
                },
            };
            let read = match runs[i] {
                Run::Whole => {
                    ask(*input, stage.input_region(need))?;
                    Some(need)
                }
                // Its input over the whole rectangle, which its result holds as it is outside the area.
                Run::Through | Run::Laid => {
                    ask(*input, need)?;
                    None
                }
                Run::Within(hit) => {
                    ask(*input, need)?;
                    Some(hit)
                }
            };
            if let Some(read) = read {
                for (k, &a) in aux.iter().enumerate() {
                    ask(a, stage.aux_region(k, read))?;
                }
            }
        }
        Ok(Self { nodes, last, rects, hits, runs, shown })
    }

    pub(super) fn output(&self) -> Rect {
        self.rects[self.last].expect("the request")
    }
}
