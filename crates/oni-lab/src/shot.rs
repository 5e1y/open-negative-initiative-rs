//! `--shot`: one component rendered headless by Slint's software renderer, keycaps included, to a
//! PNG; at 2 pixels a point unless `ONI_LAB_SCALE` says otherwise, as a Retina screen shows it.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint_interpreter::ComponentHandle;

use crate::raster::Raster;
use crate::{compile, compile_source, pick, serve};

/// One software window and no screen; with a `clock`, time is what it says, so a film's animations
/// land on the same frame at every run.
pub struct Headless {
    pub window: Rc<MinimalSoftwareWindow>,
    pub clock: Option<Rc<Cell<Duration>>>,
}

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.clock.as_ref().map_or_else(|| START.with(|s| s.elapsed()), |c| c.get())
    }
}

thread_local! {
    static START: std::time::Instant = std::time::Instant::now();
}

fn size(text: Option<&str>) -> Result<(f32, f32), String> {
    let Some(text) = text else { return Ok((1400.0, 900.0)) };
    let bad = || format!("size {text}: WIDTHxHEIGHT in points");
    let (w, h) = text.split_once(['x', 'X']).ok_or_else(bad)?;
    Ok((w.trim().parse().map_err(|_| bad())?, h.trim().parse().map_err(|_| bad())?))
}

pub fn take(path: &Path, out: &Path, component: Option<&str>, wxh: Option<&str>) -> Result<(), String> {
    let (w, h) = size(wxh)?;
    let scale: f32 = std::env::var("ONI_LAB_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless { window: window.clone(), clock: None })).map_err(|e| e.to_string())?;
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
    let result = match component.and_then(|name| page(path, name)) {
        Some((file, name)) => compile_source(wrapper(&file, &name), &path.with_file_name("shot.slint")),
        None => compile(path),
    };
    if result.has_errors() {
        return Err("the lab does not compile".into());
    }
    let name = if component.is_some_and(|c| c != "Lab") { "Shot" } else { "Lab" };
    let definition = pick(&result, Some(name)).ok_or_else(|| format!("no component {component:?} in the lab"))?;
    let instance = definition.create().map_err(|e| e.to_string())?;
    let raster = Rc::new(Raster::default());
    serve(&instance, &raster);
    window.set_size(slint::LogicalSize::new(w, h));
    instance.show().map_err(|e| e.to_string())?;
    let buffer = instance.window().take_snapshot().map_err(|e| e.to_string())?;
    let file = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?);
    let mut encoder = png::Encoder::new(file, buffer.width(), buffer.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(buffer.as_bytes()).map_err(|e| e.to_string())?;
    eprintln!("oni-lab: {} — {} × {} px", out.display(), buffer.width(), buffer.height());
    Ok(())
}

/// The lab file exporting the component `name`, unless it is the lab's own window.
fn page(lab: &Path, name: &str) -> Option<(String, String)> {
    if name == "Lab" {
        return None;
    }
    let dir = lab.parent()?;
    let needle = format!("export component {name} ");
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let text = std::fs::read_to_string(entry.path()).ok()?;
        text.contains(&needle).then(|| (entry.file_name().to_string_lossy().into_owned(), name.to_string()))
    })
}

/// A window holding one page on the lab's ground and font, `Raster` exported so it is answered.
fn wrapper(file: &str, name: &str) -> String {
    format!(
        "import \"../fonts/Inter.ttf\";\nimport {{ DS }} from \"../ui/ds/tokens.slint\";\n\
         import {{ {name} }} from \"{file}\";\nexport {{ Depth, Raster }} from \"depth.slint\";\n\
         export component Shot inherits Window {{ background: DS.palette-panel; default-font-family: \"Inter\"; {name} {{ }} }}\n"
    )
}
