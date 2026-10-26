//! The 2.5D lab: `crates/oni-ui/lab/lab.slint` in Slint's interpreter, reloaded on each save, its keycaps
//! drawn by oni-keycap; its compiler, keycaps and headless window also film the help cards' demos.

pub mod live;
pub mod raster;
pub mod shot;

use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint_interpreter::{CompilationResult, Compiler, ComponentDefinition, ComponentHandle, ComponentInstance};

/// The lab's sources, found from this crate so the binary runs from anywhere.
pub fn lab_dir() -> PathBuf {
    std::env::var_os("ONI_LAB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/lab"))
}

/// Compiles `lab.slint`, printing its diagnostics; the interpreter's build never waits.
pub fn compile(path: &Path) -> CompilationResult {
    let compiler = Compiler::default();
    ready(compiler.build_from_path(path))
}

/// Compiles `source` as though it were the file `path`, so its imports resolve beside it.
pub fn compile_source(source: String, path: &Path) -> CompilationResult {
    let compiler = Compiler::default();
    ready(compiler.build_from_source(source, path.to_path_buf()))
}

fn ready(future: impl std::future::Future<Output = CompilationResult>) -> CompilationResult {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(result) = std::future::Future::poll(future.as_mut(), &mut cx) else {
        unreachable!("the interpreter's compiler is synchronous")
    };
    result.print_diagnostics();
    result
}

/// The asked component, else the file's last export (the lab's window).
pub fn pick(result: &CompilationResult, name: Option<&str>) -> Option<ComponentDefinition> {
    match name {
        Some(name) => result.component(name),
        None => result.components().last(),
    }
}

/// Answers `Raster.keycap` for one instance at its window's scale; a file without `Raster` needs none.
pub fn serve(instance: &ComponentInstance, raster: &Rc<raster::Raster>) {
    let weak = instance.as_weak();
    let raster = raster.clone();
    let _ = instance.set_global_callback("Raster", "keycap", move |args| {
        let scale = weak.upgrade().map_or(1.0, |i| i.window().scale_factor());
        args.first().map_or(slint_interpreter::Value::Void, |look| raster.keycap(look, scale))
    });
}

