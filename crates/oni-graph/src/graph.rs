//! Sources, chains, and their evaluation by region on the GPU.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::stage::{AuxFrame, Frame, MAX_AUX, PRELUDE, aux_prelude};
use crate::stages::{AnchoredLanczos, Axis, Lanczos};
use crate::texture::{BYTES_PER_PIXEL, FloatTexture, Meter, Pool, Precision};
use crate::{Rect, Stage};

mod errors;
mod source;
pub use source::Uploader;
mod chain;
mod plan;
mod cache;
mod device;
pub use device::{Gpu, gpu};
mod rendering;
mod dispatch;
mod half;

/// Metal's and Direct3D 12's largest texture side: a pixel-shift frame of 16 736 px goes in tiles.
pub const TEXTURE_SIDE: u32 = 16_384;

/// Largest upload written at once; a whole 16 384 px tile is 4 GiB, past what one staging buffer may hold.
const UPLOAD_BAND_BYTES: u64 = 64 << 20;

#[derive(Debug, Clone)]
pub enum Error {
    NoAdapter(String),
    NoDevice(String),
    /// The request leaves the image's extent, or is empty.
    RegionOutside { region: Rect, extent: Rect },
    /// Stage `stage` reads nothing of its input's extent for this request.
    EmptyInput { stage: usize },
    /// Not even one pixel of output fits the GPU's texture side.
    TooLarge,
    /// The pixel count does not match the size given.
    Size { expected: u64, got: usize },
    /// A readback the GPU did not deliver: the device lost, or its buffer never made.
    Lost(String),
}

/// Counters a check reads: what the GPU was asked to do, and what the pool holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub dispatches: u64,
    pub copies: u64,
    pub textures_created: u64,
    pub live_bytes: i64,
    /// The most bytes the graph's textures held at once.
    pub peak_bytes: i64,
    pub idle_textures: usize,
    pub pipelines: usize,
}

struct SourceInner {
    id: u64,
    extent: Rect,
    tiles: Vec<(Rect, FloatTexture)>,
}

/// Pixels resident on the GPU, in tiles no side of which passes the graph's texture side.
#[derive(Clone)]
pub struct Source(Arc<SourceInner>);

/// A stage and the further images it reads beside its input.
#[derive(Clone)]
struct Node {
    stage: Arc<dyn Stage>,
    aux: Vec<Image>,
}

/// A source and the stages that follow it: a recipe, evaluated only when a region is asked for.
#[derive(Clone)]
pub struct Image {
    source: Source,
    stages: Vec<Node>,
}

enum Kind<'a> {
    Source(&'a Source),
    Stage { stage: &'a dyn Stage, input: usize, aux: Vec<usize> },
}

/// One node of a render: its key folds in every node upstream of it, so equal keys are one node.
struct Flat<'a> {
    key: u64,
    extent: Rect,
    kind: Kind<'a>,
}

/// What one evaluation needs: each node's rectangle, `None` where nothing reads it, and the cache's hits.
struct Plan<'a> {
    nodes: Vec<Flat<'a>>,
    last: usize,
    rects: Vec<Option<Rect>>,
    hits: Vec<Option<(FloatTexture, Rect)>>,
}

struct Entry {
    rect: Rect,
    texture: FloatTexture,
}

/// Stage results held for the next render by node key: a change on a late stage reruns only what
/// follows it. One cache serves one view of one graph; dropping it frees its textures.
#[derive(Default)]
pub struct Cache {
    entries: HashMap<u64, Entry>,
    /// Holds only the results a changed node reads, see `Cache::frontier`.
    frontier: bool,
    /// The last render's node keys, and the ones it keeps; `None` keeps them all.
    seen: HashSet<u64>,
    keep: Option<HashSet<u64>>,
}

/// A layout for each count of auxiliary inputs.
struct Layout {
    group: wgpu::BindGroupLayout,
    pipeline: wgpu::PipelineLayout,
}

/// The GPU, the compiled stages and the texture pool.
pub struct Graph {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layouts: HashMap<usize, Layout>,
    pipelines: HashMap<(usize, usize, usize), wgpu::ComputePipeline>,
    pool: Pool,
    max_side: u32,
    max_buffer: u64,
    dispatches: u64,
    copies: u64,
    uniforms: Uniforms,
    /// The pass that packs a render into half floats, made at its first use.
    packer: Option<half::Packer>,
    door: Option<Arc<dyn Door>>,
    pub adapter: String,
}

/// Every dispatch's uniforms written in turn into one buffer, so no stage creates buffers of its own;
/// a submission's are gathered here and copied in once, as each write takes the queue's lock.
struct Uniforms {
    buffer: wgpu::Buffer,
    at: u64,
    align: u64,
    staged: Vec<u8>,
    staged_from: u64,
    /// Where the submission being recorded staged its first uniforms, and whether it wrapped since.
    first: u64,
    wrapped: bool,
}

/// The ring's size: a render writes about a kilobyte a stage; a sheet of many images can stage more.
const UNIFORM_RING: u64 = 4 << 20;

static SOURCE_IDS: AtomicU64 = AtomicU64::new(1);

/// What a graph passes before it hands work to its queue, when another owner of the device must at
/// times keep the queue still (a window reconfiguring its surface); `enter` false lets it pass.
pub trait Door: Send + Sync {
    fn enter(&self) -> bool;
    fn leave(&self);
}
