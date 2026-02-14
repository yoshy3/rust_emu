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

pub mod bus;
pub mod cpu;
pub mod ppu;
pub mod cartridge;
pub mod opcodes;
pub mod joypad;
pub mod apu;

use cpu::Cpu;
use bus::Bus;
use ppu::Ppu;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub enum JoypadButtonWasm {
    A, B, Select, Start, Up, Down, Left, Right
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Nes {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub cpu: Cpu,
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(skip))]
    pub bus: Bus,
}

impl Nes {
    pub fn new_with_rom(rom_data: &[u8]) -> Self {
        let rom = crate::cartridge::Rom::new(&rom_data.to_vec()).unwrap();
        let ppu = Ppu::new(rom.screen_mirroring, rom.chr_rom);
        let bus = Bus::new(ppu, rom.prg_rom);
        let cpu = Cpu::new();
        Self { cpu, bus }
    }

    pub fn set_joypad_button(&mut self, button: crate::joypad::JoypadButton, status: bool) {
        self.bus.joypad1.set_button_status(button, status);
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
            0x00, 
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
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
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    pub fn tick(&mut self) -> usize {
        let cycles = self.cpu.step(&mut self.bus);
        
        let ppu_cycles = cycles * 3;
        let nmi = self.bus.ppu.tick(ppu_cycles as u16);
        self.bus.tick_apu(cycles as u16);
        
        if nmi {
            self.cpu.nmi(&mut self.bus);
        }

        cycles as usize
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
