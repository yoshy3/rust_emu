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

use cpu::Cpu;
use bus::Bus;
use ppu::Ppu;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Nes {
    cpu: Cpu,
    bus: Bus,
}

impl Nes {
    pub fn new() -> Self {
        let ppu = Ppu::new();
        let bus = Bus::new(ppu);
        let cpu = Cpu::new();
        Self { cpu, bus }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    pub fn tick(&mut self) {
        self.cpu.step(&mut self.bus);
        self.bus.ppu.tick();
    }

    pub fn draw(&self, frame: &mut [u8]) {
        self.bus.ppu.draw(frame);
    }
}
