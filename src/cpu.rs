use crate::bus::Bus;

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub st: u8,
    pub pc: u16,
    pub sp: u8,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            st: 0,
            pc: 0,
            sp: 0xFD,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.st = 0;
        self.sp = 0xFD;
        
        // Reset vector
        self.pc = (bus.read(0xFFFC) as u16) | ((bus.read(0xFFFD) as u16) << 8);
    }

    pub fn step(&mut self, bus: &mut Bus) {
        // Fetch
        let opcode = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        // Execute
        match opcode {
            // LDA Immediate
            0xA9 => {
                let value = self.fetch_byte(bus);
                self.lda(value);
            }
            // LDX Immediate
            0xA2 => {
                let value = self.fetch_byte(bus);
                self.ldx(value);
            }
            // LDY Immediate
            0xA0 => {
                let value = self.fetch_byte(bus);
                self.ldy(value);
            }
            // NOP
            0xEA => {}
            _ => {
                // Unknown opcode
                #[cfg(not(target_arch = "wasm32"))]
                println!("Unknown opcode: {:02X}", opcode);
            }
        }
    }

    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn lda(&mut self, value: u8) {
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn ldx(&mut self, value: u8) {
        self.x = value;
        self.update_zero_negative_flags(self.x);
    }

    fn ldy(&mut self, value: u8) {
        self.y = value;
        self.update_zero_negative_flags(self.y);
    }

    fn update_zero_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.st |= 0x02; // Set Zero flag
        } else {
            self.st &= !0x02;
        }

        if (result & 0x80) != 0 {
            self.st |= 0x80; // Set Negative flag
        } else {
            self.st &= !0x80;
        }
    }

    fn get_operand_address(&mut self, bus: &mut Bus, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            AddressingMode::ZeroPage => {
                self.fetch_byte(bus) as u16
            }
            AddressingMode::Absolute => {
                self.fetch_word(bus)
            }
            // Add other modes as needed
            _ => 0, // Placeholder
        }
    }

    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        lo | (hi << 8)
    }
}

pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    NoneAddressing,
}
