//! Scholium desktop application entry point.

use scholium_shell::app::App;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let el = EventLoop::new().expect("event loop creation failed");
    el.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    el.run_app(&mut app).expect("event loop exited with error");
}
