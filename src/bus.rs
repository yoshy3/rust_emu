use crate::apu::Apu;
use crate::cartridge::Mirroring;
use crate::joypad::Joypad;
use crate::ppu::Ppu;

pub struct Bus {
    pub cpu_vram: [u8; 2048],
    pub prg_rom: Vec<u8>,
    pub ppu: Ppu,
    pub cycles: usize, // Accumulated cycles (e.g. from DMA)
    pub joypad1: Joypad,
    pub apu: Apu,
    pub mapper: u8,
    pub prg_bank: usize,
    pub prg_ram: Vec<u8>,
    pub prg_ram_enabled: bool,
    pub has_battery: bool,
    pub mmc1_shift: u8,
    pub mmc1_control: u8,
    pub mmc1_chr_bank0: u8,
    pub mmc1_chr_bank1: u8,
    pub mmc1_prg_bank: u8,
    pub cpu_step_counter: u64,
    pub mmc1_last_write_step: Option<u64>,
    pub mmc1_debug: bool,
    /// Tracks how many PPU cycles have been "caught up" during the current CPU instruction.
    /// This is used to simulate parallel CPU/PPU execution: the PPU advances 3 cycles
    /// for every CPU memory access, so register reads see the correct PPU state.
    pub ppu_cycles_advanced: u16,
}

impl Bus {
    pub fn new(ppu: Ppu, rom: Vec<u8>, mapper: u8, prg_ram_size: usize, has_battery: bool) -> Self {
        let mmc1_debug = std::env::var("MMC1_LOG")
            .map(|value| value != "0" && !value.is_empty())
            .unwrap_or(false);

        let mut bus = Self {
            cpu_vram: [0; 2048],
            prg_rom: rom,
            ppu,
            cycles: 0,
            joypad1: Joypad::new(),
            apu: Apu::new(),
            mapper,
            prg_bank: 0,
            prg_ram: vec![0; prg_ram_size.max(0x2000)],
            prg_ram_enabled: true,
            has_battery,
            mmc1_shift: 0x10,
            mmc1_control: 0x0C,
            mmc1_chr_bank0: 0,
            mmc1_chr_bank1: 0,
            mmc1_prg_bank: 0,
            cpu_step_counter: 0,
            mmc1_last_write_step: None,
            mmc1_debug,
            ppu_cycles_advanced: 0,
        };
        bus.sync_mmc1_state_to_ppu();
        if bus.mapper == 1 && bus.mmc1_debug {
            bus.mmc1_log("init");
        }
        bus
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        // PPU catch-up: advance PPU 3 cycles per CPU memory access cycle
        // This simulates parallel CPU/PPU execution so PPU register reads
        // see the correct PPU state (e.g., VBlank flag at scanline 241).
        self.ppu_cycles_advanced += 3;
        self.ppu.tick(3);

        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF],
            0x2000..=0x3FFF => {
                let reg = addr & 0x2007;
                self.ppu.read_register(reg)
            }
            0x4014 => 0, // DMA register
            0x4015 => self.apu.read_status(),
            0x4016 => self.joypad1.read(),
            0x4017 => 0, // Joypad 2 (not implemented)
            0x6000..=0x7FFF => self.read_prg_ram(addr),
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => 0,
        }
    }

    pub fn begin_cpu_step(&mut self) {
        self.cpu_step_counter = self.cpu_step_counter.wrapping_add(1);
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
            0x6000..=0x7FFF => self.read_prg_ram(addr),
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        // PPU catch-up: advance PPU 3 cycles per CPU memory access cycle
        self.ppu_cycles_advanced += 3;
        self.ppu.tick(3);

        match addr {
            0x0000..=0x1FFF => self.cpu_vram[(addr as usize) & 0x7FF] = data,
            0x2000..=0x3FFF => {
                let reg = addr & 0x2007;
                self.ppu.write_register(reg, data)
            }
            0x4014 => {
                self.dma_transfer(data);
            }
            0x4016 => {
                self.joypad1.write(data);
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                self.apu.write_register(addr, data);
            }
            0x6000..=0x7FFF => {
                self.write_prg_ram(addr, data);
            }
            0x8000..=0xFFFF => {
                if self.mapper == 2 {
                    self.prg_bank = data as usize;
                } else if self.mapper == 3 {
                    self.ppu.chr_bank = data as usize;
                } else if self.mapper == 1 {
                    self.write_mmc1(addr, data);
                }
            }
            _ => {}
        }
    }

    pub fn load_battery_ram(&mut self, data: &[u8]) {
        let len = self.prg_ram.len().min(data.len());
        self.prg_ram[..len].copy_from_slice(&data[..len]);
    }

    pub fn battery_ram_data(&self) -> Option<&[u8]> {
        if self.has_battery && !self.prg_ram.is_empty() {
            Some(&self.prg_ram)
        } else {
            None
        }
    }

    pub fn reset_mapper_state(&mut self) {
        self.prg_bank = 0;
        self.prg_ram_enabled = true;
        self.mmc1_shift = 0x10;
        self.mmc1_control = 0x0C;
        self.mmc1_chr_bank0 = 0;
        self.mmc1_chr_bank1 = 0;
        self.mmc1_prg_bank = 0;
        self.mmc1_last_write_step = None;
        self.ppu.chr_bank = 0;
        self.sync_mmc1_state_to_ppu();
    }

    pub fn set_mmc1_debug(&mut self, enabled: bool) {
        self.mmc1_debug = enabled;
        if self.mapper == 1 && self.mmc1_debug {
            self.mmc1_log("debug enabled");
        }
    }

    fn read_prg_ram(&self, addr: u16) -> u8 {
        if self.prg_ram.is_empty() {
            return 0;
        }
        if self.mapper == 1 && !self.mmc1_is_prg_ram_enabled() {
            return 0;
        }
        let offset = (addr as usize - 0x6000) % self.prg_ram.len();
        self.prg_ram[offset]
    }

    fn write_prg_ram(&mut self, addr: u16, data: u8) {
        if self.prg_ram.is_empty() {
            return;
        }
        if self.mapper == 1 && !self.mmc1_is_prg_ram_enabled() {
            return;
        }
        let offset = (addr as usize - 0x6000) % self.prg_ram.len();
        self.prg_ram[offset] = data;
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

        if self.mapper == 1 {
            return self.read_prg_rom_mmc1(addr as usize);
        }

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

    fn write_mmc1(&mut self, addr: u16, data: u8) {
        if self.mmc1_last_write_step == Some(self.cpu_step_counter) {
            if self.mmc1_debug {
                self.mmc1_log(&format!(
                    "ignore consecutive addr=${:04X} data=${:02X}",
                    addr, data
                ));
            }
            return;
        }
        self.mmc1_last_write_step = Some(self.cpu_step_counter);

        if (data & 0x80) != 0 {
            self.mmc1_shift = 0x10;
            self.mmc1_control |= 0x0C;
            self.sync_mmc1_state_to_ppu();
            if self.mmc1_debug {
                self.mmc1_log(&format!("reset addr=${:04X} data=${:02X}", addr, data));
            }
            return;
        }

        let complete = (self.mmc1_shift & 0x01) != 0;
        self.mmc1_shift >>= 1;
        self.mmc1_shift |= (data & 0x01) << 4;

        if complete {
            let value = self.mmc1_shift & 0x1F;
            let target = match addr {
                0x8000..=0x9FFF => "control",
                0xA000..=0xBFFF => "chr0",
                0xC000..=0xDFFF => "chr1",
                0xE000..=0xFFFF => "prg",
                _ => "unknown",
            };
            match addr {
                0x8000..=0x9FFF => self.mmc1_control = value,
                0xA000..=0xBFFF => self.mmc1_chr_bank0 = value,
                0xC000..=0xDFFF => self.mmc1_chr_bank1 = value,
                0xE000..=0xFFFF => {
                    self.mmc1_prg_bank = value;
                    self.prg_ram_enabled = (value & 0x10) == 0;
                }
                _ => {}
            }

            self.mmc1_shift = 0x10;
            self.sync_mmc1_state_to_ppu();
            if self.mmc1_debug {
                self.mmc1_log(&format!(
                    "commit {} addr=${:04X} value=${:02X}",
                    target, addr, value
                ));
            }
        }
    }

    fn sync_mmc1_state_to_ppu(&mut self) {
        if self.mapper != 1 {
            return;
        }

        self.ppu.mmc1_control = self.mmc1_control;
        self.ppu.mmc1_chr_bank0 = self.mmc1_chr_bank0;
        self.ppu.mmc1_chr_bank1 = self.mmc1_chr_bank1;

        if self.ppu.mirroring != Mirroring::FourScreen {
            self.ppu.mirroring = match self.mmc1_control & 0x03 {
                0 => Mirroring::OneScreenLower,
                1 => Mirroring::OneScreenUpper,
                2 => Mirroring::Vertical,
                _ => Mirroring::Horizontal,
            };
        }
    }

    fn read_prg_rom_mmc1(&self, addr: usize) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank_size_16k = 0x4000usize;
        let total_16k_banks = (self.prg_rom.len() / bank_size_16k).max(1);

        let prg_mode = (self.mmc1_control >> 2) & 0x03;
        let region_bit = if self.prg_rom.len() > 0x40000 {
            ((self.mmc1_chr_bank0 >> 4) & 0x01) as usize
        } else {
            0
        };
        let region_base = region_bit << 4;
        let switch_bank_16k =
            (region_base | (self.mmc1_prg_bank as usize & 0x0F)) % total_16k_banks;
        let first_bank_16k = (region_base % total_16k_banks) % total_16k_banks;
        let last_bank_16k = ((region_base | 0x0F) % total_16k_banks) % total_16k_banks;

        match prg_mode {
            0 | 1 => {
                let bank_size_32k = 0x8000usize;
                let total_32k_banks = (self.prg_rom.len() / bank_size_32k).max(1);
                let bank_32k =
                    ((region_base | (self.mmc1_prg_bank as usize & 0x0E)) >> 1) % total_32k_banks;
                let offset = bank_32k * bank_size_32k + addr;
                self.prg_rom[offset % self.prg_rom.len()]
            }
            2 => {
                if addr < 0x4000 {
                    let offset = first_bank_16k * bank_size_16k + addr;
                    self.prg_rom[offset % self.prg_rom.len()]
                } else {
                    let offset = switch_bank_16k * bank_size_16k + (addr - 0x4000);
                    self.prg_rom[offset % self.prg_rom.len()]
                }
            }
            _ => {
                if addr < 0x4000 {
                    let offset = switch_bank_16k * bank_size_16k + addr;
                    self.prg_rom[offset % self.prg_rom.len()]
                } else {
                    let offset = last_bank_16k * bank_size_16k + (addr - 0x4000);
                    self.prg_rom[offset % self.prg_rom.len()]
                }
            }
        }
    }

    fn mmc1_is_prg_ram_enabled(&self) -> bool {
        if self.mapper != 1 {
            return true;
        }

        true
    }

    fn mmc1_prg_window_banks(&self) -> (usize, usize) {
        let bank_size_16k = 0x4000usize;
        let total_16k_banks = (self.prg_rom.len() / bank_size_16k).max(1);

        let prg_mode = (self.mmc1_control >> 2) & 0x03;
        let region_bit = if self.prg_rom.len() > 0x40000 {
            ((self.mmc1_chr_bank0 >> 4) & 0x01) as usize
        } else {
            0
        };
        let region_base = region_bit << 4;
        let switch_bank_16k =
            (region_base | (self.mmc1_prg_bank as usize & 0x0F)) % total_16k_banks;
        let first_bank_16k = (region_base % total_16k_banks) % total_16k_banks;
        let last_bank_16k = ((region_base | 0x0F) % total_16k_banks) % total_16k_banks;

        match prg_mode {
            0 | 1 => {
                let bank_32k = (switch_bank_16k & !1) % total_16k_banks;
                (bank_32k, (bank_32k + 1) % total_16k_banks)
            }
            2 => (first_bank_16k, switch_bank_16k),
            _ => (switch_bank_16k, last_bank_16k),
        }
    }

    fn mmc1_log(&self, event: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (bank8000, bankc000) = self.mmc1_prg_window_banks();
            println!(
                "[MMC1] {} control=${:02X} chr0=${:02X} chr1=${:02X} prg=${:02X} mir={:?} prg_ram={} prg[$8000]={} prg[$C000]={}",
                event,
                self.mmc1_control,
                self.mmc1_chr_bank0,
                self.mmc1_chr_bank1,
                self.mmc1_prg_bank,
                self.ppu.mirroring,
                if self.mmc1_is_prg_ram_enabled() { "on" } else { "off" },
                bank8000,
                bankc000
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Mirroring;
    use crate::ppu::Ppu;

    fn create_test_bus_mapper_2() -> Bus {
        let ppu = Ppu::new(Mirroring::Horizontal, vec![0; 2048]);
        let mut prg_rom = Vec::with_capacity(64 * 1024);
        for bank in 0..4u8 {
            prg_rom.extend(vec![bank; 16384]);
        }
        Bus::new(ppu, prg_rom, 2, 8192, false)
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
        Bus::new(ppu, prg_rom, 3, 8192, false)
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
