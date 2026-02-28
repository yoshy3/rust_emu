use anyhow::{Error, Result};
use log::error;
use std::time::{Instant, Duration};
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use rust_emu::joypad::JoypadButton;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

const _SAMPLE_RATE: u32 = 44100;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    
    let window = {
        let size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title(format!("Rust NES Emulator v{}", env!("CARGO_PKG_VERSION")))
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(WIDTH as f64, HEIGHT as f64))
            .build(&event_loop)
            .map_err(Error::msg)?
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).map_err(Error::msg)?
    };

    // Load ROM
    let args: Vec<String> = std::env::args().collect();
    let rom_data = if args.len() > 1 {
        std::fs::read(&args[1]).map_err(Error::msg)?
    } else {
        // Dummy ROM for testing if no file provided
        let rom = vec![0; 0x8000];
        // Header
        let header = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, // 2x 16KB PRG ROM
            0x01, // 1x 8KB CHR ROM
            0x00, // Mapper 0
            0x00, 
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        ];
        let mut full_rom = Vec::new();
        full_rom.extend(header);
        full_rom.extend(rom);
        full_rom.extend(vec![0; 0x2000]); // CHR ROM
        full_rom
    };

    let mut nes = rust_emu::Nes::new_with_rom(&rom_data);
    nes.reset();

    // Audio Setup
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device available");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0;

    let audio_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let audio_buffer_out = Arc::clone(&audio_buffer);
    let num_channels = config.channels();

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut buffer = audio_buffer_out.lock().unwrap();
            for frame in data.chunks_mut(num_channels as usize) {
                if let Some(sample) = buffer.pop_front() {
                    for channel in frame {
                        *channel = sample;
                    }
                } else {
                    for channel in frame {
                        *channel = 0.0;
                    }
                }
            }
        },
        |err| error!("Audio stream error: {}", err),
        None
    ).unwrap();
    stream.play().unwrap();

    let mut audio_samples_needed = 0.0;
    let samples_per_cpu_cycle = sample_rate as f64 / 1_789_773.0; // CPU clock rate

    // Check for trace flag
    let tracing = args.contains(&"--trace".to_string());

    let mut last_frame_time = Instant::now();
    let frame_duration = Duration::from_nanos(16639267); // NES NTSC ~60.098 Hz

    // Simple High-pass filter (DC blocker) state
    let mut prev_apu_sample = 0.0;
    let mut filtered_sample = 0.0;

    if tracing {
        // Run in headless mode for tracing
        nes.reset();
        
        loop {
            println!("{}", nes.cpu.trace(&mut nes.bus));
            nes.tick();
            
            // Optional: Break on infinite loop or specific PC
            // if nes.cpu.pc == 0xC66E { break; } 
        }
    } else {
        event_loop.run(move |event, _, control_flow| {
            control_flow.set_poll();

            // Handle redraw requests
            if let Event::RedrawRequested(_) = event {
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
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    control_flow.set_exit();
                    return;
                }
    
                if let Some(size) = input.window_resized() {
                     if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        error!("pixels.resize_surface() failed: {}", err);
                        control_flow.set_exit();
                        return;
                    }
                }
    
                nes.set_joypad_button(JoypadButton::BUTTON_A, input.key_held(VirtualKeyCode::Z));
                nes.set_joypad_button(JoypadButton::BUTTON_B, input.key_held(VirtualKeyCode::X));
                nes.set_joypad_button(JoypadButton::SELECT, input.key_held(VirtualKeyCode::RShift));
                nes.set_joypad_button(JoypadButton::START, input.key_held(VirtualKeyCode::Return));
                nes.set_joypad_button(JoypadButton::UP, input.key_held(VirtualKeyCode::Up));
                nes.set_joypad_button(JoypadButton::DOWN, input.key_held(VirtualKeyCode::Down));
                nes.set_joypad_button(JoypadButton::LEFT, input.key_held(VirtualKeyCode::Left));
                nes.set_joypad_button(JoypadButton::RIGHT, input.key_held(VirtualKeyCode::Right));
            }

            // Step emulator for one frame if it's time
            if last_frame_time.elapsed() >= frame_duration {
                let mut cycles = 0;
                let mut apu_sum = 0.0;
                let mut apu_count = 0;

                while cycles < 29781 {
                    let step_cycles = nes.tick();
                    cycles += step_cycles;

                    // Accumulate APU output for averaging (Oversampling)
                    let current_output = nes.bus.apu.output();
                    apu_sum += current_output * step_cycles as f32;
                    apu_count += step_cycles as i32;

                    audio_samples_needed += step_cycles as f64 * samples_per_cpu_cycle;
                    if audio_samples_needed >= 1.0 {
                        let mut buffer = audio_buffer.lock().unwrap();
                        if buffer.len() < 4096 {
                            for _ in 0..audio_samples_needed as i32 {
                                let avg_sample = if apu_count > 0 { apu_sum / apu_count as f32 } else { current_output };
                                
                                // Reset accumulator
                                apu_sum = 0.0;
                                apu_count = 0;

                                // DC Blocker (High-pass filter at ~20Hz)
                                filtered_sample = avg_sample - prev_apu_sample + 0.999 * filtered_sample;
                                prev_apu_sample = avg_sample;
                                
                                buffer.push_back(filtered_sample);
                            }
                        }
                        audio_samples_needed -= audio_samples_needed as i32 as f64;
                    }
                }
                last_frame_time += frame_duration;
                // Avoid "death spiral" if the computer is too slow
                if last_frame_time.elapsed() > frame_duration * 2 {
                    last_frame_time = Instant::now();
                }
                window.request_redraw();
            }
        });
 // run diverges
    }
}

