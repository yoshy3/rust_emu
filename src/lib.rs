#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn log(s: &str) {
    println!("{}", s);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod joypad;
pub mod opcodes;
pub mod ppu;

use bus::Bus;
use cpu::Cpu;
use ppu::Ppu;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub enum JoypadButtonWasm {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Nes {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub cpu: Cpu,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub bus: Bus,

    // Audio state
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub audio_samples: Vec<f32>,
    pub audio_sample_rate: f32,
    audio_samples_needed: f64,
    apu_sum: f32,
    apu_count: u32,
    // NES hardware audio filter chain state (HP 90Hz → HP 440Hz → LP 14kHz ×2)
    hp1_prev_in: f32,
    hp1_prev_out: f32,
    hp2_prev_in: f32,
    hp2_prev_out: f32,
    lp1_prev_out: f32,
    lp2_prev_out: f32,
}

impl Nes {
    pub fn new_with_rom(rom_data: &[u8]) -> Self {
        let rom = crate::cartridge::Rom::new(&rom_data.to_vec()).unwrap();
        let mut ppu = Ppu::new(rom.screen_mirroring, rom.chr_rom);
        ppu.mapper = rom.mapper;
        let bus = Bus::new(
            ppu,
            rom.prg_rom,
            rom.mapper,
            rom.prg_ram_size,
            rom.has_battery,
        );
        let cpu = Cpu::new();
        Self {
            cpu,
            bus,
            audio_samples: Vec::with_capacity(4096),
            audio_sample_rate: 44100.0,
            audio_samples_needed: 0.0,
            apu_sum: 0.0,
            apu_count: 0,
            hp1_prev_in: 0.0,
            hp1_prev_out: 0.0,
            hp2_prev_in: 0.0,
            hp2_prev_out: 0.0,
            lp1_prev_out: 0.0,
            lp2_prev_out: 0.0,
        }
    }

    pub fn set_joypad_button(&mut self, button: crate::joypad::JoypadButton, status: bool) {
        self.bus.joypad1.set_button_status(button, status);
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        self.bus.load_battery_ram(data);
    }

    pub fn battery_ram_data(&self) -> Option<Vec<u8>> {
        self.bus.battery_ram_data().map(|ram| ram.to_vec())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Nes {
    pub fn new() -> Self {
        // Create a dummy ROM by default
        let dummy_rom = vec![0; 0x8000];
        let header = vec![
            0x4E, 0x45, 0x53, 0x1A, // NES<EOF>
            0x02, // 2x 16KB PRG ROM
            0x01, // 1x 8KB CHR ROM
            0x00, // Mapper 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut full_rom = Vec::new();
        full_rom.extend(header);
        full_rom.extend(dummy_rom);
        full_rom.extend(vec![0; 0x2000]); // CHR ROM

        Self::new_with_rom(&full_rom)
    }

    pub fn load_rom(&mut self, rom_data: &[u8]) {
        if let Ok(rom) = crate::cartridge::Rom::new(&rom_data.to_vec()) {
            self.bus.prg_rom = rom.prg_rom;
            self.bus.ppu.chr_rom = rom.chr_rom;
            self.bus.ppu.mirroring = rom.screen_mirroring;
            self.bus.mapper = rom.mapper;
            self.bus.ppu.mapper = rom.mapper;
            self.bus.prg_ram = vec![0; rom.prg_ram_size.max(0x2000)];
            self.bus.has_battery = rom.has_battery;
            self.bus.reset_mapper_state();
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    pub fn tick(&mut self) -> usize {
        self.bus.ppu_cycles_advanced = 0;
        let cycles = self.cpu.step(&mut self.bus);

        // PPU catch-up: the PPU was partially advanced during bus.read()/write() calls.
        // Advance the remaining PPU cycles for this instruction.
        let total_ppu_cycles = (cycles as u16) * 3;
        let remaining = total_ppu_cycles.saturating_sub(self.bus.ppu_cycles_advanced);
        self.bus.ppu.tick(remaining);

        self.bus.tick_apu(cycles as u16);

        // NMI is checked via the persistent nmi_interrupt flag, which is set
        // by tick() during both catch-up (bus.read/write) and remaining cycles.
        if self.bus.ppu.nmi_interrupt {
            self.cpu.nmi(&mut self.bus);
            self.bus.ppu.nmi_interrupt = false;
        }

        // Handle IRQ from APU (frame counter IRQ / DMC IRQ)
        if self.bus.apu.is_irq_pending() {
            self.cpu.irq(&mut self.bus);
        }

        // Handle IRQ from MMC3 scanline counter
        if self.bus.ppu.mmc3_irq_pending {
            self.cpu.irq(&mut self.bus);
            self.bus.ppu.mmc3_irq_pending = false;
        }

        // Audio logic
        let step_cycles = cycles as u32;
        let current_output = self.bus.apu.averaged_output();
        self.apu_sum += current_output * step_cycles as f32;
        self.apu_count += step_cycles;

        self.audio_samples_needed +=
            step_cycles as f64 * (self.audio_sample_rate as f64 / 1789773.0);
        if self.audio_samples_needed >= 1.0 {
            let num_samples = self.audio_samples_needed as i32;
            for _ in 0..num_samples {
                let raw = if self.apu_count > 0 {
                    self.apu_sum / self.apu_count as f32
                } else {
                    current_output
                };

                // NES hardware audio filter chain
                // LPF first to smooth transients before HPF can amplify them
                let fs = self.audio_sample_rate;

                // Stage 1-2: Low-pass ~14 kHz (2nd-order cascaded for 12dB/oct)
                let k_lp = std::f32::consts::TAU * 14000.0 / fs;
                let a_lp = k_lp / (1.0 + k_lp);
                let lp1 = a_lp * raw + (1.0 - a_lp) * self.lp1_prev_out;
                self.lp1_prev_out = lp1;
                let lp2 = a_lp * lp1 + (1.0 - a_lp) * self.lp2_prev_out;
                self.lp2_prev_out = lp2;

                // Stage 3: High-pass ~90 Hz (DC blocking / coupling capacitor)
                let k1 = 1.0 / (1.0 + std::f32::consts::TAU * 90.0 / fs);
                let hp1 = k1 * (self.hp1_prev_out + lp2 - self.hp1_prev_in);
                self.hp1_prev_in = lp2;
                self.hp1_prev_out = hp1;

                // Stage 4: High-pass ~150 Hz
                let k2 = 1.0 / (1.0 + std::f32::consts::TAU * 150.0 / fs);
                let hp2 = k2 * (self.hp2_prev_out + hp1 - self.hp2_prev_in);
                self.hp2_prev_in = hp1;
                self.hp2_prev_out = hp2;

                // Cap buffer size to avoid memory leaks if JS doesn't consume
                if self.audio_samples.len() < 8192 {
                    self.audio_samples.push(hp2);
                }
            }

            if num_samples > 0 {
                self.apu_sum = 0.0;
                self.apu_count = 0;
            }
            self.audio_samples_needed -= num_samples as f64;
        }

        cycles as usize
    }

    pub fn get_audio_samples(&mut self) -> Vec<f32> {
        let mut samples = Vec::new();
        std::mem::swap(&mut samples, &mut self.audio_samples);
        samples
    }

    pub fn draw(&self, frame: &mut [u8]) {
        self.bus.ppu.draw(frame);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn set_joypad_button_wasm(&mut self, button: JoypadButtonWasm, status: bool) {
        let btn = match button {
            JoypadButtonWasm::A => crate::joypad::JoypadButton::BUTTON_A,
            JoypadButtonWasm::B => crate::joypad::JoypadButton::BUTTON_B,
            JoypadButtonWasm::Select => crate::joypad::JoypadButton::SELECT,
            JoypadButtonWasm::Start => crate::joypad::JoypadButton::START,
            JoypadButtonWasm::Up => crate::joypad::JoypadButton::UP,
            JoypadButtonWasm::Down => crate::joypad::JoypadButton::DOWN,
            JoypadButtonWasm::Left => crate::joypad::JoypadButton::LEFT,
            JoypadButtonWasm::Right => crate::joypad::JoypadButton::RIGHT,
        };
        self.set_joypad_button(btn, status);
    }
}
