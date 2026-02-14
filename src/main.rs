use anyhow::{Error, Result};
use log::error;
use pixels::{Error as PixelsError, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Rust NES Emulator")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .map_err(Error::msg)?
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).map_err(Error::msg)?
    };

    // Placeholder for NES instance
    let mut nes = rust_emu::Nes::new();
    nes.reset();

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            // Draw to frame buffer
            let frame = pixels.frame_mut();
            nes.draw(frame);

            if let Err(err) = pixels.render() {
                error!("pixels.render() failed: {}", err);
                control_flow.set_exit();
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                control_flow.set_exit();
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                 if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    error!("pixels.resize_surface() failed: {}", err);
                    control_flow.set_exit();
                    return;
                }
            }

            // Update internal state and request a redraw
            nes.tick();
            window.request_redraw();
        }
    }); // run diverges
}

