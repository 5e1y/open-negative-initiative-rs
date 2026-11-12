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
        self.stages.push(Node::Stage { stage: Arc::new(stage), aux: Vec::new() });
        self
    }

    /// `stage` reading `aux` beside its input, through `load1(q)`, `load2(q)`, ... in each one's own
    /// coordinates. A node `aux` shares with this chain is evaluated once per render. Panics past `MAX_AUX`.
    pub fn then_reading(mut self, stage: impl Stage + 'static, aux: &[Image]) -> Self {
        assert!(aux.len() <= MAX_AUX, "{} auxiliary inputs, at most {MAX_AUX}", aux.len());
        self.stages.push(Node::Stage { stage: Arc::new(stage), aux: aux.to_vec() });
        self
    }

    /// `laid`'s members laid on the image, each over its own area, in a few passes for them all.
    pub fn then_laid(mut self, laid: &Laid) -> Self {
        self.stages.push(Node::Laid(laid.clone()));
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
        self.stages.iter().fold(self.source.extent(), |e, node| match node {
            Node::Stage { stage, .. } => stage.extent(e),
            Node::Laid(_) => e,
        })
    }

    /// The stages of the chain itself, auxiliary images apart.
    pub fn stages(&self) -> usize {
        self.stages.len()
    }

    /// The recipe's identity: every source, stage, parameter and auxiliary image it reads, in one key.
    pub fn key(&self) -> u64 {
        let (mut nodes, mut seen) = (Vec::new(), HashMap::new());
        let last = self.flatten(&mut nodes, &mut seen);
        nodes[last].key
    }

    /// Every node this image reads, inputs before the stages reading them, each once; returns the last.
    pub(super) fn flatten<'a>(&'a self, nodes: &mut Vec<Flat<'a>>, seen: &mut HashMap<u64, usize>) -> usize {
        let mut hasher = DefaultHasher::new();
        ("source", self.source.0.id).hash(&mut hasher);
        let key = hasher.finish();
        let mut at = intern(nodes, seen, Flat { key, extent: self.source.extent(), kind: Kind::Source(&self.source) });
        for node in &self.stages {
            let (stage, aux) = match node {
                Node::Stage { stage, aux } => (stage.as_ref(), aux),
                Node::Laid(laid) => {
                    let mut hasher = DefaultHasher::new();
                    (nodes[at].key, "laid", laid.key()).hash(&mut hasher);
                    let extent = nodes[at].extent;
                    at = intern(nodes, seen, Flat { key: hasher.finish(), extent, kind: Kind::Laid { laid, input: at } });
                    continue;
                }
            };
            let aux: Vec<usize> = aux.iter().map(|a| a.flatten(nodes, seen)).collect();
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
