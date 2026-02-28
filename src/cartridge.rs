#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
    OneScreenLower,
    OneScreenUpper,
}

pub struct Rom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub screen_mirroring: Mirroring,
    pub has_battery: bool,
    pub prg_ram_size: usize,
}

impl Rom {
    pub fn new(raw: &Vec<u8>) -> Result<Rom, String> {
        if &raw[0..4] != b"NES\x1a" {
            return Err("File is not in iNES format".to_string());
        }

        let prg_rom_size = raw[4] as usize * 16384;
        let chr_rom_size = raw[5] as usize * 8192;

        let flags_6 = raw[6];
        let flags_7 = raw[7];

        let mapper = (flags_7 & 0xF0) | (flags_6 >> 4);

        let version = (flags_7 >> 2) & 0b11;
        if version != 0 {
            return Err("iNES 2.0 format is not supported".to_string());
        }

        let four_screen = (flags_6 & 0b1000) != 0;
        let vertical_mirroring = (flags_6 & 0b1) != 0;
        let screen_mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        let has_trainer = (flags_6 & 0b0100) != 0;
        let has_battery = (flags_6 & 0b0010) != 0;
        let prg_ram_units = raw[8] as usize;
        let prg_ram_size = if prg_ram_units == 0 {
            8192
        } else {
            prg_ram_units * 8192
        };
        let prg_rom_start = 16 + if has_trainer { 512 } else { 0 };
        let prg_rom_end = prg_rom_start + prg_rom_size;
        let chr_rom_start = prg_rom_end;
        let chr_rom_end = chr_rom_start + chr_rom_size;

        if raw.len() < chr_rom_end {
            return Err("File is smaller than specified in header".to_string());
        }

        Ok(Rom {
            prg_rom: raw[prg_rom_start..prg_rom_end].to_vec(),
            chr_rom: raw[chr_rom_start..chr_rom_end].to_vec(),
            mapper,
            screen_mirroring,
            has_battery,
            prg_ram_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_rom(prg_banks: u8, chr_banks: u8, mapper: u8, mirroring: Mirroring) -> Vec<u8> {
        let mut rom =
            Vec::with_capacity(16 + prg_banks as usize * 16384 + chr_banks as usize * 8192);

        // Header
        rom.extend_from_slice(b"NES\x1a");
        rom.push(prg_banks);
        rom.push(chr_banks);

        let mut flags6 = (mapper & 0x0F) << 4;
        let flags7 = mapper & 0xF0;

        match mirroring {
            Mirroring::Vertical => flags6 |= 0x01,
            Mirroring::Horizontal => {}
            Mirroring::FourScreen => flags6 |= 0x08,
            Mirroring::OneScreenLower => {}
            Mirroring::OneScreenUpper => {}
        }

        rom.push(flags6);
        rom.push(flags7);
        rom.extend_from_slice(&[0; 8]); // Padding

        // PRG ROM
        rom.extend(vec![1; prg_banks as usize * 16384]);

        // CHR ROM
        rom.extend(vec![2; chr_banks as usize * 8192]);

        rom
    }

    #[test]
    fn test_nes_header_parsing() {
        let raw = create_test_rom(2, 1, 3, Mirroring::Vertical);
        let rom = Rom::new(&raw).unwrap();

        assert_eq!(rom.prg_rom.len(), 2 * 16384);
        assert_eq!(rom.chr_rom.len(), 1 * 8192);
        assert_eq!(rom.mapper, 3);
        assert_eq!(rom.screen_mirroring, Mirroring::Vertical);
        assert!(!rom.has_battery);
        assert_eq!(rom.prg_ram_size, 8192);
    }
}
