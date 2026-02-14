use crate::cartridge::Mirroring;

pub struct Ppu {
    pub vram: [u8; 2048],
    pub oam: [u8; 256],
    pub palette: [u8; 32],
    
    // Registers
    pub ctrl: u8,      // $2000
    pub mask: u8,      // $2001
    pub status: u8,    // $2002
    pub oam_addr: u8,  // $2003
    pub oam_data: u8,  // $2004
    pub scroll: u8,    // $2005
    pub addr: u8,      // $2006 (PPUADDR is 16-bit, but accessed via 8-bit writes)
    pub data: u8,      // $2007

    // Internal State
    pub v: u16,        // Current VRAM address (15 bits)
    pub t: u16,        // Temporary VRAM address (15 bits)
    pub x: u8,         // Fine X scroll (3 bits)
    pub w: bool,       // Write latch (1st or 2nd write toggle)
    
    pub io_databus: u8, // Internal data bus latch (for reading/writing)
    pub buffered_data: u8, // For PPUDATA read buffer

    pub mirroring: Mirroring,
    pub chr_rom: Vec<u8>,
    pub chr_ram: [u8; 8192],
    
    pub scanline: u16,
    pub cycle: u16,

    // Background Rendering State
    pub bg_next_tile_id: u8,
    pub bg_next_tile_attr: u8,
    pub bg_next_tile_lsb: u8,
    pub bg_next_tile_msb: u8,
    
    pub bg_shifter_pattern_lo: u16,
    pub bg_shifter_pattern_hi: u16,
    pub bg_shifter_attrib_lo: u16,
    pub bg_shifter_attrib_hi: u16,
    
    // Sprite Rendering State
    pub secondary_oam: [u8; 32],
    pub sprite_count: u8,
    pub sprite_shifter_pattern_lo: [u8; 8],
    pub sprite_shifter_pattern_hi: [u8; 8],
    pub sprite_latch_x: [u8; 8],
    pub sprite_latch_attr: [u8; 8],  
    pub b_sprite_zero_hit_possible: bool,
    pub b_sprite_zero_being_rendered: bool,
    pub odd_frame: bool,

    pub frame_buffer: Vec<u8>,
}

const SYSTEM_PALETTE: [(u8, u8, u8); 64] = [
    (0x54, 0x54, 0x54), (0x00, 0x1E, 0x74), (0x08, 0x10, 0x90), (0x30, 0x00, 0x88), (0x44, 0x00, 0x64), (0x5C, 0x00, 0x30), (0x54, 0x04, 0x00), (0x3C, 0x18, 0x00),
    (0x20, 0x2A, 0x00), (0x08, 0x3A, 0x00), (0x00, 0x40, 0x00), (0x00, 0x3C, 0x24), (0x00, 0x32, 0x60), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0x98, 0x98, 0x98), (0x08, 0x4A, 0xBC), (0x30, 0x32, 0xD0), (0x68, 0x22, 0xB4), (0xA4, 0x00, 0x9C), (0xBC, 0x00, 0x70), (0xB0, 0x14, 0x3C), (0x94, 0x36, 0x00),
    (0x72, 0x48, 0x00), (0x2A, 0x5E, 0x00), (0x00, 0x6A, 0x00), (0x00, 0x64, 0x38), (0x00, 0x56, 0x84), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0xEC, 0xEE, 0xEC), (0x40, 0x88, 0xFC), (0x68, 0x6A, 0xFC), (0x9C, 0x56, 0xFC), (0xD0, 0x54, 0xFC), (0xEC, 0x40, 0xC4), (0xE8, 0x58, 0x74), (0xD0, 0x70, 0x2C),
    (0xAB, 0x8E, 0x00), (0x74, 0xA4, 0x00), (0x40, 0xB8, 0x00), (0x18, 0xB8, 0x58), (0x18, 0xA8, 0xC0), (0x3C, 0x3C, 0x3C), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
    (0xFC, 0xFC, 0xFC), (0xA0, 0xD2, 0xFC), (0xB4, 0xC6, 0xFC), (0xD0, 0xBA, 0xFC), (0xE4, 0xB6, 0xFC), (0xFC, 0xB2, 0xE0), (0xFC, 0xBC, 0xB0), (0xFC, 0xD0, 0x8C),
    (0xE8, 0xE2, 0x70), (0xCC, 0xEE, 0x62), (0xAC, 0xFA, 0x7A), (0xAC, 0xFE, 0xB0), (0xAC, 0xF6, 0xF8), (0xBC, 0xBC, 0xBC), (0x00, 0x00, 0x00), (0x00, 0x00, 0x00),
];

impl Ppu {
    pub fn new(mirroring: Mirroring, chr_rom: Vec<u8>) -> Self {
        Self {
            vram: [0; 2048],
            oam: [0; 256],
            palette: [0; 32],
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam_data: 0,
            scroll: 0,
            addr: 0,
            data: 0,
            v: 0,
            t: 0,
            x: 0,
            w: false,
            io_databus: 0,
            buffered_data: 0,
            mirroring,
            chr_rom,
            chr_ram: [0; 8192],
            scanline: 0,
            cycle: 0,
            bg_next_tile_id: 0,
            bg_next_tile_attr: 0,
            bg_next_tile_lsb: 0,
            bg_next_tile_msb: 0,
            bg_shifter_pattern_lo: 0,
            bg_shifter_pattern_hi: 0,
            bg_shifter_attrib_lo: 0,
            bg_shifter_attrib_hi: 0,
            
            secondary_oam: [0; 32],
            sprite_count: 0,
            sprite_shifter_pattern_lo: [0; 8],
            sprite_shifter_pattern_hi: [0; 8],
            sprite_latch_x: [0; 8],
            sprite_latch_attr: [0; 8],
            b_sprite_zero_hit_possible: false,
            b_sprite_zero_being_rendered: false,
            odd_frame: false,

            frame_buffer: vec![0; 256 * 240 * 4],
        }
    }

    pub fn tick(&mut self, cycles: u16) -> bool {
        let mut nmi_triggered = false;
        
        for _ in 0..cycles {
            // Cycle/Scanline management
            if self.cycle >= 340 {
                self.cycle = 0;
                self.scanline += 1;
                
                if self.scanline >= 262 {
                    self.scanline = 0;
                    self.odd_frame = !self.odd_frame;
                    // Skip cycle 0 on odd frames if background or sprite rendering is enabled
                    if self.odd_frame && (self.mask & 0x18) != 0 {
                        self.cycle = 1;
                    }
                }
            } else {
                self.cycle += 1;
            }

            // Background Rendering
            if self.mask & 0x18 != 0 {
                if self.scanline < 240 || self.scanline == 261 {
                    // Visible lines + Pre-render line
                    
                    if self.cycle > 0 && self.cycle <= 256 {
                        self.update_shifters();
                        self.update_sprite_shifters();
                        
                        match (self.cycle - 1) % 8 {
                            0 => {
                                self.load_background_shifters();
                                self.fetch_nt_byte();
                            },
                            2 => self.fetch_at_byte(),
                            4 => self.fetch_low_tile_byte(),
                            6 => self.fetch_high_tile_byte(),
                            7 => self.increment_scroll_x(),
                            _ => {}
                        }
                        
                        if self.cycle == 256 {
                            self.increment_scroll_y();
                        }
                    } else if self.cycle == 257 {
                        self.load_background_shifters();
                        self.transfer_address_x();
                        
                        // Sprite Evaluation
                        if self.scanline < 240 {
                            self.evaluate_sprites();
                        } else {
                            self.sprite_count = 0;
                        }

                    } else if self.cycle > 320 && self.cycle <= 336 {
                         if self.cycle == 321 {
                            // Load sprites for next line
                            // Copy possible sprite 0 flag
                            self.b_sprite_zero_being_rendered = self.b_sprite_zero_hit_possible;
                            self.prepare_sprite_shifters();
                         }
                    
                        self.update_shifters();
                        match (self.cycle - 1) % 8 {
                            0 => {
                                self.load_background_shifters();
                                self.fetch_nt_byte();
                            },
                            2 => self.fetch_at_byte(),
                            4 => self.fetch_low_tile_byte(),
                            6 => self.fetch_high_tile_byte(),
                            7 => self.increment_scroll_x(),
                            _ => {}
                        }
                    } else if self.cycle == 337 || self.cycle == 339 {
                         // Unused fetches
                    }
                    
                    // Specific logic for Scanline 261 (Pre-render)
                    if self.scanline == 261 {
                         if self.cycle == 1 {
                             self.b_sprite_zero_hit_possible = false;
                             self.b_sprite_zero_being_rendered = false;
                         }
                         if self.cycle >= 280 && self.cycle <= 304 {
                             self.transfer_address_y();
                         }
                    }
                }
            }
            
            // Pixel Output (Visible lines only)
            if self.scanline < 240 && self.cycle >= 1 && self.cycle <= 256 {
                self.render_pixel();
            }

            // VBlank / NMI Logic
            if self.scanline == 241 && self.cycle == 1 {
                self.status |= 0x80; // Set VBlank flag
                if (self.ctrl & 0x80) != 0 {
                    nmi_triggered = true;
                }
            }
            
            if self.scanline == 261 && self.cycle == 1 {
                self.status &= !0x80; // Clear VBlank flag
                self.status &= !0x40; // Clear Sprite 0 Hit
                self.status &= !0x20; // Clear Sprite Overflow
                // Also reset shifters?
            }
        }
        
        nmi_triggered
    }

    pub fn draw(&self, frame: &mut [u8]) {
        frame.copy_from_slice(&self.frame_buffer);
    }

    // Register Read/Write
    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x2002 => self.read_status(),
            0x2004 => self.read_oam_data(),
            0x2007 => self.read_data(),
            _ => 0, // Write-only registers typically return open bus or last written value
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        self.io_databus = data;
        match addr {
            0x2000 => self.write_ctrl(data),
            0x2001 => self.write_mask(data),
            0x2003 => self.write_oam_addr(data),
            0x2004 => self.write_oam_data(data),
            0x2005 => self.write_scroll(data),
            0x2006 => self.write_addr(data),
            0x2007 => self.write_data(data),
            _ => {},
        }
    }

    // $2000 PPUCTRL
    fn write_ctrl(&mut self, data: u8) {
        self.ctrl = data;
        // Update t: base nametable select (bits 0-1)
        // t: ...GH.. ........ <- d: ......GH
        self.t = (self.t & 0xF3FF) | (((data & 0x03) as u16) << 10);
    }

    // $2001 PPUMASK
    fn write_mask(&mut self, data: u8) {
        self.mask = data;
    }

    // $2002 PPUSTATUS (Read)
    fn read_status(&mut self) -> u8 {
        let status = self.status;
        // W latch is cleared on status read
        self.w = false;
        // Open bus behavior for lower 5 bits (not fully implemented here, using status)
        // Actually top 3 bits are reliable, bottom 5 are usually open bus (often last written value)
        // For simplicity, return constructed status
        
        // VBlank flag is cleared reading PPUSTATUS
        self.status &= !0x80;
        
        status
    }

    // $2003 OAMADDR
    fn write_oam_addr(&mut self, data: u8) {
        self.oam_addr = data;
    }

    // $2004 OAMDATA (Read/Write)
    fn read_oam_data(&self) -> u8 {
        self.oam[self.oam_addr as usize]
    }

    fn write_oam_data(&mut self, data: u8) {
        self.oam[self.oam_addr as usize] = data;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    // $2005 PPUSCROLL
    fn write_scroll(&mut self, data: u8) {
        if !self.w {
            // First write: X scroll
            // t: ....... ...HGFED <- d: HGFED...
            // x:              CBA <- d: .....CBA
            self.t = (self.t & 0xFFE0) | ((data >> 3) as u16);
            self.x = data & 0x07;
            self.w = true;
        } else {
            // Second write: Y scroll
            // t: CBA..HG FED..... <- d: HGFEDCBA
            self.t = (self.t & 0x8FFF) | (((data & 0x07) as u16) << 12);
            self.t = (self.t & 0xFC1F) | (((data & 0xF8) as u16) << 2);
            self.w = false;
        }
    }

    // $2006 PPUADDR
    fn write_addr(&mut self, data: u8) {
        if !self.w {
            // First write: High byte
            // t: ..FEDCB A....... <- d: ..FEDCBA
            // t: .X..... ........ <- 0 (Clear bit 14)
            self.t = (self.t & 0x80FF) | (((data & 0x3F) as u16) << 8);
            self.w = true;
        } else {
            // Second write: Low byte
            // t: ....... HGFEDCBA <- d: HGFEDCBA
            // v = t
            self.t = (self.t & 0xFF00) | (data as u16);
            self.v = self.t;
            self.w = false;
        }
    }

    // $2007 PPUDATA
    fn read_data(&mut self) -> u8 {
        let mut value = self.read_vram(self.v);

        // Buffered read for VRAM (up to 0x3EFF)
        // Palette read (0x3F00+) is immediate
        if self.v % 0x4000 < 0x3F00 {
            let buffered = self.buffered_data;
            self.buffered_data = value;
            value = buffered;
        } else {
            self.buffered_data = self.read_vram(self.v - 0x1000); // Buffer under-the-hood VRAM
        }
        
        self.increment_vram_addr();
        value
    }

    fn write_data(&mut self, data: u8) {
        self.write_vram(self.v, data);
        self.increment_vram_addr();
    }

    fn increment_vram_addr(&mut self) {
        // Increment determined by bit 2 of PPUCTRL (0: +1, 1: +32)
        let increment = if (self.ctrl & 0x04) == 0 { 1 } else { 32 };
        self.v = (self.v + increment) & 0x7FFF; // 15-bit wrap
    }

    // VRAM Access
    fn read_vram(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                // Read from CHR ROM/RAM
                // If CHR ROM is present, read from it.
                // If CHR RAM (no CHR ROM), use vram? No, need dedicated CHR RAM.
                // For now, assume CHR ROM is correct or CHR RAM if empty.
                if self.chr_rom.len() > 0 {
                     self.chr_rom[addr as usize % self.chr_rom.len()]
                } else {
                    self.chr_ram[addr as usize]
                }
            }
            0x2000..=0x3EFF => {
                self.read_nametable(addr)
            }
            0x3F00..=0x3FFF => {
                self.read_palette(addr)
            }
            _ => 0,
        }
    }

    fn write_vram(&mut self, addr: u16, data: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                // CHR RAM write
                if self.chr_rom.is_empty() {
                    self.chr_ram[addr as usize] = data;
                }
            }
            0x2000..=0x3EFF => {
                self.write_nametable(addr, data);
            }
            0x3F00..=0x3FFF => {
                self.write_palette(addr, data);
            }
            _ => {},
        }
    }
    
    fn read_nametable(&self, addr: u16) -> u8 {
        let addr = self.mirror_vram_addr(addr);
        self.vram[addr as usize]
    }

    fn write_nametable(&mut self, addr: u16, data: u8) {
        let addr = self.mirror_vram_addr(addr);
        self.vram[addr as usize] = data;
    }

    fn read_palette(&self, addr: u16) -> u8 {
        let mut addr = addr & 0x001F;
        if addr >= 16 && (addr % 4) == 0 {
            addr -= 16; // Mirror 0x3F10, 0x3F14, etc to 0x3F00, 0x3F04...
        }
        self.palette[addr as usize]
    }

    fn write_palette(&mut self, addr: u16, data: u8) {
        let mut addr = addr & 0x001F;
        if addr >= 16 && (addr % 4) == 0 {
            addr -= 16;
        }
        self.palette[addr as usize] = data;
    }
    
    fn mirror_vram_addr(&self, addr: u16) -> u16 {
        // Nametables at 0x2000, size 0x1000 (4KB space), but physical VRAM is 2KB.
        let addr = (addr - 0x2000) % 0x1000; // 0x0000 - 0x0FFF inside Nametable space
        
        // 0x2000, 0x2400, 0x2800, 0x2C00
        let table = addr / 0x400; // 0, 1, 2, 3
        let offset = addr % 0x400;

        match (self.mirroring, table) {
            (Mirroring::Vertical, 2) | (Mirroring::Vertical, 3) => (table - 2) * 0x400 + offset,
            (Mirroring::Vertical, _) => table * 0x400 + offset,

            (Mirroring::Horizontal, 2) | (Mirroring::Horizontal, 1) => (table - 1) * 0x400 + offset, // 1->0, 2->1
             (Mirroring::Horizontal, 3) => (table - 2) * 0x400 + offset, // 3->1
            (Mirroring::Horizontal, _) => table * 0x400 + offset,
            
            _ => addr & 0x7FF, // Fallback?
        }
    }

    // Scrolling Helpers
    fn increment_scroll_x(&mut self) {
        if (self.mask & 0x18) == 0 { return; } // Only if rendering enabled

        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F; // Coarse X = 0
            self.v ^= 0x0400;  // Switch horizontal nametable
        } else {
            self.v += 1;
        }
    }

    fn increment_scroll_y(&mut self) {
        if (self.mask & 0x18) == 0 { return; }

        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000; // Increment Fine Y
        } else {
            self.v &= !0x7000; // Fine Y = 0
            let mut y = (self.v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800; // Switch vertical nametable
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    fn transfer_address_x(&mut self) {
        if (self.mask & 0x18) == 0 { return; }
        // v: .....F.. ...EDCBA = t: .....F.. ...EDCBA
        self.v = (self.v & 0xFBE0) | (self.t & 0x041F);
    }

    fn transfer_address_y(&mut self) {
        if (self.mask & 0x18) == 0 { return; }
        // v: .IHG.ED CBA..... = t: .IHG.ED CBA.....
        self.v = (self.v & 0x841F) | (self.t & 0x7BE0);
    }

    fn load_background_shifters(&mut self) {
        self.bg_shifter_pattern_lo = (self.bg_shifter_pattern_lo & 0xFF00) | self.bg_next_tile_lsb as u16;
        self.bg_shifter_pattern_hi = (self.bg_shifter_pattern_hi & 0xFF00) | self.bg_next_tile_msb as u16;
        
        self.bg_shifter_attrib_lo = (self.bg_shifter_attrib_lo & 0xFF00) | if (self.bg_next_tile_attr & 0x01) != 0 { 0xFF } else { 0x00 };
        self.bg_shifter_attrib_hi = (self.bg_shifter_attrib_hi & 0xFF00) | if (self.bg_next_tile_attr & 0x02) != 0 { 0xFF } else { 0x00 };
    }

    // ... logic inside tick ...

    fn evaluate_sprites(&mut self) {
        // Clear secondary OAM and count
        self.secondary_oam.fill(0xFF);
        self.sprite_count = 0;
        self.b_sprite_zero_hit_possible = false;

        let sprite_height = if (self.ctrl & 0x20) != 0 { 16 } else { 8 };
        // Scan OAM for sprites on this scanline
        // Primary OAM: 64 sprites * 4 bytes
        for i in 0..64 {
            let n = i * 4;
            let y = self.oam[n] as u16; // Y-position
            
            // Check if sprite is on this scanline
            // Note: Sprite Y is delayed by one scanline in rendering? 
            // The scanline variable represents the line *currently being rendered*.
            // We are evaluating for the *next* line (for fetching).
            // But usually logic is: if scanline is inside [y, y+height).
            // Actually, sprite Y in OAM is top coordinate - 1? No, it's just top.
            // But sprites are rendered one line late.
            // If internal `scanline` matches `y`, it will be rendered on `scanline`.
            
            let diff = (self.scanline as i16) - (y as i16);
            
            if diff >= 0 && diff < sprite_height {
                if self.sprite_count < 8 {
                    // Copy to secondary OAM
                    // OAM: Y, Box (Tile), Attr, X
                    // Secondary: same? structure varies but simplified emulator often keeps same.
                    if i == 0 {
                        self.b_sprite_zero_hit_possible = true;
                    }
                    
                    self.secondary_oam[self.sprite_count as usize * 4 + 0] = self.oam[n + 0];
                    self.secondary_oam[self.sprite_count as usize * 4 + 1] = self.oam[n + 1];
                    self.secondary_oam[self.sprite_count as usize * 4 + 2] = self.oam[n + 2];
                    self.secondary_oam[self.sprite_count as usize * 4 + 3] = self.oam[n + 3];
                    
                    self.sprite_count += 1;
                }
            }
        }
    }
    
    fn prepare_sprite_shifters(&mut self) {
        let sprite_height = if (self.ctrl & 0x20) != 0 { 16 } else { 8 };
        
        for i in 0..self.sprite_count {
            let n = i as usize * 4;
            let y = self.secondary_oam[n + 0] as u16;
            let tile_id = self.secondary_oam[n + 1];
            let attr = self.secondary_oam[n + 2];
            let x = self.secondary_oam[n + 3];
            
            self.sprite_latch_x[i as usize] = x;
            self.sprite_latch_attr[i as usize] = attr;
            
            // Fetch Pattern Data
            let mut addr_lo: u16 = 0;
            let mut addr_hi: u16 = 0;
            
            let flip_v = (attr & 0x80) != 0;
            let flip_h = (attr & 0x40) != 0;
            
            let mut row = (self.scanline as u16).wrapping_sub(y);
            
            // 8x8 Mode
            if sprite_height == 8 {
                if flip_v {
                    row = 7 - row;
                }
                
                let table = if (self.ctrl & 0x08) != 0 { 0x1000 } else { 0x0000 };
                addr_lo = table + ((tile_id as u16) << 4) + row;
                addr_hi = addr_lo + 8;
            } else {
                // 8x16 Mode
                if flip_v {
                    row = 15 - row;
                }
                
                let table = if (tile_id & 0x01) != 0 { 0x1000 } else { 0x0000 };
                let index = tile_id & 0xFE;
                
                if row < 8 {
                    // Top half
                    addr_lo = table + ((index as u16) << 4) + row;
                } else {
                    // Bottom half
                    addr_lo = table + (((index + 1) as u16) << 4) + (row - 8);
                }
                addr_hi = addr_lo + 8;
            }
            
            let mut pat_lo = self.read_vram(addr_lo);
            let mut pat_hi = self.read_vram(addr_hi);
            
            // Flip Horizontal implies reversing the bits
            if flip_h {
                pat_lo = pat_lo.reverse_bits();
                pat_hi = pat_hi.reverse_bits();
            }
            
            self.sprite_shifter_pattern_lo[i as usize] = pat_lo;
            self.sprite_shifter_pattern_hi[i as usize] = pat_hi;
        }
    }

    fn update_sprite_shifters(&mut self) {
        if self.mask & 0x18 == 0 { return; }

        for i in 0..self.sprite_count {
            if self.sprite_latch_x[i as usize] > 0 {
                self.sprite_latch_x[i as usize] -= 1;
            } else {
                self.sprite_shifter_pattern_lo[i as usize] <<= 1;
                self.sprite_shifter_pattern_hi[i as usize] <<= 1;
            }
        }
    }
    fn update_shifters(&mut self) {
        if (self.mask & 0x18) != 0 { // If rendering enabled (BG or Sprite)
            self.bg_shifter_pattern_lo <<= 1;
            self.bg_shifter_pattern_hi <<= 1;
            self.bg_shifter_attrib_lo <<= 1;
            self.bg_shifter_attrib_hi <<= 1;
        }
    }

    fn fetch_nt_byte(&mut self) {
        let addr = 0x2000 | (self.v & 0x0FFF);
        self.bg_next_tile_id = self.read_vram(addr);
    }

    fn fetch_at_byte(&mut self) {
        let addr = 0x23C0 | (self.v & 0x0C00) | ((self.v >> 4) & 0x38) | ((self.v >> 2) & 0x07);
        let byte = self.read_vram(addr);
        
        // Coarse X bit 1, Coarse Y bit 1 determine which 2 bits we want
        // Shift determined by (v.coarse_x & 2) and (v.coarse_y & 2)
        // shift = (v & 2) ? 2 : 0 + (v & 64) ? 4 : 0 ??
        // Actually: 
        // Bottom right: (v & 0x0002) && (v & 0x0040) -> shift 6
        // Bottom left:  !(v & 0x0002) && (v & 0x0040) -> shift 4
        // Top right:    (v & 0x0002) && !(v & 0x0040) -> shift 2
        // Top left:     !(v & 0x0002) && !(v & 0x0040) -> shift 0
        
        let shift = ((self.v >> 4) & 4) | (self.v & 2);
        self.bg_next_tile_attr = (byte >> shift) & 0x03;
    }

    fn fetch_low_tile_byte(&mut self) {
        let fine_y = (self.v >> 12) & 7;
        let table = if (self.ctrl & 0x10) != 0 { 0x1000 } else { 0x0000 };
        let addr = table + ((self.bg_next_tile_id as u16) << 4) + fine_y;
        self.bg_next_tile_lsb = self.read_vram(addr);
    }

    fn fetch_high_tile_byte(&mut self) {
        let fine_y = (self.v >> 12) & 7;
        let table = if (self.ctrl & 0x10) != 0 { 0x1000 } else { 0x0000 };
        let addr = table + ((self.bg_next_tile_id as u16) << 4) + fine_y + 8;
        self.bg_next_tile_msb = self.read_vram(addr);
    }

    fn render_pixel(&mut self) {
        let mask_bg = (self.mask & 0x08) != 0;
        let mask_spr = (self.mask & 0x10) != 0;
        let mask_bg_left = (self.mask & 0x02) != 0;
        let mask_spr_left = (self.mask & 0x04) != 0;

        // --- Background Pixel ---
        let mut bg_pixel = 0;
        let mut bg_palette = 0;

        if mask_bg {
            if mask_bg_left || self.cycle > 8 {
                let bit_mux = 0x8000 >> self.x;
                
                let p0 = if (self.bg_shifter_pattern_lo & bit_mux) != 0 { 1 } else { 0 };
                let p1 = if (self.bg_shifter_pattern_hi & bit_mux) != 0 { 2 } else { 0 };
                bg_pixel = p0 | p1;

                let pal0 = if (self.bg_shifter_attrib_lo & bit_mux) != 0 { 1 } else { 0 };
                let pal1 = if (self.bg_shifter_attrib_hi & bit_mux) != 0 { 2 } else { 0 };
                bg_palette = pal0 | pal1;
            }
        }

        // --- Sprite Pixel ---
        let mut fg_pixel = 0;
        let mut fg_palette = 0;
        let mut fg_priority = 0; // 0: Front, 1: Back
        let mut fg_is_sprite_zero = false;

        if mask_spr {
            if mask_spr_left || self.cycle > 8 {
                for i in 0..self.sprite_count {
                    if self.sprite_latch_x[i as usize] == 0 {
                        let p0 = if (self.sprite_shifter_pattern_lo[i as usize] & 0x80) != 0 { 1 } else { 0 };
                        let p1 = if (self.sprite_shifter_pattern_hi[i as usize] & 0x80) != 0 { 2 } else { 0 };
                        let pixel = p0 | p1;

                        // Transparency check: if pixel is 0, it's transparent, look for next sprite
                        if pixel != 0 {
                            fg_pixel = pixel;
                            fg_palette = (self.sprite_latch_attr[i as usize] & 0x03) + 4; // Sprites use palettes 4-7
                            fg_priority = (self.sprite_latch_attr[i as usize] & 0x20) >> 5;
                            
                            // Check for Sprite 0
                            if i == 0 && self.b_sprite_zero_being_rendered {
                                fg_is_sprite_zero = true;
                            }
                            
                            break; // Priority to lower index sprites
                        }
                    }
                }
            }
        }

        // --- Mixing & Priority ---
        let mut final_pixel = 0;
        let mut final_palette = 0;

        if bg_pixel == 0 && fg_pixel == 0 {
            // Background is universal color (0x3F00)
            final_pixel = 0;
            final_palette = 0;
        } else if bg_pixel == 0 && fg_pixel > 0 {
            final_pixel = fg_pixel;
            final_palette = fg_palette;
        } else if bg_pixel > 0 && fg_pixel == 0 {
            final_pixel = bg_pixel;
            final_palette = bg_palette;
        } else if bg_pixel > 0 && fg_pixel > 0 {
            // Both opaque
            
            // Sprite 0 Hit
            if fg_is_sprite_zero {
                // Must actuate on visible pixels (bg & fg) - done
                // Must be at cycle != 255 (255 is right edge, 6502 treats 255 weirdly?)
                // Actually if x < 255. cycle is x+1. So if cycle < 256.
                if self.cycle != 256 { // Avoid right edge edge-case?
                    self.status |= 0x40; // Set Sprite 0 Hit
                }
            }
            
            if fg_priority == 0 {
                // Foreground Priority
                final_pixel = fg_pixel;
                final_palette = fg_palette;
            } else {
                // Background Priority
                final_pixel = bg_pixel;
                final_palette = bg_palette;
            }
        }
        
        // Address: 0x3F00 + (palette << 2) + pixel
        let color_addr = 0x3F00 + ((final_palette as u16) << 2) + (final_pixel as u16);
        let mut color_byte = self.read_vram(color_addr) & 0x3F;
        
        // Grayscale mode
        if (self.mask & 0x01) != 0 {
            color_byte &= 0x30;
        }
        
        let color = SYSTEM_PALETTE[color_byte as usize];
        
        // Plot to frame buffer
        let x = (self.cycle - 1) as usize;
        let y = self.scanline as usize;
        let idx = (y * 256 + x) * 4;
        
        if idx < self.frame_buffer.len() {
            self.frame_buffer[idx] = color.0;
            self.frame_buffer[idx+1] = color.1;
            self.frame_buffer[idx+2] = color.2;
            self.frame_buffer[idx+3] = 0xFF;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_vblank_nmi() {
        let mut ppu = Ppu::new(Mirroring::Horizontal, vec![]);
        
        // Enable NMI
        ppu.write_ctrl(0x80);
        
        // Tick until VBlank (Scanline 241, Cycle 1)
        // Scanline 0-240 is 241 scanlines. Each 341 cycles.
        // 241 * 341 = 82181 cycles.
        // Plus 1 cycle to reach cycle 1 of scanline 241.
        
        // We can jump ahead for testing internal state?
        ppu.scanline = 240;
        ppu.cycle = 340;
        
        // Tick once -> Scanline 241, cycle 0
        let nmi = ppu.tick(1);
        assert!(!nmi);
        assert_eq!(ppu.scanline, 241);
        assert_eq!(ppu.cycle, 0);
        
        // Tick again -> Scanline 241, cycle 1 -> VBlank set, NMI triggered
        let nmi = ppu.tick(1);
        assert!(nmi);
        assert_eq!(ppu.status & 0x80, 0x80);
    }
    
    #[test]
    fn test_ppu_vblank_clear() {
        let mut ppu = Ppu::new(Mirroring::Horizontal, vec![]);
        
        ppu.scanline = 260;
        ppu.cycle = 340;
        ppu.status |= 0x80; // Set VBlank manually
        
        // Tick -> Scanline 261, Cycle 0
        let _ = ppu.tick(1);
        
        // Tick -> Scanline 261, Cycle 1 -> Clear VBlank
        let _ = ppu.tick(1);
        
        assert_eq!(ppu.status & 0x80, 0x00);
        assert_eq!(ppu.scanline, 261);
    }
}
