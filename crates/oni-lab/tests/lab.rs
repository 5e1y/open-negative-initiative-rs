//! The 2.5D lab compiles, every page with it: the design system's showcase now that the app holds no board.

use slint_interpreter::{Compiler, DiagnosticLevel};
use std::path::Path;

fn errors(result: &slint_interpreter::CompilationResult) -> Vec<String> {
    result.diagnostics().filter(|d| d.level() == DiagnosticLevel::Error).map(|d| d.to_string()).collect()
}

#[test]
fn the_lab_and_its_pages_compile() {
    let lab = Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/lab/lab.slint");
    let built = spin_on(Compiler::default().build_from_path(&lab));
    assert!(errors(&built).is_empty(), "{:?}", errors(&built));
    // The window lays every page in a tab, so they compiled with it.
    assert!(built.component("Lab").is_some(), "the lab's window is exported");
    // Twin: a page naming a component the design system lacks is refused.
    let broken = spin_on(Compiler::default().build_from_source(
        "import { Keycap } from \"depth.slint\"; export component P { NoSuchKey { } }".into(),
        lab.clone(),
    ));
    assert!(!errors(&broken).is_empty());
}

/// The compiler's futures never wait on anything: polled once, they are ready.
fn spin_on<F: std::future::Future>(future: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let std::task::Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
    }
}
