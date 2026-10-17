//! Opens roll #8's frame in the proof-of-concept window.

use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = oni_ui_poc::frame::roll_dir().join(oni_ui_poc::frame::FRAME);
    let frame = oni_ui_poc::frame::Frame::open(&path).map_err(|e| format!("{}: {e:?}", path.display()))?;
    let poc = oni_ui_poc::Poc::new(frame, 2.0)?;
    poc.window.run()?;
    Ok(())
}
