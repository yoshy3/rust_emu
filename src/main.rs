use anyhow::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rust_emu::joypad::JoypadButton;
use std::collections::{HashMap, HashSet, VecDeque};
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
/// Both channels carry the emulator output sample for easy comparison.
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
const GAMEPAD_AXIS_THRESHOLD: f32 = 0.5;
const XBOX_MAC_BUTTON_A_CODE: u32 = (9 << 16) | 1;
const XBOX_MAC_BUTTON_X_CODE: u32 = (9 << 16) | 4;
const XBOX_MAC_BUTTON_BACK_CODE: u32 = (9 << 16) | 11;
const XBOX_MAC_BUTTON_START_CODE: u32 = (9 << 16) | 12;
const XBOX_MAC_BUTTON_MODE_CODE: u32 = (9 << 16) | 13;
const XBOX_MAC_AXIS_LEFT_X_CODE: u32 = (1 << 16) | 48;
const XBOX_MAC_AXIS_LEFT_Y_CODE: u32 = (1 << 16) | 49;
const XBOX_MAC_AXIS_DPAD_X_CODE: u32 = (1 << 16) | 57;
const XBOX_MAC_AXIS_DPAD_Y_CODE: u32 = (1 << 16) | 58;

#[derive(Clone, Copy)]
enum GamepadProfile {
    Default,
    XboxWirelessMac,
}

impl GamepadProfile {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::XboxWirelessMac => "xbox-wireless-mac",
        }
    }
}

struct GamepadInput {
    gilrs: Gilrs,
    active_gamepad: Option<GamepadId>,
    profile: GamepadProfile,
    pressed_codes: HashSet<u32>,
    axis_values: HashMap<u32, f32>,
}

impl GamepadInput {
    fn new() -> Result<Self> {
        let gilrs = Gilrs::new().map_err(|err| Error::msg(err.to_string()))?;
        let active_gamepad = gilrs
            .gamepads()
            .find(|(_, gamepad)| gamepad.is_connected())
            .map(|(id, _)| id);

        if let Some(id) = active_gamepad {
            let gamepad = gilrs.gamepad(id);
            println!("[Input] Gamepad connected: {}", gamepad.name());
        }

        let profile = active_gamepad
            .map(|id| Self::detect_profile(gilrs.gamepad(id).name()))
            .unwrap_or(GamepadProfile::Default);

        Ok(Self {
            gilrs,
            active_gamepad,
            profile,
            pressed_codes: HashSet::new(),
            axis_values: HashMap::new(),
        })
    }

    fn detect_profile(gamepad_name: &str) -> GamepadProfile {
        if cfg!(target_os = "macos") && gamepad_name == "Xbox Wireless Controller" {
            GamepadProfile::XboxWirelessMac
        } else {
            GamepadProfile::Default
        }
    }

    fn set_active_gamepad(&mut self, active_gamepad: Option<GamepadId>) {
        self.active_gamepad = active_gamepad;
        self.pressed_codes.clear();
        self.axis_values.clear();
        self.profile = active_gamepad
            .map(|id| Self::detect_profile(self.gilrs.gamepad(id).name()))
            .unwrap_or(GamepadProfile::Default);

        if let Some(id) = self.active_gamepad {
            let gamepad = self.gilrs.gamepad(id);
            println!(
                "[Input] Active gamepad: {} (profile: {})",
                gamepad.name(),
                self.profile.name()
            );
        }
    }

    fn is_code_pressed(&self, code: u32) -> bool {
        self.pressed_codes.contains(&code)
    }

    fn axis_value(&self, code: u32) -> f32 {
        self.axis_values.get(&code).copied().unwrap_or(0.0)
    }

    fn update(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    let gamepad = self.gilrs.gamepad(event.id);
                    println!("[Input] Gamepad connected: {}", gamepad.name());
                    if self.active_gamepad.is_none() {
                        self.set_active_gamepad(Some(event.id));
                    }
                }
                EventType::Disconnected => {
                    let disconnected_active = self.active_gamepad == Some(event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    println!("[Input] Gamepad disconnected: {}", gamepad.name());
                    if disconnected_active {
                        let next_gamepad = self
                            .gilrs
                            .gamepads()
                            .find(|(_, gamepad)| gamepad.is_connected())
                            .map(|(id, _)| id);
                        self.set_active_gamepad(next_gamepad);
                    }
                }
                EventType::ButtonPressed(button, code) => {
                    let gamepad = self.gilrs.gamepad(event.id);
                    self.pressed_codes.insert(code.into_u32());
                    println!(
                        "[Input] Button pressed on {}: {:?} (code: {:?}, raw: {})",
                        gamepad.name(),
                        button,
                        code,
                        code.into_u32()
                    );
                }
                EventType::ButtonChanged(button, value, code) => {
                    // Some controllers surface digital inputs as value changes.
                    if value >= 0.5 {
                        self.pressed_codes.insert(code.into_u32());
                    } else {
                        self.pressed_codes.remove(&code.into_u32());
                    }
                    let gamepad = self.gilrs.gamepad(event.id);
                    println!(
                        "[Input] Button changed on {}: {:?} = {:.3} (code: {:?}, raw: {})",
                        gamepad.name(),
                        button,
                        value,
                        code,
                        code.into_u32()
                    );
                }
                EventType::ButtonReleased(button, code) => {
                    let gamepad = self.gilrs.gamepad(event.id);
                    self.pressed_codes.remove(&code.into_u32());
                    println!(
                        "[Input] Button released on {}: {:?} (code: {:?}, raw: {})",
                        gamepad.name(),
                        button,
                        code,
                        code.into_u32()
                    );
                }
                EventType::AxisChanged(axis, value, code) => {
                    self.axis_values.insert(code.into_u32(), value);
                    let gamepad = self.gilrs.gamepad(event.id);
                    println!(
                        "[Input] Axis changed on {}: {:?} = {:.3} (code: {:?}, raw: {})",
                        gamepad.name(),
                        axis,
                        value,
                        code,
                        code.into_u32()
                    );
                }
                _ => {}
            }
        }
    }

    fn button_held(&self, button: JoypadButton) -> bool {
        let Some(id) = self.active_gamepad else {
            return false;
        };

        let gamepad = self.gilrs.gamepad(id);
        match self.profile {
            GamepadProfile::XboxWirelessMac => self.button_held_xbox_wireless_mac(button),
            GamepadProfile::Default => {
                let pressed = |button| gamepad.is_pressed(button);
                let axis = |axis| gamepad.value(axis);

                if button == JoypadButton::BUTTON_A {
                    pressed(Button::South)
                } else if button == JoypadButton::BUTTON_B {
                    pressed(Button::West)
                } else if button == JoypadButton::SELECT {
                    pressed(Button::Select)
                } else if button == JoypadButton::START {
                    pressed(Button::Start)
                } else if button == JoypadButton::UP {
                    pressed(Button::DPadUp) || axis(Axis::LeftStickY) <= -GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::DOWN {
                    pressed(Button::DPadDown) || axis(Axis::LeftStickY) >= GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::LEFT {
                    pressed(Button::DPadLeft) || axis(Axis::LeftStickX) <= -GAMEPAD_AXIS_THRESHOLD
                } else if button == JoypadButton::RIGHT {
                    pressed(Button::DPadRight) || axis(Axis::LeftStickX) >= GAMEPAD_AXIS_THRESHOLD
                } else {
                    false
                }
            }
        }
    }

    fn button_held_xbox_wireless_mac(&self, button: JoypadButton) -> bool {
        let dpad_x = self.axis_value(XBOX_MAC_AXIS_DPAD_X_CODE);
        let dpad_y = self.axis_value(XBOX_MAC_AXIS_DPAD_Y_CODE);
        let left_x = self.axis_value(XBOX_MAC_AXIS_LEFT_X_CODE);
        let left_y = self.axis_value(XBOX_MAC_AXIS_LEFT_Y_CODE);

        if button == JoypadButton::BUTTON_A {
            self.is_code_pressed(XBOX_MAC_BUTTON_A_CODE)
        } else if button == JoypadButton::BUTTON_B {
            self.is_code_pressed(XBOX_MAC_BUTTON_X_CODE)
        } else if button == JoypadButton::SELECT {
            self.is_code_pressed(XBOX_MAC_BUTTON_BACK_CODE)
                || self.is_code_pressed(XBOX_MAC_BUTTON_MODE_CODE)
        } else if button == JoypadButton::START {
            self.is_code_pressed(XBOX_MAC_BUTTON_START_CODE)
        } else if button == JoypadButton::UP {
            dpad_y >= 0.5 || left_y >= GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::DOWN {
            dpad_y <= -0.5 || left_y <= -GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::LEFT {
            dpad_x <= -0.5 || left_x <= -GAMEPAD_AXIS_THRESHOLD
        } else if button == JoypadButton::RIGHT {
            dpad_x >= 0.5 || left_x >= GAMEPAD_AXIS_THRESHOLD
        } else {
            false
        }
    }
}

fn write_save_if_needed(nes: &rust_emu::Nes, save_path: &Option<PathBuf>) {
    if let (Some(path), Some(save_data)) = (save_path, nes.battery_ram_data()) {
        let _ = std::fs::write(path, save_data);
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let mut gamepad_input = GamepadInput::new().ok();

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
    let mut hpf2_cutoff: f32 = 440.0;   // default HPF stage 2
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
    nes.audio_sample_rate = sample_rate as f32;

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

    let mut last_frame_time = Instant::now();
    let frame_duration = Duration::from_nanos(16639267); // NES NTSC ~60.098 Hz

    // WAV dump buffer: (raw, filtered) stereo pairs
    let mut wav_samples: Vec<(f32, f32)> = Vec::new();
    let wav_enabled = wav_dump_path.is_some();
    if wav_enabled {
        println!("[WAV] Capture enabled ↁE{}", wav_dump_path.as_deref().unwrap());
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
                if let Some(gamepad_input) = gamepad_input.as_mut() {
                    gamepad_input.update();
                }

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

                let gamepad_held = |button| {
                    gamepad_input
                        .as_ref()
                        .map(|gamepad_input| gamepad_input.button_held(button))
                        .unwrap_or(false)
                };

                nes.set_joypad_button(
                    JoypadButton::BUTTON_A,
                    input.key_held(VirtualKeyCode::Z) || gamepad_held(JoypadButton::BUTTON_A),
                );
                nes.set_joypad_button(
                    JoypadButton::BUTTON_B,
                    input.key_held(VirtualKeyCode::X) || gamepad_held(JoypadButton::BUTTON_B),
                );
                nes.set_joypad_button(
                    JoypadButton::SELECT,
                    input.key_held(VirtualKeyCode::RShift) || gamepad_held(JoypadButton::SELECT),
                );
                nes.set_joypad_button(
                    JoypadButton::START,
                    input.key_held(VirtualKeyCode::Return) || gamepad_held(JoypadButton::START),
                );
                nes.set_joypad_button(
                    JoypadButton::UP,
                    input.key_held(VirtualKeyCode::Up) || gamepad_held(JoypadButton::UP),
                );
                nes.set_joypad_button(
                    JoypadButton::DOWN,
                    input.key_held(VirtualKeyCode::Down) || gamepad_held(JoypadButton::DOWN),
                );
                nes.set_joypad_button(
                    JoypadButton::LEFT,
                    input.key_held(VirtualKeyCode::Left) || gamepad_held(JoypadButton::LEFT),
                );
                nes.set_joypad_button(
                    JoypadButton::RIGHT,
                    input.key_held(VirtualKeyCode::Right) || gamepad_held(JoypadButton::RIGHT),
                );
            }

            // Step emulator for one frame if it's time
            if last_frame_time.elapsed() >= frame_duration {
                let mut cycles = 0;
                while cycles < 29781 {
                    let step_cycles = nes.tick();
                    cycles += step_cycles;
                    let samples = nes.get_audio_samples();
                    if !samples.is_empty() {
                        let mut buffer = audio_buffer.lock().unwrap();
                        for sample in samples {
                            if buffer.len() < 4096 {
                                buffer.push_back(sample);
                            }
                            if wav_enabled {
                                wav_samples.push((sample, sample));
                            }
                        }
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
