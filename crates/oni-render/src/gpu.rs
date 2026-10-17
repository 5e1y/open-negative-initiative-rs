//! Every kernel on whatever `wgpu` finds: Metal on macOS, DirectX 12 on Windows, Vulkan elsewhere.
//! Images travel as storage buffers of `vec4<f32>`, rows top first; one `pass` runs any kernel.

use std::collections::HashMap;
use std::sync::Mutex;

use wgpu::util::DeviceExt;

use crate::params::uniform;
use crate::{
    AreaHistogram, ColorClamp, ColorCube, ColorMatrix, CorrectionBiasField, CorrectionCost, Curves, Detail,
    ExposureAdjust, FlatField, Image, LinearGradient, RadialGradient, Rect, SaturationCurve, ScannerDensity,
    ScannerEncode, ScannerOutput, ScreenGamutZebra, Spectrogram, Stage47, Texture, TextureCoeff, Vignette,
};

const PRELUDE: &str = include_str!("wgsl/prelude.wgsl");
const DETAIL_TAPS: &str = include_str!("wgsl/detail_taps.wgsl");
const WORKGROUP: u32 = 64;
const MAX_GROUPS: u32 = 65_535;

/// One kernel: the shared prelude, then `parts`, entry point `main`. Binding 0 is the uniform
/// block, then each buffer `pass` receives, in order.
#[derive(Clone, Copy)]
pub(crate) struct Shader {
    name: &'static str,
    parts: &'static [&'static str],
}

macro_rules! shader {
    ($name:literal) => {
        Shader { name: $name, parts: &[include_str!(concat!("wgsl/", $name, ".wgsl"))] }
    };
    ($name:literal, $lib:expr) => {
        Shader { name: $name, parts: &[$lib, include_str!(concat!("wgsl/", $name, ".wgsl"))] }
    };
}

const STAGE47: Shader = shader!("stage47");
const OUTPUT_CLIP: Shader = shader!("output_clip");
const FLAT_FIELD: Shader = shader!("flat_field");
const CURVES: Shader = shader!("curves");
const SATURATION_CURVE: Shader = shader!("saturation_curve");
const TEXTURE_MOMENTS: Shader = shader!("texture_moments");
const TEXTURE_COEFF: Shader = shader!("texture_coeff");
const TEXTURE: Shader = shader!("texture");
const DETAIL_ROWS: Shader = shader!("detail_rows", DETAIL_TAPS);
const DETAIL_COLUMNS: Shader = shader!("detail_columns", DETAIL_TAPS);
const SPECTROGRAM: Shader = shader!("spectrogram");
const VIGNETTE: Shader = shader!("vignette");
const SCREEN_GAMUT_ZEBRA: Shader = shader!("screen_gamut_zebra");
const RGB_PASSES: Shader = shader!("rgb_passes");
const SCANNER_DENSITY: Shader = shader!("scanner_density");
const SCANNER_OUTPUT: Shader = shader!("scanner_output");
const SCANNER_ENCODE: Shader = shader!("scanner_encode");
const CORRECTION_COST: Shader = shader!("correction_cost");
const CORRECTION_BIAS_FIELD: Shader = shader!("correction_bias_field");
const COLOR_MATRIX: Shader = shader!("color_matrix");
const COLOR_CLAMP: Shader = shader!("color_clamp");
const EXPOSURE_ADJUST: Shader = shader!("exposure_adjust");
const BLEND_WITH_MASK: Shader = shader!("blend_with_mask");
const SOURCE_OVER: Shader = shader!("source_over");
const LINEAR_GRADIENT: Shader = shader!("linear_gradient");
const RADIAL_GRADIENT: Shader = shader!("radial_gradient");
const COLOR_CUBE: Shader = shader!("color_cube");
const AREA_ROWS: Shader = shader!("area_rows");
const AREA_TOTAL: Shader = shader!("area_total");
const HISTOGRAM_COUNT: Shader = shader!("histogram_count");
const HISTOGRAM_SHARE: Shader = shader!("histogram_share");

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Mutex<HashMap<&'static str, wgpu::ComputePipeline>>,
    /// The largest buffer one binding may hold on this adapter.
    pub max_binding: u64,
    pub adapter: String,
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    NoDevice(String),
}

fn head(count: usize, a: usize, b: usize, c: usize) -> [u32; 4] {
    let n = |v: usize| u32::try_from(v).expect("dimension past u32");
    [n(count), n(a), n(b), n(c)]
}

fn same_shape(a: &Image, b: &Image, what: &str) {
    assert!(a.width == b.width && a.height == b.height, "{what}: {}x{} against {}x{}", a.width, a.height, b.width, b.height);
}

/// A uniform row of whole numbers, laid bit for bit where the shader reads a `vec4<u32>`.
fn whole(v: [usize; 4]) -> [f32; 4] {
    v.map(|x| f32::from_bits(u32::try_from(x).expect("dimension past u32")))
}

impl Gpu {
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(Self::open())
    }

    async fn open() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        let info = adapter.get_info();
        // The adapter's own limits: the defaults bind 128 MiB, a third of one 24 MP frame in rgba32float.
        let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), ..Default::default() };
        let (device, queue) = adapter.request_device(&descriptor).await.map_err(|e| GpuError::NoDevice(e.to_string()))?;
        Ok(Self::from_device(device, queue, format!("{} ({:?})", info.name, info.backend)))
    }

    /// The kernels on a device another crate already opened, so one GPU serves the whole app.
    /// Bands follow the binding limit the caller requested with that device.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, adapter: String) -> Self {
        let limits = device.limits();
        let max_binding = limits.max_storage_buffer_binding_size.min(limits.max_buffer_size);
        Self { device, queue, pipelines: Mutex::new(HashMap::new()), max_binding, adapter }
    }

    // MARK: - Plumbing

    fn pipeline(&self, shader: Shader) -> wgpu::ComputePipeline {
        let mut cache = self.pipelines.lock().expect("pipeline cache");
        cache
            .entry(shader.name)
            .or_insert_with(|| {
                let source: String = std::iter::once(PRELUDE).chain(shader.parts.iter().copied()).collect();
                let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(shader.name),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(shader.name),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .clone()
    }

    /// Refuses, by name, a buffer one binding cannot hold: such an image is tiled upstream.
    fn fits(&self, what: &str, texels: usize) {
        let bytes = texels as u64 * 16;
        assert!(bytes <= self.max_binding, "{what}: {bytes} bytes exceed this adapter's binding of {}; tile it", self.max_binding);
    }

    /// A buffer holding `pixels`; never empty, since a binding cannot be.
    pub(crate) fn upload(&self, pixels: &[[f32; 4]]) -> wgpu::Buffer {
        self.fits("upload", pixels.len());
        let bytes: &[u8] = if pixels.is_empty() { &[0; 16] } else { bytemuck::cast_slice(pixels) };
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        })
    }

    /// A zeroed buffer of `len` texels of 16 bytes.
    pub(crate) fn blank(&self, len: usize) -> wgpu::Buffer {
        self.fits("buffer", len);
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (len.max(1) * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Runs `shader` over `invocations`, the uniform block at binding 0 and `buffers` from binding 1.
    pub(crate) fn pass(&self, shader: Shader, uniform: &[u8], buffers: &[&wgpu::Buffer], invocations: usize) {
        let pipeline = self.pipeline(shader);
        let block = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: uniform,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() }];
        entries.extend(
            buffers.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32 + 1, resource: b.as_entire_binding() }),
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(shader.name),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let total = (invocations as u64).div_ceil(WORKGROUP as u64).max(1);
        let x = total.min(MAX_GROUPS as u64);
        let y = total.div_ceil(x);
        assert!(y <= MAX_GROUPS as u64, "{invocations} invocations exceed one dispatch");
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(x as u32, y as u32, 1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// The first `len` texels of `buffer`, read back.
    pub(crate) fn download(&self, buffer: &wgpu::Buffer, len: usize) -> Vec<[f32; 4]> {
        let bytes = (len * 16) as u64;
        if bytes == 0 {
            return Vec::new();
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let out = bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped range")).to_vec();
        readback.unmap();
        out
    }

    /// One pass from images to a fresh output of `len` texels, read back.
    fn run(&self, shader: Shader, uniform: &[u8], inputs: &[&[[f32; 4]]], len: usize, invocations: usize) -> Vec<[f32; 4]> {
        if len == 0 {
            return Vec::new();
        }
        let uploaded: Vec<_> = inputs.iter().map(|p| self.upload(p)).collect();
        let output = self.blank(len);
        let mut buffers: Vec<&wgpu::Buffer> = uploaded.iter().collect();
        buffers.push(&output);
        self.pass(shader, uniform, &buffers, invocations);
        self.download(&output, len)
    }

    /// A per-pixel kernel on images of one shape, the output taking the first one's.
    fn per_pixel(
        &self,
        shader: Shader,
        rows: impl Fn(&Image) -> Vec<[f32; 4]>,
        inputs: &[&Image],
        tables: &[&[[f32; 4]]],
        aux: usize,
    ) -> Image {
        self.banded(shader, rows, inputs[0], inputs, tables, aux)
    }

    /// Runs over `shape` in bands of whole rows each binding can hold; `rows` builds the uniform
    /// rows for a band, which carries its own origin and height. `tables` bind whole after the images.
    fn banded(
        &self,
        shader: Shader,
        rows: impl Fn(&Image) -> Vec<[f32; 4]>,
        shape: &Image,
        inputs: &[&Image],
        tables: &[&[[f32; 4]]],
        aux: usize,
    ) -> Image {
        for other in inputs {
            same_shape(shape, other, shader.name);
        }
        let (w, h) = (shape.width, shape.height);
        if w * h == 0 {
            return shape.with_pixels(Vec::new());
        }
        let band = ((self.max_binding / 16) as usize / w).clamp(1, h);
        let mut out = Vec::with_capacity(w * h);
        for y0 in (0..h).step_by(band) {
            let y1 = (y0 + band).min(h);
            let n = (y1 - y0) * w;
            let origin = [shape.origin[0], shape.origin[1] + (h - y1) as f32];
            let part = Image { width: w, height: y1 - y0, origin, pixels: Vec::new() };
            let block = uniform(head(n, w, y1 - y0, aux), &rows(&part));
            let mut buffers: Vec<&[[f32; 4]]> = inputs.iter().map(|i| &i.pixels[y0 * w..y1 * w]).collect();
            buffers.extend_from_slice(tables);
            out.extend(self.run(shader, &block, &buffers, n, n));
        }
        shape.with_pixels(out)
    }

    // MARK: - pipeline.metal

    /// Runs the fused kernel on `pixels` and reads the result back, in rgba32float throughout.
    pub fn stage47(&self, pixels: &[[f32; 4]], k: &Stage47) -> Vec<[f32; 4]> {
        let chunk = (self.max_binding / 16) as usize;
        let mut out = Vec::with_capacity(pixels.len());
        for part in pixels.chunks(chunk.max(1)) {
            let packed = k.packed(part.len() as u32);
            out.extend(self.run(STAGE47, bytemuck::bytes_of(&packed), &[part], part.len(), part.len()));
        }
        out
    }

    /// Every per-pixel kernel writes its input's extent and reads its own pixel alone.
    pub fn output_clip(&self, image: &Image) -> Image {
        self.per_pixel(OUTPUT_CLIP, |_| Vec::new(), &[image], &[], 0)
    }

    /// `mask` is the flat-field already fitted to `image`'s extent, as `FlatField.fitted` lays it.
    pub fn flat_field(&self, image: &Image, mask: &Image, k: &FlatField) -> Image {
        self.per_pixel(FLAT_FIELD, |_| k.rows(), &[image, mask], &[], 0)
    }

    // MARK: - curves.metal

    pub fn curves(&self, image: &Image, k: &Curves) -> Image {
        assert!(k.rgb_lut.len() >= 2 && k.rgb_lut.len() == k.luma_lut.len(), "curves: two tables of one length");
        let bent = [k.bent[0], k.bent[1], k.bent[2], 0.0];
        self.per_pixel(CURVES, |_| vec![bent], &[image], &[&k.rgb_lut, &k.luma_lut], k.rgb_lut.len())
    }

    // MARK: - chroma.metal

    pub fn saturation_curve(&self, image: &Image, k: &SaturationCurve) -> Image {
        self.per_pixel(SATURATION_CURVE, |_| k.rows(), &[image], &[], 0)
    }

    pub fn texture_moments(&self, luma: &Image) -> Image {
        self.per_pixel(TEXTURE_MOMENTS, |_| Vec::new(), &[luma], &[], 0)
    }

    pub fn texture_coeff(&self, moments: &Image, k: &TextureCoeff) -> Image {
        self.per_pixel(TEXTURE_COEFF, |_| vec![[k.relative, 0.0, 0.0, 0.0]], &[moments], &[], 0)
    }

    pub fn texture(&self, image: &Image, narrow_luma: &Image, coeff: &Image, k: &Texture) -> Image {
        self.per_pixel(TEXTURE, |_| vec![[k.gain, 0.0, 0.0, 0.0]], &[image, narrow_luma, coeff], &[], 0)
    }

    /// `detailRows` then `detailColumns`, output on `image`'s extent, reading `max(reach)` columns and
    /// `margin()` rows away through an edge clamp, as `clampedToExtent` and the rows' ROI do.
    pub fn detail(&self, image: &Image, k: &Detail) -> Image {
        let (w, h) = (image.width, image.height);
        if w * h == 0 {
            return image.clone();
        }
        let wide = w + 2 * k.disk as usize;
        let source = self.upload(&image.pixels);
        let rows = self.blank(wide * h);
        self.pass(DETAIL_ROWS, &uniform(head(wide * h, wide, h, w), &k.rows()), &[&source, &rows], wide * h);
        let output = self.blank(w * h);
        self.pass(DETAIL_COLUMNS, &uniform(head(w * h, w, h, wide), &k.rows()), &[&source, &rows, &output], w * h);
        image.with_pixels(self.download(&output, w * h))
    }

    pub fn spectrogram(&self, image: &Image, k: &Spectrogram) -> Image {
        self.per_pixel(SPECTROGRAM, |_| k.rows(), &[image], &[], 0)
    }

    /// Per pixel, reading each pixel's Core Image coordinate: `image.origin` places the centre.
    pub fn vignette(&self, image: &Image, k: &Vignette) -> Image {
        self.per_pixel(VIGNETTE, |band| [vec![[band.origin[0], band.origin[1], 0.0, 0.0]], k.rows()].concat(), &[image], &[], 0)
    }

    // MARK: - gamut.metal

    /// Per pixel on one extent, the stripes laid on Core Image coordinates from `picture.origin`.
    pub fn screen_gamut_zebra(&self, picture: &Image, shown: &Image, k: &ScreenGamutZebra) -> Image {
        let rows = |band: &Image| [vec![[band.origin[0], band.origin[1], 0.0, 0.0]], k.rows()].concat();
        self.per_pixel(SCREEN_GAMUT_ZEBRA, rows, &[picture, shown], &[], 0)
    }

    // MARK: - rgbPasses.metal

    /// The three passes already laid on the green pass's extent, as `RGBPasses.composed` lays them.
    pub fn rgb_passes(&self, red: &Image, green: &Image, blue: &Image) -> Image {
        self.per_pixel(RGB_PASSES, |_| Vec::new(), &[red, green, blue], &[], 0)
    }

    // MARK: - scanner.metal

    pub fn scanner_density(&self, image: &Image, k: &ScannerDensity) -> Image {
        self.per_pixel(SCANNER_DENSITY, |_| k.rows(), &[image], &[], 0)
    }

    pub fn scanner_output(&self, code: &Image, warmth: &Image, k: &ScannerOutput) -> Image {
        self.per_pixel(SCANNER_OUTPUT, |_| vec![[k.warmth_share, k.lift, k.contrast, 0.0]], &[code, warmth], &[], 0)
    }

    pub fn scanner_encode(&self, image: &Image, k: &ScannerEncode) -> Image {
        self.per_pixel(SCANNER_ENCODE, |_| vec![[k.root, 0.0, 0.0, 0.0]], &[image], &[], 0)
    }

    // MARK: - correctionSearch.metal

    /// The cost map, (SSD, offset x, offset y, 1) per candidate on (0, 0, grid_size, grid_size); the
    /// source is read anywhere in the rings' union, so it binds whole, as the ROI asks.
    pub fn correction_cost(&self, source: &Image, k: &CorrectionCost, grid_size: usize) -> Image {
        let n = grid_size * grid_size;
        let block = uniform(head(n, grid_size, source.width, source.height), &k.rows(source.origin));
        Image::new(grid_size, grid_size, self.run(CORRECTION_COST, &block, &[&source.pixels], n, n))
    }

    /// Per pixel, the bias read at each pixel's Core Image coordinate from `patch.origin`.
    pub fn correction_bias_field(&self, patch: &Image, k: &CorrectionBiasField) -> Image {
        self.per_pixel(CORRECTION_BIAS_FIELD, |band| k.rows(band.origin), &[patch], &[], 0)
    }

    // MARK: - Core Image built-ins

    pub fn color_matrix(&self, image: &Image, k: &ColorMatrix) -> Image {
        self.per_pixel(COLOR_MATRIX, |_| k.rows(), &[image], &[], 0)
    }

    pub fn color_clamp(&self, image: &Image, k: &ColorClamp) -> Image {
        self.per_pixel(COLOR_CLAMP, |_| vec![k.min_components, k.max_components], &[image], &[], 0)
    }

    pub fn exposure_adjust(&self, image: &Image, k: &ExposureAdjust) -> Image {
        self.per_pixel(EXPOSURE_ADJUST, |_| vec![[k.ev, 0.0, 0.0, 0.0]], &[image], &[], 0)
    }

    /// The three on one extent: the caller has placed the patch and its mask on the background's.
    pub fn blend_with_mask(&self, image: &Image, background: &Image, mask: &Image) -> Image {
        self.per_pixel(BLEND_WITH_MASK, |_| Vec::new(), &[image, background, mask], &[], 0)
    }

    pub fn source_over(&self, image: &Image, background: &Image) -> Image {
        self.per_pixel(SOURCE_OVER, |_| Vec::new(), &[image, background], &[], 0)
    }

    /// The gradient rendered on the extent of `width` by `height` at `origin`.
    pub fn linear_gradient(&self, origin: [f32; 2], width: usize, height: usize, k: &LinearGradient) -> Image {
        self.generated(LINEAR_GRADIENT, origin, width, height, &k.rows())
    }

    pub fn radial_gradient(&self, origin: [f32; 2], width: usize, height: usize, k: &RadialGradient) -> Image {
        self.generated(RADIAL_GRADIENT, origin, width, height, &k.rows())
    }

    fn generated(&self, shader: Shader, origin: [f32; 2], width: usize, height: usize, k: &[[f32; 4]]) -> Image {
        let shape = Image { width, height, origin, pixels: Vec::new() };
        self.banded(shader, |band| [vec![[band.origin[0], band.origin[1], 0.0, 0.0]], k.to_vec()].concat(), &shape, &[], &[], 0)
    }

    pub fn color_cube(&self, image: &Image, k: &ColorCube) -> Image {
        let d = k.cube_dimension;
        assert!(d >= 2 && k.cube_data.len() == d * d * d, "color cube: {} nodes for dimension {d}", k.cube_data.len());
        self.per_pixel(COLOR_CUBE, |_| Vec::new(), &[image], &[&k.cube_data], d)
    }

    /// `CIAreaAverage` and `CIAreaMaximum` over `rect`, whole pixels in the buffer's frame.
    pub fn area_average_and_maximum(&self, image: &Image, rect: Rect) -> ([f32; 4], [f32; 4]) {
        check_rect(image, rect);
        let source = self.upload(&image.pixels);
        let partial = self.blank(2 * rect.height);
        let block = uniform(head(rect.height, image.width, 0, 0), &[whole([rect.x, rect.y, rect.width, rect.height])]);
        self.pass(AREA_ROWS, &block, &[&source, &partial], rect.height);
        let output = self.blank(2);
        self.pass(AREA_TOTAL, &uniform(head(rect.height, rect.width * rect.height, 0, 0), &[]), &[&partial, &output], 1);
        let out = self.download(&output, 2);
        (out[0], out[1])
    }

    pub fn area_average(&self, image: &Image, rect: Rect) -> [f32; 4] {
        self.area_average_and_maximum(image, rect).0
    }

    pub fn area_maximum(&self, image: &Image, rect: Rect) -> [f32; 4] {
        self.area_average_and_maximum(image, rect).1
    }

    /// `CIAreaHistogram` over `rect`: `k.count` texels, each channel's share of the pixels.
    pub fn area_histogram(&self, image: &Image, rect: Rect, k: &AreaHistogram) -> Vec<[f32; 4]> {
        check_rect(image, rect);
        let n = rect.width * rect.height;
        let source = self.upload(&image.pixels);
        let counts = self.blank(k.count);
        let block = uniform(head(n, image.width, k.count, 0), &[whole([rect.x, rect.y, rect.width, rect.height])]);
        self.pass(HISTOGRAM_COUNT, &block, &[&source, &counts], n);
        let output = self.blank(k.count);
        self.pass(HISTOGRAM_SHARE, &uniform(head(k.count, n, 0, 0), &[[k.scale, 0.0, 0.0, 0.0]]), &[&counts, &output], k.count);
        self.download(&output, k.count)
    }
}

fn check_rect(image: &Image, rect: Rect) {
    assert!(
        rect.width > 0 && rect.height > 0 && rect.x + rect.width <= image.width && rect.y + rect.height <= image.height,
        "{rect:?} outside {}x{}",
        image.width,
        image.height
    );
}
