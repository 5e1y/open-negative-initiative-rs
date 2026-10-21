//! A chain of stages over a source, flattened into nodes shared by identity.

use super::*;

pub(super) fn intern<'a>(nodes: &mut Vec<Flat<'a>>, seen: &mut HashMap<u64, usize>, flat: Flat<'a>) -> usize {
    if let Some(&i) = seen.get(&flat.key) {
        return i;
    }
    seen.insert(flat.key, nodes.len());
    nodes.push(flat);
    nodes.len() - 1
}

impl Image {
    pub fn new(source: &Source) -> Self {
        Self { source: source.clone(), stages: Vec::new() }
    }

    pub fn then(mut self, stage: impl Stage + 'static) -> Self {
        self.stages.push(Node { stage: Arc::new(stage), aux: Vec::new() });
        self
    }

    /// `stage` reading `aux` beside its input, through `load1(q)`, `load2(q)`, ... in each one's own
    /// coordinates. A node `aux` shares with this chain is evaluated once per render. Panics past `MAX_AUX`.
    pub fn then_reading(mut self, stage: impl Stage + 'static, aux: &[Image]) -> Self {
        assert!(aux.len() <= MAX_AUX, "{} auxiliary inputs, at most {MAX_AUX}", aux.len());
        self.stages.push(Node { stage: Arc::new(stage), aux: aux.to_vec() });
        self
    }

    /// Lanczos-3 by `scale` on both axes, each side rounded to a whole pixel; the result starts at 0.
    /// Panics on a scale that is not finite and positive, which has no picture to give.
    pub fn lanczos(self, scale: f64) -> Self {
        assert!(scale.is_finite() && scale > 0.0, "Lanczos scale {scale}");
        let e = self.extent();
        let side = |s: u32| ((s as f64 * scale).round() as u32).max(1);
        self.then(Lanczos { axis: Axis::X, origin: e.x, from: e.w, to: side(e.w) })
            .then(Lanczos { axis: Axis::Y, origin: e.y, from: e.h, to: side(e.h) })
    }

    /// `CILanczosScaleTransform` by `scale`: see `AnchoredLanczos`; the result starts at 0.
    pub fn lanczos_anchored(self, scale: f64) -> Self {
        self.lanczos_shifted(scale, (0.0, 0.0))
    }

    /// `lanczos_anchored` with the left and bottom edges `shift` output pixels in, each under one.
    pub fn lanczos_shifted(self, scale: f64, shift: (f64, f64)) -> Self {
        assert!(scale.is_finite() && scale > 0.0, "Lanczos scale {scale}");
        let e = self.extent();
        self.then(AnchoredLanczos { axis: Axis::X, origin: e.x, from: e.w, scale, shift: shift.0 })
            .then(AnchoredLanczos { axis: Axis::Y, origin: e.y, from: e.h, scale, shift: shift.1 })
    }

    pub fn extent(&self) -> Rect {
        self.stages.iter().fold(self.source.extent(), |e, node| node.stage.extent(e))
    }

    /// The stages of the chain itself, auxiliary images apart.
    pub fn stages(&self) -> usize {
        self.stages.len()
    }

    /// Every node this image reads, inputs before the stages reading them, each once; returns the last.
    pub(super) fn flatten<'a>(&'a self, nodes: &mut Vec<Flat<'a>>, seen: &mut HashMap<u64, usize>) -> usize {
        let mut hasher = DefaultHasher::new();
        ("source", self.source.0.id).hash(&mut hasher);
        let key = hasher.finish();
        let mut at = intern(nodes, seen, Flat { key, extent: self.source.extent(), kind: Kind::Source(&self.source) });
        for node in &self.stages {
            let aux: Vec<usize> = node.aux.iter().map(|a| a.flatten(nodes, seen)).collect();
            let stage = node.stage.as_ref();
            let mut hasher = DefaultHasher::new();
            nodes[at].key.hash(&mut hasher);
            stage.name().hash(&mut hasher);
            stage.shader().hash(&mut hasher);
            stage.params().hash(&mut hasher);
            for &a in &aux {
                nodes[a].key.hash(&mut hasher);
            }
            let key = hasher.finish();
            let extent = stage.extent(nodes[at].extent);
            at = intern(nodes, seen, Flat { key, extent, kind: Kind::Stage { stage, input: at, aux } });
        }
        at
    }
}
