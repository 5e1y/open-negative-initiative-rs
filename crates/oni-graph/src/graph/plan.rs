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
        rects[last] = Some(request);
        for i in (0..n).rev() {
            let Some(need) = rects[i] else { continue };
            let Kind::Stage { stage, input, aux } = &nodes[i].kind else { continue };
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
            ask(*input, stage.input_region(need))?;
            for (k, &a) in aux.iter().enumerate() {
                ask(a, stage.aux_region(k, need))?;
            }
        }
        Ok(Self { nodes, last, rects, hits })
    }

    pub(super) fn output(&self) -> Rect {
        self.rects[self.last].expect("the request")
    }
}
