use crate::ppu::Ppu;
use crate::joypad::Joypad;
use crate::apu::Apu;

pub struct Bus {
    pub cpu_vram: [u8; 2048],
    pub prg_rom: Vec<u8>,
    pub ppu: Ppu,
    pub cycles: usize, // Accumulated cycles (e.g. from DMA)
    pub joypad1: Joypad,
    pub apu: Apu,
    pub mapper: u8,
    pub prg_bank: usize,
}

impl Bus {
    pub fn new(ppu: Ppu, rom: Vec<u8>, mapper: u8) -> Self {
        Self {
            cpu_vram: [0; 2048],
            prg_rom: rom,
            ppu,
            cycles: 0,
            joypad1: Joypad::new(),
            apu: Apu::new(),
            mapper,
            prg_bank: 0,
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF],
            0x2000..=0x3FFF => self.ppu.read_register(addr & 0x2007),
            0x4014 => 0, // DMA register
            0x4015 => self.apu.read_status(),
            0x4016 => self.joypad1.read(),
            0x4017 => 0, // Joypad 2 (not implemented)
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => 0,
        }
    }

    /// Non-side-effecting read for trace/debug. Does not clear VBlank,
    /// advance joypad state, or affect APU status.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF],
            0x2000..=0x3FFF => self.ppu.peek_register(addr & 0x2007),
            0x4014 => 0,
            0x4015 => 0, // Don't clear APU status flags
            0x4016 => 0, // Don't advance joypad shift register
            0x4017 => 0,
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF] = data,
            0x2000..=0x3FFF => self.ppu.write_register(addr & 0x2007, data),
            0x4014 => {
                 self.dma_transfer(data);
            }
            0x4016 => {
                self.joypad1.write(data);
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                self.apu.write_register(addr, data);
            }
            0x8000..=0xFFFF => {
                 if self.mapper == 2 {
                     self.prg_bank = data as usize;
                 } else if self.mapper == 3 {
                     self.ppu.chr_bank = data as usize;
                 }
            }
            _ => {}, 
        }
    }

    fn dma_transfer(&mut self, data: u8) {
        let hi = (data as u16) << 8;
        for i in 0..256 {
            let addr = hi + i;
            let byte = self.read(addr);
            self.ppu.oam_data = byte;
            // Writes to OAMDATA ($2004) automatically increment OAMADDR?
            // Actually DMA writes directly to OAM memory, bypassing OAMADDR increment usually?
            // "The CPU is suspended... 513 or 514 cycles... writes to $2004".
            // So it effectively writes to $2004 256 times.
            self.ppu.write_register(0x2004, byte);
        }
        self.cycles += 514; // Approx for read/write alignment (odd/even cycles matters but 514 is good avg)
    }

    pub fn poll_dma_cycles(&mut self) -> usize {
        let cycles = self.cycles;
        self.cycles = 0;
        cycles
    }

    pub fn tick_apu(&mut self, cycles: u16) {
        for _ in 0..cycles {
            if self.apu.dmc_needs_fetch() {
                let addr = self.apu.dmc_fetch_address();
                let data = self.read(addr);
                self.apu.dmc_provide_sample(data);
                self.cycles += 4;
            }
            self.apu.tick(1);
        }
    }

    fn read_prg_rom(&self, addr: u16) -> u8 {
        let mut addr = addr - 0x8000;
        
        if self.mapper == 2 {
            let bank_size = 0x4000; // 16KB
            
            if addr < bank_size as u16 {
                // $8000-$BFFF: Switchable bank
                let target_bank = self.prg_bank % (self.prg_rom.len() / bank_size);
                let offset = (target_bank * bank_size) + addr as usize;
                self.prg_rom[offset]
            } else {
                // $C000-$FFFF: Fixed to the last bank
                let last_bank = (self.prg_rom.len() / bank_size) - 1;
                let offset = (last_bank * bank_size) + (addr as usize - 0x4000);
                self.prg_rom[offset]
            }
        } else {
            // Default Mapper 0 behavior
            // Mirror if PRG ROM is 16KB (NROM-128)
            if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
                addr = addr % 0x4000;
            }
            self.prg_rom[addr as usize]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ppu::Ppu;
    use crate::cartridge::Mirroring;

    fn create_test_bus_mapper_2() -> Bus {
        let ppu = Ppu::new(Mirroring::Horizontal, vec![0; 2048]);
        let mut prg_rom = Vec::with_capacity(64 * 1024);
        for bank in 0..4u8 {
            prg_rom.extend(vec![bank; 16384]);
        }
        Bus::new(ppu, prg_rom, 2)
    }

    #[test]
    fn test_mapper_2_bank_switching() {
        let mut bus = create_test_bus_mapper_2();
        
        // Initial state, bank 0 is at 0x8000
        assert_eq!(bus.read(0x8000), 0);
        assert_eq!(bus.read(0xBFFF), 0);
        
        // Fixed bank is always the last bank (Bank 3) at 0xC000
        assert_eq!(bus.read(0xC000), 3);
        assert_eq!(bus.read(0xFFFF), 3);
        
        // Switch to Bank 1
        bus.write(0x8000, 1);
        assert_eq!(bus.read(0x8000), 1);
        assert_eq!(bus.read(0xBFFF), 1);
        
        // Fixed bank is untouched
        assert_eq!(bus.read(0xC000), 3);
        assert_eq!(bus.read(0xFFFF), 3);
        
        // Switch to Bank 2
        bus.write(0xFFFF, 2);
        assert_eq!(bus.read(0x8000), 2);
        assert_eq!(bus.read(0xBFFF), 2);
    }

    fn create_test_bus_mapper_3() -> Bus {
        let mut chr_rom = Vec::with_capacity(32 * 1024);
        for bank in 0..4u8 {
            chr_rom.extend(vec![bank; 8192]);
        }
        let mut ppu = Ppu::new(Mirroring::Horizontal, chr_rom);
        ppu.mapper = 3;
        
        let prg_rom = vec![0; 0x8000]; // 32KB PRG ROM
        Bus::new(ppu, prg_rom, 3)
    }

    #[test]
    fn test_mapper_3_chr_bank_switching() {
        let mut bus = create_test_bus_mapper_3();
        
        // Initial state, CHR bank 0 is mapped
        assert_eq!(bus.ppu.read_register(0x2002), 0); // Need to test via ppu.read_vram directly, but we can't easily without PPU address latch setup
        
        // We'll read VRAM directly for testing purposes, assuming read_vram is pub or visible
        // However read_vram is private, so we test by interacting through `Ppu` public state
        // Let's modify Ppu to make `read_vram` testable or just use a helper method/struct
        
        // Since `read_vram` is private, we will update `ppu::read_vram` call internally by doing a PPUDATA read.
        
        // Setup PPUADDR to 0x0000
        bus.ppu.write_register(0x2006, 0x00);
        bus.ppu.write_register(0x2006, 0x00);
        
        // Read PPUDATA (first read is buffered)
        bus.ppu.read_register(0x2007); 
        let val1 = bus.ppu.read_register(0x2007); // Now reads from 0x0001 (which was buffered from 0x0000)
        assert_eq!(val1, 0); // Bank 0 data
        
        // Switch to CHR Bank 2
        bus.write(0x8000, 2);
        
        // Reset PPUADDR to 0x0000
        bus.ppu.write_register(0x2006, 0x00);
        bus.ppu.write_register(0x2006, 0x00);
        
        // Read PPUDATA
        bus.ppu.read_register(0x2007); // buff
        let val2 = bus.ppu.read_register(0x2007); 
        assert_eq!(val2, 2); // Bank 2 data
    }
}
