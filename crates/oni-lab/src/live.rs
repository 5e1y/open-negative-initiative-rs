//! The live window: the lab's and the design system's `.slint` files watched, the component rebuilt
//! into the same window on a save, and every tunable the hand moved carried across.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance, Value};

use crate::raster::Raster;
use crate::{compile, lab_dir, pick, serve};

/// How often the sources are looked at: a save shows within a quarter second.
const POLL: Duration = Duration::from_millis(250);

type Globals = HashMap<(String, String), Value>;

struct Live {
    instance: ComponentInstance,
    /// The globals as the file set them, to tell a value the hand moved from a default.
    defaults: Globals,
    stamps: Vec<(PathBuf, SystemTime)>,
}

pub fn run(path: &Path) -> Result<(), String> {
    backend().map_err(|e| format!("no window backend: {e}"))?;
    let raster = Rc::new(Raster::default());
    let result = compile(path);
    let definition = pick(&result, Some("Lab")).or_else(|| pick(&result, None)).ok_or("lab.slint has no component")?;
    let instance = definition.create().map_err(|e| e.to_string())?;
    serve(&instance, &raster);
    instance.show().map_err(|e| e.to_string())?;
    let defaults = globals(&definition, &instance);
    let live = Rc::new(RefCell::new(Live { instance, defaults, stamps: stamps() }));
    let timer = slint::Timer::default();
    let path = path.to_path_buf();
    timer.start(slint::TimerMode::Repeated, POLL, move || {
        let now = stamps();
        let mut live = live.borrow_mut();
        if now != live.stamps {
            live.stamps = now;
            reload(&mut live, &path, &raster);
        }
    });
    slint_interpreter::run_event_loop().map_err(|e| e.to_string())
}

/// femtovg on wgpu, as the app; `ONI_BACKGROUND=1` opens the window behind, the focus left where it is.
fn backend() -> Result<(), slint::PlatformError> {
    let selector = slint::BackendSelector::new().renderer_name("femtovg-wgpu".into());
    #[cfg(target_os = "macos")]
    if std::env::var_os("ONI_BACKGROUND").is_some_and(|v| v == "1") {
        use slint::winit_030::winit::platform::macos::EventLoopBuilderExtMacOS;
        let mut builder = slint::winit_030::winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
        builder.with_activate_ignoring_other_apps(false);
        return selector
            .with_winit_event_loop_builder(builder)
            .with_winit_window_attributes_hook(|a| a.with_active(false))
            .select();
    }
    selector.select()
}

fn reload(live: &mut Live, path: &Path, raster: &Rc<Raster>) {
    let result = compile(path);
    if result.has_errors() {
        return;
    }
    let Some(definition) = pick(&result, Some("Lab")).or_else(|| pick(&result, None)) else { return };
    let instance = match definition.create_with_existing_window(live.instance.window()) {
        Ok(instance) => instance,
        Err(e) => return eprintln!("oni-lab: {e}"),
    };
    let defaults = globals(&definition, &instance);
    // A value the hand moved survives the reload, unless the file changed that default itself.
    for (name, value) in globals(&definition, &live.instance) {
        if live.defaults.get(&name) != Some(&value) && defaults.get(&name) == live.defaults.get(&name) {
            let _ = instance.set_global_property(&name.0, &name.1, value);
        }
    }
    serve(&instance, raster);
    live.instance = instance;
    live.defaults = defaults;
    eprintln!("oni-lab: reloaded");
}

fn globals(definition: &ComponentDefinition, instance: &ComponentInstance) -> Globals {
    let mut out = Globals::new();
    for global in definition.globals() {
        for (property, _) in definition.global_properties(&global).into_iter().flatten() {
            if let Ok(value) = instance.get_global_property(&global, &property) {
                out.insert((global.clone(), property), value);
            }
        }
    }
    out
}

/// Every `.slint` file under the lab and the design system, with its modification time.
fn stamps() -> Vec<(PathBuf, SystemTime)> {
    let lab = lab_dir();
    let mut out = Vec::new();
    let mut stack = vec![lab.clone(), lab.join("../ui/ds")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "slint") {
                let time = entry.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                out.push((path, time));
            }
        }
    }
    out.sort();
    out
}
