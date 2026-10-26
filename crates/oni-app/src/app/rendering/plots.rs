//! The plots over a histogram: the picture's, the levels' and curves' inputs, each traced at its size
//! on one thread that keeps its buffers and writes each plot's texture in place.

use super::*;
use crate::editors::Plot;
use crate::histogram::{PlotSize, Tracer};
use oni_measure::histogram::Histogram;
use std::sync::{OnceLock, mpsc};

/// The levels' and curves' traces asked last: one overtaken by a later reading is dropped.
static INPUT_TRACES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Each slot's clears: a trace asked before one lands nowhere, so a photo left never shows its plots
/// on the next. Its texture goes with the clear, so the next trace imports a new one.
static CLEARS: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

/// The slot's clears so far, which a trace keeps from its ask to its landing.
pub(super) fn clears(slot: Slot) -> u64 {
    CLEARS[slot as usize].load(Ordering::SeqCst)
}

fn cleared(slot: Slot) {
    CLEARS[slot as usize].fetch_add(1, Ordering::SeqCst);
}

/// Whether a trace asked at `asked` clears may still land.
fn lands(slot: Slot, asked: u64) -> bool {
    clears(slot) == asked
}

/// `traced` shown unless the slot was cleared since its trace was asked.
pub(super) fn land_since(w: &MainWindow, slot: Slot, asked: u64, traced: Traced) {
    if lands(slot, asked) {
        land(w, slot, traced);
    }
}

/// The four plots a histogram is traced into.
#[derive(Clone, Copy)]
pub(super) enum Slot {
    Output,
    Saturation,
    Levels,
    Curves,
}

/// A trace's result for the interface thread: nothing laid out, pixels on a window that reads back,
/// or the texture written, with the texture itself when it is new to the interface.
pub(super) enum Traced {
    Empty,
    Pixels(SharedPixelBuffer<Rgba8Pixel>),
    Written(Option<wgpu::Texture>),
}

/// The device and queue a plot is written with; none on a window that reads back.
pub(super) type Writer = Option<(wgpu::Device, wgpu::Queue)>;

/// The plotting thread's own: the tracer's buffers and each slot's texture, kept while its size holds.
#[derive(Default)]
pub(super) struct Plotter {
    tracer: Tracer,
    textures: [Option<wgpu::Texture>; 4],
}

type Job = Box<dyn FnOnce(&mut Plotter) + Send>;

/// Runs `job` on the plotting thread, after every job asked before it.
pub(super) fn plot(job: impl FnOnce(&mut Plotter) + Send + 'static) {
    static JOBS: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    let jobs = JOBS.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("plots".into())
            .spawn(move || {
                let mut plotter = Plotter::default();
                for job in rx {
                    job(&mut plotter);
                }
            })
            .expect("the plotting thread starts");
        tx
    });
    let _ = jobs.send(Box::new(job));
}

impl Plotter {
    /// `histogram` traced `w` by `h` points at `scale`, `focus` its channel in front.
    pub(super) fn trace(&mut self, writer: &Writer, slot: Slot, histogram: &Histogram, focus: Option<usize>, (w, h): (f64, f64), scale: f32) -> Traced {
        if w < 1.0 || h < 1.0 {
            self.textures[slot as usize] = None;
            return Traced::Empty;
        }
        let size = PlotSize::of(w as f32, h as f32, scale);
        let (width, height) = (size.width as u32, size.height as u32);
        let bytes = self.tracer.raster(histogram, focus, size);
        let Some((device, queue)) = writer else { return Traced::Pixels(SharedPixelBuffer::clone_from_slice(bytes, width, height)) };
        crate::histogram::straighten(bytes);
        let kept = &mut self.textures[slot as usize];
        let fresh = kept.as_ref().is_none_or(|t| (t.width(), t.height()) != (width, height));
        let texture = match kept {
            Some(t) if !fresh => t,
            _ => kept.insert(target(device, width, height)),
        };
        let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) };
        queue.write_texture(texture.as_image_copy(), bytes, layout, texture.size());
        Traced::Written(fresh.then(|| texture.clone()))
    }
}

/// A trace shown: a new texture imported, its revision bumped so the picture redraws.
pub(super) fn land(w: &MainWindow, slot: Slot, traced: Traced) {
    let image = match traced {
        Traced::Empty => Some(Image::default()),
        Traced::Pixels(p) => Some(Image::from_rgba8_premultiplied(p)),
        // A texture refused is let go, so the next trace makes another rather than drawing nowhere.
        Traced::Written(fresh) => fresh.map(|t| {
            Image::try_from(t).unwrap_or_else(|e| {
                crate::timing::line(&format!("a plot cannot take its texture: {e}"));
                forgotten(slot)
            })
        }),
    };
    let shown: (fn(&Plots, Image), fn(&Plots) -> i32, fn(&Plots, i32)) = match slot {
        Slot::Output => (|p, i| p.set_output(i), |p| p.get_output_revision(), |p, r| p.set_output_revision(r)),
        Slot::Saturation => (|p, i| p.set_saturation(i), |p| p.get_saturation_revision(), |p, r| p.set_saturation_revision(r)),
        Slot::Levels => (|p, i| p.set_levels(i), |p| p.get_levels_revision(), |p, r| p.set_levels_revision(r)),
        Slot::Curves => (|p, i| p.set_curves(i), |p| p.get_curves_revision(), |p, r| p.set_curves_revision(r)),
    };
    let p = w.global::<Plots>();
    if let Some(image) = image {
        shown.0(&p, image);
    }
    shown.2(&p, shown.1(&p).wrapping_add(1));
}

/// The slot's texture let go, so its next trace makes and imports a new one.
fn forgotten(slot: Slot) -> Image {
    plot(move |p| p.textures[slot as usize] = None);
    Image::default()
}

/// A plot emptied from the interface thread, its texture let go with it.
pub(super) fn clear(w: &MainWindow, slot: Slot) {
    cleared(slot);
    land(w, slot, Traced::Empty);
    let _ = forgotten(slot);
}

/// A texture a plot is written into and Slint samples: `Image::try_from` takes this format and
/// refuses one without `RENDER_ATTACHMENT`.
fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("plot"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

impl Ctl {
    /// The device and queue the plots are written with; none on a window that reads back.
    pub(super) fn plot_writer(&self) -> Writer {
        let s = self.state.borrow();
        (!s.gpu.reads_back).then(|| (s.gpu.device.clone(), s.gpu.queue.clone()))
    }

    /// The levels' and the manual curves' plots over their input distributions, each with its own
    /// channel in focus; drawn again only when a reading lands, a channel tab moves or a plot is laid out.
    pub(crate) fn publish_inputs(&self) {
        let Some(w) = self.window() else { return };
        let focus = {
            let g = self.editors.gestures.borrow();
            (g.levels_channel().histogram_channel(), g.curve_channel().histogram_channel())
        };
        let inputs = {
            let mut s = self.state.borrow_mut();
            if s.requests.inputs_drawn == Some(focus) {
                return;
            }
            s.requests.inputs_drawn = Some(focus);
            s.requests.inputs.clone()
        };
        let (levels, curves) = (crate::editors::size(self, Plot::LevelsHistogram), crate::editors::size(self, Plot::Curves));
        let Some(i) = inputs else {
            clear(&w, Slot::Levels);
            clear(&w, Slot::Curves);
            return;
        };
        // Traced off this thread, 1.5 ms a plot, as the picture's histogram is.
        let (scale, ticket, writer) = (w.window().scale_factor(), INPUT_TRACES.fetch_add(1, Ordering::SeqCst) + 1, self.plot_writer());
        let since = (clears(Slot::Levels), clears(Slot::Curves));
        plot(move |p| {
            if INPUT_TRACES.load(Ordering::SeqCst) != ticket {
                return;
            }
            let levels = p.trace(&writer, Slot::Levels, &i.levels, focus.0, (levels.w, levels.h), scale);
            let curves = p.trace(&writer, Slot::Curves, &i.curves, focus.1, (curves.w, curves.h), scale);
            with_shell(move |c| {
                let Some(w) = c.window() else { return };
                land_since(&w, Slot::Levels, since.0, levels);
                land_since(&w, Slot::Curves, since.1, curves);
            });
        });
    }

    /// A plot laid out at a new size is traced again at it.
    pub(crate) fn plot_resized(&self, plot: Plot) {
        match plot {
            Plot::Curves | Plot::LevelsHistogram => {
                self.state.borrow_mut().requests.inputs_drawn = None;
                self.publish_inputs();
            }
            Plot::Saturation => self.state.borrow_mut().histogram_dirty = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trace_asked_before_a_clear_lands_nowhere() {
        let (levels, curves) = (clears(Slot::Levels), clears(Slot::Curves));
        assert!(lands(Slot::Levels, levels), "a trace with no clear since lands");
        cleared(Slot::Levels);
        assert!(!lands(Slot::Levels, levels), "the photo left drops its trace");
        // Twins: the slot not cleared, and a trace asked after the clear, both land.
        assert!(lands(Slot::Curves, curves));
        assert!(lands(Slot::Levels, clears(Slot::Levels)));
    }
}
