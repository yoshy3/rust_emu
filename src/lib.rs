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
use joypad::JoypadButton;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Nes {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Nes {
    pub fn new(rom_data: &[u8]) -> Self {
        let rom = crate::cartridge::Rom::new(&rom_data.to_vec()).unwrap();
        let ppu = Ppu::new(rom.screen_mirroring, rom.chr_rom);
        let bus = Bus::new(ppu, rom.prg_rom);
        let cpu = Cpu::new();
        Self { cpu, bus }
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

    pub fn set_joypad_button(&mut self, button: JoypadButton, status: bool) {
        self.bus.joypad1.set_button_status(button, status);
    }
}
