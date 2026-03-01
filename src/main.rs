use anyhow::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rust_emu::joypad::JoypadButton;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

/// Write a stereo WAV file (IEEE float 32-bit)
/// Left channel = raw (pre-filter), Right channel = filtered (post-filter)
fn write_wav_file(path: &str, sample_rate: u32, samples: &[(f32, f32)]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 32;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * block_align as u32;
    let file_size = 36 + data_size;

    // RIFF header
    f.write_all(b"RIFF")?;
    f.write_all(&file_size.to_le_bytes())?;
    f.write_all(b"WAVE")?;

    // fmt chunk (IEEE float = format tag 3)
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;       // chunk size
    f.write_all(&3u16.to_le_bytes())?;        // format: IEEE float
    f.write_all(&num_channels.to_le_bytes())?;
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&byte_rate.to_le_bytes())?;
    f.write_all(&block_align.to_le_bytes())?;
    f.write_all(&bits_per_sample.to_le_bytes())?;

    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_size.to_le_bytes())?;
    for &(left, right) in samples {
        f.write_all(&left.to_le_bytes())?;
        f.write_all(&right.to_le_bytes())?;
    }

    println!("[WAV] Wrote {} samples ({:.1}s) to {}",
        samples.len(),
        samples.len() as f64 / sample_rate as f64,
        path);
    Ok(())
}

const _SAMPLE_RATE: u32 = 44100;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;

fn write_save_if_needed(nes: &rust_emu::Nes, save_path: &Option<PathBuf>) {
    if let (Some(path), Some(save_data)) = (save_path, nes.battery_ram_data()) {
        let _ = std::fs::write(path, save_data);
    }
}

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
    let mut rom_path: Option<PathBuf> = None;
    let mut tracing = false;
    let mut mmc1_logging = false;
    let mut apu_solo: u8 = 0;
    let mut wav_dump_path: Option<String> = None;
    let mut lpf_cutoff: f32 = 14000.0; // default LPF cutoff frequency (Hz)
    let mut hpf1_cutoff: f32 = 90.0;    // default HPF stage 1 (DC blocking)
    let mut hpf2_cutoff: f32 = 150.0;   // default HPF stage 2
    for arg in args.iter().skip(1) {
        if arg == "--trace" {
            tracing = true;
        } else if arg == "--mmc1-log" {
            mmc1_logging = true;
        } else if arg.starts_with("--apu-solo=") {
            // --apu-solo=1..5 (1=pulse1, 2=pulse2, 3=triangle, 4=noise, 5=dmc)
            if let Ok(ch) = arg.trim_start_matches("--apu-solo=").parse::<u8>() {
                apu_solo = ch;
            }
        } else if arg.starts_with("--wav-dump=") {
            wav_dump_path = Some(arg.trim_start_matches("--wav-dump=").to_string());
        } else if arg.starts_with("--lpf=") {
            // --lpf=<freq> : LPF cutoff in Hz (default 14000)
            if let Ok(f) = arg.trim_start_matches("--lpf=").parse::<f32>() {
                lpf_cutoff = f.clamp(1000.0, 22000.0);
            }
        } else if arg.starts_with("--hpf=") {
            // --hpf=<freq1>,<freq2> or --hpf=<freq> (sets both stages)
            let val = arg.trim_start_matches("--hpf=");
            if let Some((a, b)) = val.split_once(',') {
                if let Ok(f1) = a.parse::<f32>() {
                    hpf1_cutoff = f1.clamp(0.0, 1000.0);
                }
                if let Ok(f2) = b.parse::<f32>() {
                    hpf2_cutoff = f2.clamp(0.0, 1000.0);
                }
            } else if let Ok(f) = val.parse::<f32>() {
                let f = f.clamp(0.0, 1000.0);
                hpf1_cutoff = f;
                hpf2_cutoff = f;
            }
        } else if !arg.starts_with("--") && rom_path.is_none() {
            rom_path = Some(PathBuf::from(arg));
        }
    }
    println!("[Audio] LPF: {} Hz, HPF: {} / {} Hz", lpf_cutoff, hpf1_cutoff, hpf2_cutoff);

    let rom_data = if let Some(path) = rom_path.as_ref() {
        std::fs::read(path).map_err(Error::msg)?
    } else {
        // Dummy ROM for testing if no file provided
        let rom = vec![0; 0x8000];
        // Header
        let header = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, // 2x 16KB PRG ROM
            0x01, // 1x 8KB CHR ROM
            0x00, // Mapper 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut full_rom = Vec::new();
        full_rom.extend(header);
        full_rom.extend(rom);
        full_rom.extend(vec![0; 0x2000]); // CHR ROM
        full_rom
    };

    let save_path = rom_path.as_ref().map(|path| path.with_extension("sav"));

    let mut nes = rust_emu::Nes::new_with_rom(&rom_data);
    if mmc1_logging {
        nes.bus.set_mmc1_debug(true);
    }
    if apu_solo > 0 {
        let names = ["", "Pulse1", "Pulse2", "Triangle", "Noise", "DMC"];
        let name = names.get(apu_solo as usize).unwrap_or(&"?");
        println!("[APU] Solo channel: {} ({})", apu_solo, name);
        nes.bus.apu.solo_channel = apu_solo;
    }
    if let Some(path) = save_path.as_ref() {
        if let Ok(save_data) = std::fs::read(path) {
            nes.load_battery_ram(&save_data);
        }
    }
    nes.reset();

    // Audio Setup
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("No output device available");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0;

    let audio_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
    let audio_buffer_out = Arc::clone(&audio_buffer);
    let num_channels = config.channels();

    let stream = device
        .build_output_stream(
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
            None,
        )
        .unwrap();
    stream.play().unwrap();

    let mut audio_samples_needed = 0.0;
    let samples_per_cpu_cycle = sample_rate as f64 / 1_789_773.0; // CPU clock rate

    let mut last_frame_time = Instant::now();
    let frame_duration = Duration::from_nanos(16639267); // NES NTSC ~60.098 Hz

    // NES audio filter chain state (LP 14kHz ×2 → HP 90Hz → HP 150Hz)
    let mut lp1_prev_out: f32 = 0.0;
    let mut lp2_prev_out: f32 = 0.0;
    let mut hp1_prev_in: f32 = 0.0;
    let mut hp1_prev_out: f32 = 0.0;
    let mut hp2_prev_in: f32 = 0.0;
    let mut hp2_prev_out: f32 = 0.0;

    // WAV dump buffer: (raw, filtered) stereo pairs
    let mut wav_samples: Vec<(f32, f32)> = Vec::new();
    let wav_enabled = wav_dump_path.is_some();
    if wav_enabled {
        println!("[WAV] Capture enabled → {}", wav_dump_path.as_deref().unwrap());
    }

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
                    write_save_if_needed(&nes, &save_path);
                    control_flow.set_exit();
                    return;
                }
            }

            // Handle input events
            if input.update(&event) {
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    write_save_if_needed(&nes, &save_path);
                    if let Some(ref path) = wav_dump_path {
                        if let Err(e) = write_wav_file(path, sample_rate, &wav_samples) {
                            error!("Failed to write WAV: {}", e);
                        }
                    }
                    control_flow.set_exit();
                    return;
                }

                if let Some(size) = input.window_resized() {
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        error!("pixels.resize_surface() failed: {}", err);
                        write_save_if_needed(&nes, &save_path);
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
                    let current_output = nes.bus.apu.averaged_output();
                    apu_sum += current_output * step_cycles as f32;
                    apu_count += step_cycles as i32;

                    audio_samples_needed += step_cycles as f64 * samples_per_cpu_cycle;
                    if audio_samples_needed >= 1.0 {
                        let mut buffer = audio_buffer.lock().unwrap();
                        if buffer.len() < 4096 {
                            let num_samples = audio_samples_needed as i32;
                            let raw = if apu_count > 0 {
                                apu_sum / apu_count as f32
                            } else {
                                current_output
                            };
                            for _ in 0..num_samples {
                                // NES audio filter chain: LP first, then HP
                                let fs = sample_rate as f32;

                                // Stage 1-2: Low-pass (2nd-order cascaded)
                                let k_lp = std::f32::consts::TAU * lpf_cutoff / fs;
                                let a_lp = k_lp / (1.0 + k_lp);
                                let lp1 = a_lp * raw + (1.0 - a_lp) * lp1_prev_out;
                                lp1_prev_out = lp1;
                                let lp2 = a_lp * lp1 + (1.0 - a_lp) * lp2_prev_out;
                                lp2_prev_out = lp2;

                                // Stage 3: High-pass (DC blocking)
                                let k1 = 1.0 / (1.0 + std::f32::consts::TAU * hpf1_cutoff / fs);
                                let hp1 = k1 * (hp1_prev_out + lp2 - hp1_prev_in);
                                hp1_prev_in = lp2;
                                hp1_prev_out = hp1;

                                // Stage 4: High-pass
                                let k2 = 1.0 / (1.0 + std::f32::consts::TAU * hpf2_cutoff / fs);
                                let hp2 = k2 * (hp2_prev_out + hp1 - hp2_prev_in);
                                hp2_prev_in = hp1;
                                hp2_prev_out = hp2;

                                // Capture for WAV dump
                                if wav_enabled {
                                    wav_samples.push((raw, hp2));
                                }

                                buffer.push_back(hp2);
                            }
                            // Reset accumulator once per batch (NOT per sample)
                            apu_sum = 0.0;
                            apu_count = 0;
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
