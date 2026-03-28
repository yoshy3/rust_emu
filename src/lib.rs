#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Serialize, Deserialize)]
pub struct NesSnapshot {
    cpu: Cpu,
    bus: Bus,
    audio_sample_rate: f32,
    audio_samples_needed: f64,
    hp1_prev_in: f32,
    hp1_prev_out: f32,
    hp2_prev_in: f32,
    hp2_prev_out: f32,
    lp1_prev_out: f32,
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
    // NES hardware audio filter chain state (HP 90Hz -> HP 440Hz -> LP 14kHz)
    hp1_prev_in: f32,
    hp1_prev_out: f32,
    hp2_prev_in: f32,
    hp2_prev_out: f32,
    lp1_prev_out: f32,
}

impl Nes {
    fn filter_audio_sample(&mut self, raw: f32) -> f32 {
        // NES hardware audio filter chain: HP 90Hz -> HP 440Hz -> LP 14kHz
        let fs = self.audio_sample_rate;

        let k1 = 1.0 / (1.0 + std::f32::consts::TAU * 90.0 / fs);
        let hp1 = k1 * (self.hp1_prev_out + raw - self.hp1_prev_in);
        self.hp1_prev_in = raw;
        self.hp1_prev_out = hp1;

        let k2 = 1.0 / (1.0 + std::f32::consts::TAU * 440.0 / fs);
        let hp2 = k2 * (self.hp2_prev_out + hp1 - self.hp2_prev_in);
        self.hp2_prev_in = hp1;
        self.hp2_prev_out = hp2;

        let k_lp = std::f32::consts::TAU * 14000.0 / fs;
        let a_lp = k_lp / (1.0 + k_lp);
        let lp = a_lp * hp2 + (1.0 - a_lp) * self.lp1_prev_out;
        self.lp1_prev_out = lp;
        lp
    }

    fn clock_apu_audio(&mut self, cycles: u16) {
        let samples_per_cpu_cycle = self.audio_sample_rate as f64 / 1_789_773.0;
        for _ in 0..cycles {
            self.bus.tick_apu(1);
            let raw = self.bus.apu.averaged_output();
            self.audio_samples_needed += samples_per_cpu_cycle;
            while self.audio_samples_needed >= 1.0 {
                let filtered = self.filter_audio_sample(raw);
                if self.audio_samples.len() < 8192 {
                    self.audio_samples.push(filtered);
                }
                self.audio_samples_needed -= 1.0;
            }
        }
    }

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
            hp1_prev_in: 0.0,
            hp1_prev_out: 0.0,
            hp2_prev_in: 0.0,
            hp2_prev_out: 0.0,
            lp1_prev_out: 0.0,
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

    pub fn save_state(&self) -> NesSnapshot {
        NesSnapshot {
            cpu: self.cpu.clone(),
            bus: self.bus.clone(),
            audio_sample_rate: self.audio_sample_rate,
            audio_samples_needed: self.audio_samples_needed,
            hp1_prev_in: self.hp1_prev_in,
            hp1_prev_out: self.hp1_prev_out,
            hp2_prev_in: self.hp2_prev_in,
            hp2_prev_out: self.hp2_prev_out,
            lp1_prev_out: self.lp1_prev_out,
        }
    }

    pub fn load_state(&mut self, snapshot: &NesSnapshot) {
        self.cpu = snapshot.cpu.clone();
        self.bus = snapshot.bus.clone();
        self.audio_sample_rate = snapshot.audio_sample_rate;
        self.audio_samples_needed = snapshot.audio_samples_needed;
        self.hp1_prev_in = snapshot.hp1_prev_in;
        self.hp1_prev_out = snapshot.hp1_prev_out;
        self.hp2_prev_in = snapshot.hp2_prev_in;
        self.hp2_prev_out = snapshot.hp2_prev_out;
        self.lp1_prev_out = snapshot.lp1_prev_out;
        self.audio_samples.clear();
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

        self.clock_apu_audio(cycles as u16);

        // VRC4 IRQ: clock once per CPU cycle
        if self.bus.mapper == 21 || self.bus.mapper == 23 || self.bus.mapper == 25 {
            for _ in 0..cycles {
                self.bus.clock_vrc4_irq();
            }
        }

        let mut total_cycles = cycles as usize;

        // NMI is checked via the persistent nmi_interrupt flag, which is set
        // by tick() during both catch-up (bus.read/write) and remaining cycles.
        if self.bus.ppu.nmi_interrupt {
            // NMI takes 7 CPU cycles on the 6502. Account for PPU and APU advancement.
            self.bus.ppu_cycles_advanced = 0;
            self.cpu.nmi(&mut self.bus);
            self.bus.ppu.nmi_interrupt = false;
            let nmi_ppu_remaining = (7u16 * 3).saturating_sub(self.bus.ppu_cycles_advanced);
            self.bus.ppu.tick(nmi_ppu_remaining);
            self.clock_apu_audio(7);
            total_cycles += 7;
        }

        // Handle IRQ from APU (frame counter IRQ / DMC IRQ)
        // Only dispatch when the CPU I flag is clear (IRQ not masked).
        // If I=1, the IRQ stays pending and will fire once I is cleared.
        if self.bus.apu.is_irq_pending() && (self.cpu.st & 0x04) == 0 {
            self.bus.ppu_cycles_advanced = 0;
            self.cpu.irq(&mut self.bus);
            let irq_ppu_remaining = (7u16 * 3).saturating_sub(self.bus.ppu_cycles_advanced);
            self.bus.ppu.tick(irq_ppu_remaining);
            self.clock_apu_audio(7);
            total_cycles += 7;
        }

        // Handle IRQ from MMC3 scanline counter
        // Only dispatch when CPU I flag is clear.
        if self.bus.ppu.mmc3_irq_pending && (self.cpu.st & 0x04) == 0 {
            self.bus.ppu_cycles_advanced = 0;
            self.cpu.irq(&mut self.bus);
            self.bus.ppu.mmc3_irq_pending = false;
            let irq_ppu_remaining = (7u16 * 3).saturating_sub(self.bus.ppu_cycles_advanced);
            self.bus.ppu.tick(irq_ppu_remaining);
            self.clock_apu_audio(7);
            total_cycles += 7;
        }

        // Handle IRQ from VRC4 counter
        if self.bus.vrc4_irq_pending && (self.cpu.st & 0x04) == 0 {
            self.bus.ppu_cycles_advanced = 0;
            self.cpu.irq(&mut self.bus);
            self.bus.vrc4_irq_pending = false;
            let irq_ppu_remaining = (7u16 * 3).saturating_sub(self.bus.ppu_cycles_advanced);
            self.bus.ppu.tick(irq_ppu_remaining);
            self.clock_apu_audio(7);
            total_cycles += 7;
        }

        total_cycles
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
