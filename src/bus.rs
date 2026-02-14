use crate::ppu::Ppu;

pub struct Bus {
    pub cpu_vram: [u8; 2048],
    pub ppu: Ppu,
}

impl Bus {
    pub fn new(ppu: Ppu) -> Self {
        Self {
            cpu_vram: [0; 2048],
            ppu,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF],
            _ => 0, // Placeholder
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF] = data,
            _ => {}, // Placeholder
        }
    }
}
