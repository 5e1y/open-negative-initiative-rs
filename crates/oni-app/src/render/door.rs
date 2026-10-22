//! The app's one GPU: the device every graph records on, and the door the window's surface
//! reconfigure closes, since wgpu refuses one that another thread submits work during.

use super::*;
use std::sync::OnceLock;

#[derive(Default)]
struct Gate {
    held: bool,
    inside: usize,
}

/// Threads submitting work pass through it; the interface thread holds it while it reconfigures.
#[derive(Default)]
pub(crate) struct Door {
    gate: Mutex<Gate>,
    turn: Condvar,
}

impl Door {
    /// Waits while the door is held, then counts the caller inside.
    pub(crate) fn enter(&self) -> Result<(), String> {
        let mut g = self.gate.lock().map_err(|_| "the render gate failed earlier")?;
        while g.held {
            g = self.turn.wait(g).map_err(|_| "the render gate failed earlier")?;
        }
        g.inside += 1;
        Ok(())
    }

    pub(crate) fn leave(&self) {
        if let Ok(mut g) = self.gate.lock() {
            g.inside -= 1;
        }
        self.turn.notify_all();
    }

    /// Closes the door and waits for the threads inside to leave.
    pub(crate) fn hold(&self) {
        let Ok(mut g) = self.gate.lock() else { return };
        g.held = true;
        while g.inside > 0 {
            match self.turn.wait(g) {
                Ok(next) => g = next,
                Err(_) => return,
            }
        }
    }

    pub(crate) fn release(&self) {
        if let Ok(mut g) = self.gate.lock() {
            g.held = false;
        }
        self.turn.notify_all();
    }
}

/// The bytes a reader's graph keeps idle: its reads are not on the hand's path.
const READER_IDLE: u64 = 64 << 20;

/// The window's engine's device and door, once a window is made.
static APP: OnceLock<(wgpu::Device, wgpu::Queue, Arc<Door>)> = OnceLock::new();

/// Makes `device` and `door` the ones every other graph of the app records on and passes through.
pub(crate) fn adopt(device: (wgpu::Device, wgpu::Queue), door: Arc<Door>) {
    let _ = APP.set((device.0, device.1, door));
}

/// A graph of its own on the app's device, each submission through the window's door, so a reader
/// keeps its textures apart from the canvas's without a device of its own; on a device of its own
/// where no window made one (tools, checks).
pub fn graph() -> Option<Graph> {
    let Some((device, queue, door)) = APP.get() else { return Graph::new().ok() };
    let mut graph = Graph::from_device(device.clone(), queue.clone(), "reader".into(), oni_graph::TEXTURE_SIDE);
    graph.through(door.clone());
    graph.idle_budget(READER_IDLE);
    Some(graph)
}

impl oni_graph::Door for Door {
    fn enter(&self) -> bool {
        Door::enter(self).is_ok()
    }

    fn leave(&self) {
        Door::leave(self);
    }
}
