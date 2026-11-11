//! Many small images of one shape, and their laying on an image: a recipe evaluated a step at a time,
//! each step one pass for every member, where an image of its own would cost each member its passes.

use super::*;

/// The stages of one step, one a member; none where a member passes the step as it is.
pub(super) trait Members: Send + Sync {
    fn get(&self, member: usize) -> Option<&dyn Stage>;
}

impl<S: Stage> Members for Vec<Option<S>> {
    fn get(&self, member: usize) -> Option<&dyn Stage> {
        self[member].as_ref().map(|s| s as &dyn Stage)
    }
}

#[derive(Clone)]
pub(super) enum Step {
    Stage(Arc<dyn Members>),
    /// Each member moved by whole pixels, which costs no pass.
    Moved(Arc<Vec<[i32; 2]>>),
}

pub(super) struct BatchInner {
    pub key: u64,
    pub source: Source,
    /// Where each member lies on the source.
    pub at: Arc<Vec<Rect>>,
    pub steps: Vec<Step>,
    /// Each member's extent before each step, and after the last.
    pub extents: Vec<Arc<Vec<Rect>>>,
}

/// Small images read off one source, each through its own stages of the same kinds: member `k` of
/// every step is evaluated with the others, in one pass a step.
#[derive(Clone)]
pub struct Batch(pub(super) Arc<BatchInner>);

/// The kind every stage of a step shares: its shader, by address as the pipelines name it.
fn kind(stages: &dyn Members, count: usize) -> Option<(&'static str, usize, usize)> {
    let mut kinds = (0..count).filter_map(|k| stages.get(k)).map(|s| (s.name(), s.shader().as_ptr() as usize, s.shader().len()));
    let first = kinds.next()?;
    assert!(kinds.all(|k| k == first), "the stages of one step are of one kind");
    Some(first)
}

fn digest(stages: &dyn Members, count: usize, hasher: &mut DefaultHasher) {
    kind(stages, count).hash(hasher);
    for k in 0..count {
        stages.get(k).map(|s| s.params()).hash(hasher);
    }
}

impl Batch {
    /// The pixels of `source` over each rectangle of `at`, each an image of its own starting at (0, 0).
    /// Panics on a source in tiles, or a rectangle past it: a member is read off one texture.
    pub fn of(source: &Source, at: Vec<Rect>) -> Batch {
        assert_eq!(source.tiles(), 1, "a batch reads one texture");
        assert!(at.iter().all(|r| !r.is_empty() && source.extent().contains(r)), "a member lies on its source");
        let mut hasher = DefaultHasher::new();
        ("batch", source.id(), &at).hash(&mut hasher);
        let extents = at.iter().map(|r| Rect::new(0, 0, r.w, r.h)).collect();
        Batch(Arc::new(BatchInner { key: hasher.finish(), source: source.clone(), at: Arc::new(at), steps: Vec::new(), extents: vec![Arc::new(extents)] }))
    }

    pub fn len(&self) -> usize {
        self.0.at.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Each member's extent after the last step.
    pub fn extents(&self) -> &[Rect] {
        self.0.extents.last().expect("a batch has its source's extents")
    }

    fn with(&self, step: Step, extents: Vec<Rect>, hasher: DefaultHasher) -> Batch {
        let b = &self.0;
        let (mut steps, mut all) = (b.steps.clone(), b.extents.clone());
        steps.push(step);
        all.push(Arc::new(extents));
        Batch(Arc::new(BatchInner { key: hasher.finish(), source: b.source.clone(), at: b.at.clone(), steps, extents: all }))
    }

    /// Each member through its stage of `stages`, one a member and all of one kind; a member given
    /// none passes as it is.
    pub fn then<S: Stage + 'static>(&self, stages: Vec<Option<S>>) -> Batch {
        assert_eq!(stages.len(), self.len(), "one stage a member");
        let mut hasher = DefaultHasher::new();
        (self.0.key, "then").hash(&mut hasher);
        digest(&stages, self.len(), &mut hasher);
        let extents = self.extents().iter().zip(&stages).map(|(e, s)| s.as_ref().map_or(*e, |s| s.extent(*e))).collect();
        self.with(Step::Stage(Arc::new(stages)), extents, hasher)
    }

    /// Each member moved by whole pixels.
    pub fn moved(&self, by: Vec<[i32; 2]>) -> Batch {
        assert_eq!(by.len(), self.len(), "one move a member");
        let mut hasher = DefaultHasher::new();
        (self.0.key, "moved", &by).hash(&mut hasher);
        let extents = self.extents().iter().zip(&by).map(|(e, by)| Rect::new(e.x + by[0], e.y + by[1], e.w, e.h)).collect();
        self.with(Step::Moved(Arc::new(by)), extents, hasher)
    }
}

pub(super) struct LaidInner {
    pub key: u64,
    pub stages: Arc<dyn Members>,
    /// Each member's area, outside which its stage returns its input.
    pub areas: Vec<Rect>,
    /// Each member's layer: past every earlier member its area meets, so one layer's members lie apart.
    pub layers: Vec<u32>,
    pub aux: Vec<Batch>,
}

/// Stages laid on an image together, member `k` over its own `area` and reading member `k` of each
/// auxiliary batch: what a chain of them gives, each in turn, in a few passes for them all.
#[derive(Clone)]
pub struct Laid(pub(super) Arc<LaidInner>);

/// The side of the squares the areas are sorted into, to find the earlier ones an area meets.
const BUCKET: i32 = 256;

/// Each area's layer, in order: one past the highest layer among the earlier areas it meets.
fn layers(areas: &[Rect]) -> Vec<u32> {
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut layers = vec![0u32; areas.len()];
    for (k, a) in areas.iter().enumerate() {
        let cells = |a: &Rect| {
            let (x0, y0, x1, y1) = (a.x.div_euclid(BUCKET), a.y.div_euclid(BUCKET), (a.right() - 1).div_euclid(BUCKET), (a.bottom() - 1).div_euclid(BUCKET));
            (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
        };
        let under = cells(a).filter_map(|c| buckets.get(&c)).flatten().filter(|&&j| !areas[j].intersect(a).is_empty()).map(|&j| layers[j] + 1).max();
        layers[k] = under.unwrap_or(0);
        for c in cells(a) {
            buckets.entry(c).or_default().push(k);
        }
    }
    layers
}

impl Laid {
    /// `stages`, one a member and all of one kind, each naming its `area`; `aux` holds a member for each.
    /// Panics on a stage with no area, or one that changes its input's extent.
    pub fn new<S: Stage + 'static>(stages: Vec<S>, aux: &[Batch]) -> Laid {
        assert!(aux.len() <= MAX_AUX && aux.iter().all(|b| b.len() == stages.len()), "a member of each auxiliary batch for each stage");
        let areas: Vec<Rect> = stages.iter().map(|s| s.area().filter(|a| !a.is_empty()).expect("a laid stage names its area")).collect();
        let stages: Vec<Option<S>> = stages.into_iter().map(Some).collect();
        let mut hasher = DefaultHasher::new();
        digest(&stages, areas.len(), &mut hasher);
        for b in aux {
            b.0.key.hash(&mut hasher);
        }
        Laid(Arc::new(LaidInner { key: hasher.finish(), stages: Arc::new(stages), layers: layers(&areas), areas, aux: aux.to_vec() }))
    }

    pub fn len(&self) -> usize {
        self.0.areas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn key(&self) -> u64 {
        self.0.key
    }

    /// The members a render of `need` shows, in order, each with the part of its area `need` holds.
    pub(super) fn shown(&self, need: Rect) -> Vec<(usize, Rect)> {
        self.0.areas.iter().enumerate().map(|(k, a)| (k, a.intersect(&need))).filter(|(_, hit)| !hit.is_empty()).collect()
    }
}
