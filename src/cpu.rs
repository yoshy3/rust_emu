use crate::bus::Bus;

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub st: u8,
    pub pc: u16,
    pub sp: u8,
    /// Extra cycles accumulated during instruction execution
    /// (branch taken penalty, page crossing penalty)
    pub extra_cycles: u16,
    /// Set by get_operand_address when AbsoluteX/AbsoluteY/IndirectY crosses a page boundary
    pub page_crossed: bool,
}

impl Cpu {
    fn irq_log_enabled() -> bool {
        std::env::var("IRQ_LOG")
            .map(|value| value != "0" && !value.is_empty())
            .unwrap_or(false)
    }

    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            st: 0,
            pc: 0,
            sp: 0xFD,
            extra_cycles: 0,
            page_crossed: false,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.st = 0; // nestest expects 0x24 but strictly 0 on startup? Usually 0x34 or 0x24.
        self.sp = 0xFD;

        // Reset vector
        self.pc = (bus.read(0xFFFC) as u16) | ((bus.read(0xFFFD) as u16) << 8);
    }

    pub fn trace(&mut self, bus: &mut Bus) -> String {
        let opcode = bus.peek(self.pc);
        let ops = match crate::opcodes::OPCODES_MAP.get(&opcode) {
            Some(op) => op,
            None => panic!("OpCode {:02X} is not implemented in opcodes.rs", opcode),
        };

        let mut hex_dump = vec![];
        hex_dump.push(opcode);

        let (mem_addr, stored_value) = match ops.mode {
            AddressingMode::Immediate | AddressingMode::NoneAddressing => (0, 0),
            _ => {
                let addr = self.get_absolute_address(bus, &ops.mode, self.pc + 1);
                (addr, bus.peek(addr))
            }
        };

        let tmp = match ops.len {
            1 => match ops.mode {
                AddressingMode::Accumulator => "A ".to_string(),
                _ => "".to_string(),
            },
            2 => {
                let address = bus.peek(self.pc + 1);
                hex_dump.push(address);
                match ops.mode {
                    AddressingMode::Immediate => format!("#${:02X}", address),
                    AddressingMode::ZeroPage => format!("${:02X} = {:02X}", address, stored_value),
                    AddressingMode::ZeroPageX => format!(
                        "${:02X},X @ {:02X} = {:02X}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::ZeroPageY => format!(
                        "${:02X},Y @ {:02X} = {:02X}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::IndirectX => format!(
                        "(${:02X},X) @ {:02X} = {:04X} = {:02X}",
                        address,
                        (address.wrapping_add(self.x)),
                        mem_addr,
                        stored_value
                    ),
                    AddressingMode::IndirectY => format!(
                        "(${:02X}),Y = {:04X} @ {:04X} = {:02X}",
                        address,
                        (mem_addr.wrapping_sub(self.y as u16)),
                        mem_addr,
                        stored_value
                    ),
                    _ => format!("${:02X}", address), // fallback
                }
            }
            3 => {
                let address_lo = bus.peek(self.pc + 1);
                let address_hi = bus.peek(self.pc + 2);
                hex_dump.push(address_lo);
                hex_dump.push(address_hi);

                let address = (address_hi as u16) << 8 | address_lo as u16;

                match ops.mode {
                    AddressingMode::Absolute => {
                        if ops.name == "JMP" || ops.name == "JSR" {
                            format!("${:04X}", address)
                        } else {
                            format!("${:04X} = {:02X}", address, stored_value)
                        }
                    }
                    AddressingMode::AbsoluteX => format!(
                        "${:04X},X @ {:04X} = {:02X}",
                        address, mem_addr, stored_value
                    ),
                    AddressingMode::AbsoluteY => format!(
                        "${:04X},Y @ {:04X} = {:02X}",
                        address, mem_addr, stored_value
                    ),
                    _ => format!("${:04X}", address),
                }
            }
            _ => "".to_string(),
        };

        let hex_str = hex_dump
            .iter()
            .map(|z| format!("{:02X}", z))
            .collect::<Vec<String>>()
            .join(" ");
        let asm_str = format!("{:04X}  {:8} {: >4} {}", self.pc, hex_str, ops.name, tmp);

        format!(
            "{:47} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
            asm_str, self.a, self.x, self.y, self.st, self.sp
        )
        .to_uppercase()
    }

    pub fn step(&mut self, bus: &mut Bus) -> u16 {
        bus.begin_cpu_step();
        self.extra_cycles = 0;
        self.page_crossed = false;

        // Fetch
        let opcode = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        // Execute
        match opcode {
            // LDA
            0xA9 => {
                self.lda(bus, AddressingMode::Immediate);
            }
            0xA5 => {
                self.lda(bus, AddressingMode::ZeroPage);
            }
            0xB5 => {
                self.lda(bus, AddressingMode::ZeroPageX);
            }
            0xAD => {
                self.lda(bus, AddressingMode::Absolute);
            }
            0xBD => {
                self.lda(bus, AddressingMode::AbsoluteX);
            }
            0xB9 => {
                self.lda(bus, AddressingMode::AbsoluteY);
            }
            0xA1 => {
                self.lda(bus, AddressingMode::IndirectX);
            }
            0xB1 => {
                self.lda(bus, AddressingMode::IndirectY);
            }

            // LDX
            0xA2 => {
                self.ldx(bus, AddressingMode::Immediate);
            }
            0xA6 => {
                self.ldx(bus, AddressingMode::ZeroPage);
            }
            0xB6 => {
                self.ldx(bus, AddressingMode::ZeroPageY);
            }
            0xAE => {
                self.ldx(bus, AddressingMode::Absolute);
            }
            0xBE => {
                self.ldx(bus, AddressingMode::AbsoluteY);
            }

            // LDY
            0xA0 => {
                self.ldy(bus, AddressingMode::Immediate);
            }
            0xA4 => {
                self.ldy(bus, AddressingMode::ZeroPage);
            }
            0xB4 => {
                self.ldy(bus, AddressingMode::ZeroPageX);
            }
            0xAC => {
                self.ldy(bus, AddressingMode::Absolute);
            }
            0xBC => {
                self.ldy(bus, AddressingMode::AbsoluteX);
            }

            // STA
            0x85 => {
                self.sta(bus, AddressingMode::ZeroPage);
            }
            0x95 => {
                self.sta(bus, AddressingMode::ZeroPageX);
            }
            0x8D => {
                self.sta(bus, AddressingMode::Absolute);
            }
            0x9D => {
                self.sta(bus, AddressingMode::AbsoluteX);
            }
            0x99 => {
                self.sta(bus, AddressingMode::AbsoluteY);
            }
            0x81 => {
                self.sta(bus, AddressingMode::IndirectX);
            }
            0x91 => {
                self.sta(bus, AddressingMode::IndirectY);
            }

            // STX
            0x86 => {
                self.stx(bus, AddressingMode::ZeroPage);
            }
            0x96 => {
                self.stx(bus, AddressingMode::ZeroPageY);
            }
            0x8E => {
                self.stx(bus, AddressingMode::Absolute);
            }

            // STY
            0x84 => {
                self.sty(bus, AddressingMode::ZeroPage);
            }
            0x94 => {
                self.sty(bus, AddressingMode::ZeroPageX);
            }
            0x8C => {
                self.sty(bus, AddressingMode::Absolute);
            }

            // Increment/Decrement Registers
            0xE8 => {
                self.inx();
            }
            0xC8 => {
                self.iny();
            }
            0xCA => {
                self.dex();
            }
            0x88 => {
                self.dey();
            }

            // Increment/Decrement Memory
            0xE6 => {
                self.inc(bus, AddressingMode::ZeroPage);
            }
            0xF6 => {
                self.inc(bus, AddressingMode::ZeroPageX);
            }
            0xEE => {
                self.inc(bus, AddressingMode::Absolute);
            }
            0xFE => {
                self.inc(bus, AddressingMode::AbsoluteX);
            }

            0xC6 => {
                self.dec(bus, AddressingMode::ZeroPage);
            }
            0xD6 => {
                self.dec(bus, AddressingMode::ZeroPageX);
            }
            0xCE => {
                self.dec(bus, AddressingMode::Absolute);
            }
            0xDE => {
                self.dec(bus, AddressingMode::AbsoluteX);
            }

            // Stack Operations
            0x48 => {
                self.pha(bus);
            }
            0x08 => {
                self.php(bus);
            }
            0x68 => {
                self.pla(bus);
            }
            0x28 => {
                self.plp(bus);
            }

            // Transfers
            0xAA => {
                self.tax();
            }
            0xA8 => {
                self.tay();
            }
            0x8A => {
                self.txa();
            }
            0x98 => {
                self.tya();
            }
            0x9A => {
                self.txs();
            }
            0xBA => {
                self.tsx();
            }

            // Status Flags
            0x18 => {
                self.clc();
            }
            0x38 => {
                self.sec();
            }
            0x58 => {
                self.cli();
            }
            0x78 => {
                self.sei();
            }
            0xB8 => {
                self.clv();
            }
            0xD8 => {
                self.cld();
            }
            0xF8 => {
                self.sed();
            }

            // Logical Operations
            0x29 => {
                self.and(bus, AddressingMode::Immediate);
            }
            0x25 => {
                self.and(bus, AddressingMode::ZeroPage);
            }
            0x35 => {
                self.and(bus, AddressingMode::ZeroPageX);
            }
            0x2D => {
                self.and(bus, AddressingMode::Absolute);
            }
            0x3D => {
                self.and(bus, AddressingMode::AbsoluteX);
            }
            0x39 => {
                self.and(bus, AddressingMode::AbsoluteY);
            }
            0x21 => {
                self.and(bus, AddressingMode::IndirectX);
            }
            0x31 => {
                self.and(bus, AddressingMode::IndirectY);
            }

            0x09 => {
                self.ora(bus, AddressingMode::Immediate);
            }
            0x05 => {
                self.ora(bus, AddressingMode::ZeroPage);
            }
            0x15 => {
                self.ora(bus, AddressingMode::ZeroPageX);
            }
            0x0D => {
                self.ora(bus, AddressingMode::Absolute);
            }
            0x1D => {
                self.ora(bus, AddressingMode::AbsoluteX);
            }
            0x19 => {
                self.ora(bus, AddressingMode::AbsoluteY);
            }
            0x01 => {
                self.ora(bus, AddressingMode::IndirectX);
            }
            0x11 => {
                self.ora(bus, AddressingMode::IndirectY);
            }

            0x49 => {
                self.eor(bus, AddressingMode::Immediate);
            }
            0x45 => {
                self.eor(bus, AddressingMode::ZeroPage);
            }
            0x55 => {
                self.eor(bus, AddressingMode::ZeroPageX);
            }
            0x4D => {
                self.eor(bus, AddressingMode::Absolute);
            }
            0x5D => {
                self.eor(bus, AddressingMode::AbsoluteX);
            }
            0x59 => {
                self.eor(bus, AddressingMode::AbsoluteY);
            }
            0x41 => {
                self.eor(bus, AddressingMode::IndirectX);
            }
            0x51 => {
                self.eor(bus, AddressingMode::IndirectY);
            }

            0x24 => {
                self.bit(bus, AddressingMode::ZeroPage);
            }
            0x2C => {
                self.bit(bus, AddressingMode::Absolute);
            }

            // Compare Operations
            0xC9 => {
                self.cmp(bus, AddressingMode::Immediate);
            }
            0xC5 => {
                self.cmp(bus, AddressingMode::ZeroPage);
            }
            0xD5 => {
                self.cmp(bus, AddressingMode::ZeroPageX);
            }
            0xCD => {
                self.cmp(bus, AddressingMode::Absolute);
            }
            0xDD => {
                self.cmp(bus, AddressingMode::AbsoluteX);
            }
            0xD9 => {
                self.cmp(bus, AddressingMode::AbsoluteY);
            }
            0xC1 => {
                self.cmp(bus, AddressingMode::IndirectX);
            }
            0xD1 => {
                self.cmp(bus, AddressingMode::IndirectY);
            }

            0xE0 => {
                self.cpx(bus, AddressingMode::Immediate);
            }
            0xE4 => {
                self.cpx(bus, AddressingMode::ZeroPage);
            }
            0xEC => {
                self.cpx(bus, AddressingMode::Absolute);
            }

            0xC0 => {
                self.cpy(bus, AddressingMode::Immediate);
            }
            0xC4 => {
                self.cpy(bus, AddressingMode::ZeroPage);
            }
            0xCC => {
                self.cpy(bus, AddressingMode::Absolute);
            }

            // Shifts and Rotates
            0x0A => {
                self.asl_acc();
            }
            0x06 => {
                self.asl(bus, AddressingMode::ZeroPage);
            }
            0x16 => {
                self.asl(bus, AddressingMode::ZeroPageX);
            }
            0x0E => {
                self.asl(bus, AddressingMode::Absolute);
            }
            0x1E => {
                self.asl(bus, AddressingMode::AbsoluteX);
            }

            0x4A => {
                self.lsr_acc();
            }
            0x46 => {
                self.lsr(bus, AddressingMode::ZeroPage);
            }
            0x56 => {
                self.lsr(bus, AddressingMode::ZeroPageX);
            }
            0x4E => {
                self.lsr(bus, AddressingMode::Absolute);
            }
            0x5E => {
                self.lsr(bus, AddressingMode::AbsoluteX);
            }

            0x2A => {
                self.rol_acc();
            }
            0x26 => {
                self.rol(bus, AddressingMode::ZeroPage);
            }
            0x36 => {
                self.rol(bus, AddressingMode::ZeroPageX);
            }
            0x2E => {
                self.rol(bus, AddressingMode::Absolute);
            }
            0x3E => {
                self.rol(bus, AddressingMode::AbsoluteX);
            }

            0x6A => {
                self.ror_acc();
            }
            0x66 => {
                self.ror(bus, AddressingMode::ZeroPage);
            }
            0x76 => {
                self.ror(bus, AddressingMode::ZeroPageX);
            }
            0x6E => {
                self.ror(bus, AddressingMode::Absolute);
            }
            0x7E => {
                self.ror(bus, AddressingMode::AbsoluteX);
            }

            // Arithmetic with Carry
            0x69 => {
                self.adc(bus, AddressingMode::Immediate);
            }
            0x65 => {
                self.adc(bus, AddressingMode::ZeroPage);
            }
            0x75 => {
                self.adc(bus, AddressingMode::ZeroPageX);
            }
            0x6D => {
                self.adc(bus, AddressingMode::Absolute);
            }
            0x7D => {
                self.adc(bus, AddressingMode::AbsoluteX);
            }
            0x79 => {
                self.adc(bus, AddressingMode::AbsoluteY);
            }
            0x61 => {
                self.adc(bus, AddressingMode::IndirectX);
            }
            0x71 => {
                self.adc(bus, AddressingMode::IndirectY);
            }

            0xE9 => {
                self.sbc(bus, AddressingMode::Immediate);
            }
            0xE5 => {
                self.sbc(bus, AddressingMode::ZeroPage);
            }
            0xF5 => {
                self.sbc(bus, AddressingMode::ZeroPageX);
            }
            0xED => {
                self.sbc(bus, AddressingMode::Absolute);
            }
            0xFD => {
                self.sbc(bus, AddressingMode::AbsoluteX);
            }
            0xF9 => {
                self.sbc(bus, AddressingMode::AbsoluteY);
            }
            0xE1 => {
                self.sbc(bus, AddressingMode::IndirectX);
            }
            0xF1 => {
                self.sbc(bus, AddressingMode::IndirectY);
            }

            // Jumps and Calls
            0x4C => {
                self.jmp(bus, AddressingMode::Absolute);
            }
            0x6C => {
                self.jmp(bus, AddressingMode::Indirect);
            } // Need Indirect mode for JMP
            0x20 => {
                self.jsr(bus);
            }
            0x60 => {
                self.rts(bus);
            }

            // System
            0x00 => {
                self.brk(bus);
            }
            0x40 => {
                self.rti(bus);
            }
            0xEA => { /* NOP */ }

            // Branching
            0x10 => {
                self.bpl(bus);
            }
            0x30 => {
                self.bmi(bus);
            }
            0x50 => {
                self.bvc(bus);
            }
            0x70 => {
                self.bvs(bus);
            }
            0x90 => {
                self.bcc(bus);
            }
            0xB0 => {
                self.bcs(bus);
            }
            0xD0 => {
                self.bne(bus);
            }
            0xF0 => {
                self.beq(bus);
            }

            // Unofficial NOPs
            0x04 | 0x14 | 0x34 | 0x44 | 0x54 | 0x64 | 0x74 | 0x80 | 0x82 | 0x89 | 0xC2 | 0xD4
            | 0xE2 | 0xF4 | 0x0C | 0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC | 0x02 | 0x12 | 0x22
            | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
                // All these NOPs read the operand but do nothing.
                let mode = &crate::opcodes::OPCODES_MAP.get(&opcode).unwrap().mode;
                match mode {
                    AddressingMode::Immediate => {
                        self.pc = self.pc.wrapping_add(1);
                    }
                    _ => {
                        let _ = self.get_operand_address(bus, mode);
                    }
                }
            }
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => {
                // NOP Implied - do nothing, no operand fetch (already fetched opcode)
            }

            // SLO
            0x07 => {
                self.slo(bus, AddressingMode::ZeroPage);
            }
            0x17 => {
                self.slo(bus, AddressingMode::ZeroPageX);
            }
            0x0F => {
                self.slo(bus, AddressingMode::Absolute);
            }
            0x1F => {
                self.slo(bus, AddressingMode::AbsoluteX);
            }
            0x1B => {
                self.slo(bus, AddressingMode::AbsoluteY);
            }
            0x03 => {
                self.slo(bus, AddressingMode::IndirectX);
            }
            0x13 => {
                self.slo(bus, AddressingMode::IndirectY);
            }

            // RLA
            0x27 => {
                self.rla(bus, AddressingMode::ZeroPage);
            }
            0x37 => {
                self.rla(bus, AddressingMode::ZeroPageX);
            }
            0x2F => {
                self.rla(bus, AddressingMode::Absolute);
            }
            0x3F => {
                self.rla(bus, AddressingMode::AbsoluteX);
            }
            0x3B => {
                self.rla(bus, AddressingMode::AbsoluteY);
            }
            0x23 => {
                self.rla(bus, AddressingMode::IndirectX);
            }
            0x33 => {
                self.rla(bus, AddressingMode::IndirectY);
            }

            // SRE
            0x47 => {
                self.sre(bus, AddressingMode::ZeroPage);
            }
            0x57 => {
                self.sre(bus, AddressingMode::ZeroPageX);
            }
            0x4F => {
                self.sre(bus, AddressingMode::Absolute);
            }
            0x5F => {
                self.sre(bus, AddressingMode::AbsoluteX);
            }
            0x5B => {
                self.sre(bus, AddressingMode::AbsoluteY);
            }
            0x43 => {
                self.sre(bus, AddressingMode::IndirectX);
            }
            0x53 => {
                self.sre(bus, AddressingMode::IndirectY);
            }

            // RRA
            0x67 => {
                self.rra(bus, AddressingMode::ZeroPage);
            }
            0x77 => {
                self.rra(bus, AddressingMode::ZeroPageX);
            }
            0x6F => {
                self.rra(bus, AddressingMode::Absolute);
            }
            0x7F => {
                self.rra(bus, AddressingMode::AbsoluteX);
            }
            0x7B => {
                self.rra(bus, AddressingMode::AbsoluteY);
            }
            0x63 => {
                self.rra(bus, AddressingMode::IndirectX);
            }
            0x73 => {
                self.rra(bus, AddressingMode::IndirectY);
            }

            // SAX
            0x87 => {
                self.sax(bus, AddressingMode::ZeroPage);
            }
            0x97 => {
                self.sax(bus, AddressingMode::ZeroPageY);
            }
            0x8F => {
                self.sax(bus, AddressingMode::Absolute);
            }
            0x83 => {
                self.sax(bus, AddressingMode::IndirectX);
            }

            // LAX
            0xA7 => {
                self.lax(bus, AddressingMode::ZeroPage);
            }
            0xB7 => {
                self.lax(bus, AddressingMode::ZeroPageY);
            }
            0xAF => {
                self.lax(bus, AddressingMode::Absolute);
            }
            0xBF => {
                self.lax(bus, AddressingMode::AbsoluteY);
            }
            0xA3 => {
                self.lax(bus, AddressingMode::IndirectX);
            }
            0xB3 => {
                self.lax(bus, AddressingMode::IndirectY);
            }

            // DCP
            0xC7 => {
                self.dcp(bus, AddressingMode::ZeroPage);
            }
            0xD7 => {
                self.dcp(bus, AddressingMode::ZeroPageX);
            }
            0xCF => {
                self.dcp(bus, AddressingMode::Absolute);
            }
            0xDF => {
                self.dcp(bus, AddressingMode::AbsoluteX);
            }
            0xDB => {
                self.dcp(bus, AddressingMode::AbsoluteY);
            }
            0xC3 => {
                self.dcp(bus, AddressingMode::IndirectX);
            }
            0xD3 => {
                self.dcp(bus, AddressingMode::IndirectY);
            }

            // ISC
            0xE7 => {
                self.isc(bus, AddressingMode::ZeroPage);
            }
            0xF7 => {
                self.isc(bus, AddressingMode::ZeroPageX);
            }
            0xEF => {
                self.isc(bus, AddressingMode::Absolute);
            }
            0xFF => {
                self.isc(bus, AddressingMode::AbsoluteX);
            }
            0xFB => {
                self.isc(bus, AddressingMode::AbsoluteY);
            }
            0xE3 => {
                self.isc(bus, AddressingMode::IndirectX);
            }
            0xF3 => {
                self.isc(bus, AddressingMode::IndirectY);
            }

            // ALR
            0x4B => {
                self.alr(bus);
            }
            // ANC
            0x0B | 0x2B => {
                self.anc(bus);
            }
            // ARR
            0x6B => {
                self.arr(bus);
            }
            // AXS
            0xCB => {
                self.axs(bus);
            }
            // LAS
            0xBB => {
                self.las(bus);
            }
            // XAA
            0x8B => {
                self.xaa(bus);
            }
            // AHX
            0x93 | 0x9F => {
                let mode = &crate::opcodes::OPCODES_MAP.get(&opcode).unwrap().mode;
                self.ahx(bus, mode);
            }
            // SHY
            0x9C => {
                self.shy(bus);
            }
            // SHX
            0x9E => {
                self.shx(bus);
            }
            // TAS
            0x9B => {
                self.tas(bus);
            }

            _ => {
                // Unknown opcode
                #[cfg(not(target_arch = "wasm32"))]
                println!("Unknown opcode: {:02X}", opcode);
            }
        }

        let base_cycles = crate::opcodes::OPCODES_MAP
            .get(&opcode)
            .map(|op| op.cycles)
            .unwrap_or(0) as u16;

        // Apply page-crossing penalty for read instructions:
        // AbsoluteX/AbsoluteY reads (base 4) and IndirectY reads (base 5)
        // get +1 cycle when crossing a page boundary.
        // Write/RMW instructions already include the extra cycle in their base count.
        if self.page_crossed {
            let mode = crate::opcodes::OPCODES_MAP
                .get(&opcode)
                .map(|op| &op.mode);
            match mode {
                Some(AddressingMode::AbsoluteX) | Some(AddressingMode::AbsoluteY)
                    if base_cycles == 4 =>
                {
                    self.extra_cycles += 1;
                }
                Some(AddressingMode::IndirectY) if base_cycles == 5 => {
                    self.extra_cycles += 1;
                }
                _ => {}
            }
        }

        let dma_cycles = bus.poll_dma_cycles() as u16;
        base_cycles + self.extra_cycles + dma_cycles
    }

    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn lda(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn ldx(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.x = value;
        self.update_zero_negative_flags(self.x);
    }

    fn ldy(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.y = value;
        self.update_zero_negative_flags(self.y);
    }

    fn sta(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        bus.write(addr, self.a);
    }

    fn stx(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        bus.write(addr, self.x);
    }

    fn sty(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        bus.write(addr, self.y);
    }

    fn tax(&mut self) {
        self.x = self.a;
        self.update_zero_negative_flags(self.x);
    }

    fn tay(&mut self) {
        self.y = self.a;
        self.update_zero_negative_flags(self.y);
    }

    fn txa(&mut self) {
        self.a = self.x;
        self.update_zero_negative_flags(self.a);
    }

    fn tya(&mut self) {
        self.a = self.y;
        self.update_zero_negative_flags(self.a);
    }

    fn txs(&mut self) {
        self.sp = self.x;
    }

    fn tsx(&mut self) {
        self.x = self.sp;
        self.update_zero_negative_flags(self.x);
    }

    // Increment/Decrement Registers
    fn inx(&mut self) {
        self.x = self.x.wrapping_add(1);
        self.update_zero_negative_flags(self.x);
    }

    fn iny(&mut self) {
        self.y = self.y.wrapping_add(1);
        self.update_zero_negative_flags(self.y);
    }

    fn dex(&mut self) {
        self.x = self.x.wrapping_sub(1);
        self.update_zero_negative_flags(self.x);
    }

    fn dey(&mut self) {
        self.y = self.y.wrapping_sub(1);
        self.update_zero_negative_flags(self.y);
    }

    // Increment/Decrement Memory
    fn inc(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        let new_value = old_value.wrapping_add(1);
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
    }

    fn dec(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        let new_value = old_value.wrapping_sub(1);
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
    }

    // Stack Operations
    fn push(&mut self, bus: &mut Bus, value: u8) {
        bus.write(0x0100 + self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 + self.sp as u16)
    }

    fn pha(&mut self, bus: &mut Bus) {
        self.push(bus, self.a);
    }

    fn php(&mut self, bus: &mut Bus) {
        // Break flag and bit 5 are set when pushing status
        self.push(bus, self.st | 0x30);
    }

    fn pla(&mut self, bus: &mut Bus) {
        self.a = self.pop(bus);
        self.update_zero_negative_flags(self.a);
    }

    fn plp(&mut self, bus: &mut Bus) {
        self.st = self.pop(bus);
        // Break flag and bit 5 are ignored when pulling
        self.st &= !0x10;
        self.st |= 0x20; // Bit 5 is always 1? Actually bit 5 is unused/always 1.
    }

    // Status Flags
    fn clc(&mut self) {
        self.st &= !0x01;
    }
    fn sec(&mut self) {
        self.st |= 0x01;
    }
    fn cli(&mut self) {
        self.st &= !0x04;
    }
    fn sei(&mut self) {
        self.st |= 0x04;
    }
    fn clv(&mut self) {
        self.st &= !0x40;
    }
    fn cld(&mut self) {
        self.st &= !0x08;
    }
    fn sed(&mut self) {
        self.st |= 0x08;
    }

    // Branching
    fn branch(&mut self, bus: &mut Bus, condition: bool) {
        let offset = self.fetch_byte(bus) as i8;
        if condition {
            self.extra_cycles += 1; // +1 for branch taken
            let jump_addr = self.pc.wrapping_add(offset as u16);
            // +1 more if branch crosses a page boundary
            if (self.pc & 0xFF00) != (jump_addr & 0xFF00) {
                self.extra_cycles += 1;
            }
            self.pc = jump_addr;
        }
    }

    fn bpl(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x80) == 0;
        self.branch(bus, condition);
    }

    fn bmi(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x80) != 0;
        self.branch(bus, condition);
    }

    fn bvc(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x40) == 0;
        self.branch(bus, condition);
    }

    fn bvs(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x40) != 0;
        self.branch(bus, condition);
    }

    fn bcc(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x01) == 0;
        self.branch(bus, condition);
    }

    fn bcs(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x01) != 0;
        self.branch(bus, condition);
    }

    fn bne(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x02) == 0;
        self.branch(bus, condition);
    }

    fn beq(&mut self, bus: &mut Bus) {
        let condition = (self.st & 0x02) != 0;
        self.branch(bus, condition);
    }

    // Logical Operations
    fn and(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.a &= value;
        self.update_zero_negative_flags(self.a);
    }

    fn ora(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.a |= value;
        self.update_zero_negative_flags(self.a);
    }

    fn eor(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.a ^= value;
        self.update_zero_negative_flags(self.a);
    }

    fn bit(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        let result = self.a & value;
        if result == 0 {
            self.st |= 0x02; // Zero flag
        } else {
            self.st &= !0x02;
        }
        self.st = (self.st & 0x3F) | (value & 0xC0); // Copy N and V bits from Memory
    }

    // Compare Operations
    fn cmp(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.compare(self.a, value);
    }

    fn cpx(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.compare(self.x, value);
    }

    fn cpy(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);
        self.compare(self.y, value);
    }

    fn compare(&mut self, reg: u8, value: u8) {
        if reg >= value {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        self.update_zero_negative_flags(reg.wrapping_sub(value));
    }

    // Shifts and Rotates
    fn asl_acc(&mut self) {
        let mut value = self.a;
        if (value >> 7) != 0 {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        value = value << 1;
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn asl(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        if (old_value >> 7) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        let new_value = old_value << 1;
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
    }

    fn lsr_acc(&mut self) {
        let mut value = self.a;
        if (value & 0x01) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        value = value >> 1;
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn lsr(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        if (old_value & 0x01) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        let new_value = old_value >> 1;
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
    }

    fn rol_acc(&mut self) {
        let mut value = self.a;
        let old_carry = self.st & 0x01;
        if (value >> 7) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        value = (value << 1) | old_carry;
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn rol(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        let old_carry = self.st & 0x01;
        if (old_value >> 7) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        let new_value = (old_value << 1) | old_carry;
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
    }

    fn ror_acc(&mut self) {
        let mut value = self.a;
        let old_carry = (self.st & 0x01) << 7;
        if (value & 0x01) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        value = (value >> 1) | old_carry;
        self.a = value;
        self.update_zero_negative_flags(self.a);
    }

    fn ror(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_value = bus.read(addr);
        let old_carry = (self.st & 0x01) << 7;
        if (old_value & 0x01) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        let new_value = (old_value >> 1) | old_carry;
        bus.write(addr, old_value);
        bus.write(addr, new_value);
        self.update_zero_negative_flags(new_value);
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

    // Arithmetic
    fn adc(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);

        let carry_in = if (self.st & 0x01) != 0 { 1 } else { 0 };
        let sum = (self.a as u16) + (value as u16) + carry_in;

        // Carry flag
        if sum > 0xFF {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }

        // Overflow flag
        // ~(A ^ M) & (A ^ R) & 0x80
        let result = sum as u8;
        let overflow = (!(self.a ^ value) & (self.a ^ result)) & 0x80;
        if overflow != 0 {
            self.st |= 0x40;
        } else {
            self.st &= !0x40;
        }

        self.a = result;
        self.update_zero_negative_flags(self.a);
    }

    fn sbc(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let value = bus.read(addr);

        // SBC is ADC with inverted value
        let value = value ^ 0xFF; // Invert bits

        let carry_in = if (self.st & 0x01) != 0 { 1 } else { 0 };
        let sum = (self.a as u16) + (value as u16) + carry_in;

        if sum > 0xFF {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }

        let result = sum as u8;
        let overflow = (!(self.a ^ value) & (self.a ^ result)) & 0x80;
        if overflow != 0 {
            self.st |= 0x40;
        } else {
            self.st &= !0x40;
        }

        self.a = result;
        self.update_zero_negative_flags(self.a);
    }

    // Control Flow
    fn jmp(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        self.pc = addr;
    }

    fn jsr(&mut self, bus: &mut Bus) {
        let sub_addr = self.fetch_word(bus);

        // Push PC + 2 - 1 (which is PC - 1 since fetch_word advanced pc by 2, wait)
        // fetch_word advances PC by 2.
        // Current PC points to next instruction.
        // We want to push the address of the 3rd byte of JSR instruction.
        // Original PC -> JSR opcode
        // PC+1 -> sub_addr LO
        // PC+2 -> sub_addr HI
        // After fetch_word, PC points to next instruction (PC+3).
        // So we push PC - 1.

        let ret_addr = self.pc.wrapping_sub(1);
        self.push(bus, (ret_addr >> 8) as u8);
        self.push(bus, (ret_addr & 0xFF) as u8);

        self.pc = sub_addr;
    }

    fn rts(&mut self, bus: &mut Bus) {
        let lo = self.pop(bus) as u16;
        let hi = self.pop(bus) as u16;
        let ret_addr = lo | (hi << 8);
        self.pc = ret_addr.wrapping_add(1);
    }

    // System
    fn brk(&mut self, bus: &mut Bus) {
        if Self::irq_log_enabled() {
            #[cfg(not(target_arch = "wasm32"))]
            println!(
                "[IRQ] BRK at PC=${:04X} mapper={} ctrl=${:02X} prg=${:02X}",
                self.pc, bus.mapper, bus.mmc1_control, bus.mmc1_prg_bank
            );
        }

        // Push PC + 2 (BRK is 2 bytes, but usually padding byte is skipped by interrupt handler?)
        // 6502 BRK pushes PC+2.
        let pc = self.pc.wrapping_add(1);
        self.push(bus, (pc >> 8) as u8);
        self.push(bus, (pc & 0xFF) as u8);
        self.push(bus, self.st | 0x10 | 0x20); // Break flag set, bit 5 set
        self.st |= 0x04; // Set Interrupt Disable

        // Load vector from 0xFFFE
        let lo = bus.read(0xFFFE) as u16;
        let hi = bus.read(0xFFFF) as u16;
        self.pc = lo | (hi << 8);
    }

    pub fn nmi(&mut self, bus: &mut Bus) {
        if Self::irq_log_enabled() {
            #[cfg(not(target_arch = "wasm32"))]
            println!("[IRQ] NMI at PC=${:04X}", self.pc);
        }

        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        self.push(bus, self.st & !0x10 | 0x20); // Break flag clear, bit 5 set
        self.st |= 0x04; // Set Interrupt Disable

        // Load vector from 0xFFFA
        let lo = bus.read(0xFFFA) as u16;
        let hi = bus.read(0xFFFB) as u16;
        self.pc = lo | (hi << 8);
    }

    pub fn irq(&mut self, bus: &mut Bus) {
        // IRQ is maskable: only fires when I flag is clear
        if (self.st & 0x04) != 0 {
            return;
        }
        if Self::irq_log_enabled() {
            #[cfg(not(target_arch = "wasm32"))]
            println!("[IRQ] IRQ at PC=${:04X}", self.pc);
        }
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        self.push(bus, self.st & !0x10 | 0x20); // Break flag clear, bit 5 set
        self.st |= 0x04; // Set Interrupt Disable

        // Load vector from 0xFFFE (same as BRK)
        let lo = bus.read(0xFFFE) as u16;
        let hi = bus.read(0xFFFF) as u16;
        self.pc = lo | (hi << 8);
    }

    fn rti(&mut self, bus: &mut Bus) {
        self.plp(bus);
        let lo = self.pop(bus) as u16;
        let hi = self.pop(bus) as u16;
        self.pc = lo | (hi << 8);
    }
    // Unofficial Opcode Implementations

    fn slo(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // ASL part
        if (old_data & 0x80) != 0 {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        let new_data = old_data << 1;
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // ORA part
        self.a |= new_data;
        self.update_zero_negative_flags(self.a);
    }

    fn rla(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // ROL part
        let carry_in = if (self.st & 0x01) != 0 { 1 } else { 0 };
        if (old_data & 0x80) != 0 {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        let new_data = (old_data << 1) | carry_in;
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // AND part
        self.a &= new_data;
        self.update_zero_negative_flags(self.a);
    }

    fn sre(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // LSR part
        if (old_data & 0x01) != 0 {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        let new_data = old_data >> 1;
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // EOR part
        self.a ^= new_data;
        self.update_zero_negative_flags(self.a);
    }

    fn rra(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // ROR part
        let carry_in = if (self.st & 0x01) != 0 { 0x80 } else { 0 };
        if (old_data & 0x01) != 0 {
            self.st |= 0x01; // Carry
        } else {
            self.st &= !0x01;
        }
        let new_data = (old_data >> 1) | carry_in;
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // ADC part
        self.add_to_accumulator(new_data);
    }

    fn sax(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let data = self.a & self.x;
        bus.write(addr, data);
    }

    fn lax(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let data = bus.read(addr);
        self.a = data;
        self.x = data;
        self.update_zero_negative_flags(self.a);
    }

    fn dcp(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // DEC part
        let new_data = old_data.wrapping_sub(1);
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // CMP part
        self.compare(self.a, new_data);
    }

    fn isc(&mut self, bus: &mut Bus, mode: AddressingMode) {
        let addr = self.get_operand_address(bus, &mode);
        let old_data = bus.read(addr);

        // INC part
        let new_data = old_data.wrapping_add(1);
        bus.write(addr, old_data);
        bus.write(addr, new_data);

        // SBC part
        let value = new_data ^ 0xFF;
        let carry_in = if (self.st & 0x01) != 0 { 1 } else { 0 };
        let sum = (self.a as u16) + (value as u16) + carry_in;

        if sum > 0xFF {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }

        let result = sum as u8;
        let overflow = (!(self.a ^ value) & (self.a ^ result)) & 0x80;
        if overflow != 0 {
            self.st |= 0x40;
        } else {
            self.st &= !0x40;
        }

        self.a = result;
        self.update_zero_negative_flags(self.a);
    }

    fn alr(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::Immediate);
        let data = bus.read(addr);
        self.a &= data;

        if (self.a & 0x01) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        self.a >>= 1;
        self.update_zero_negative_flags(self.a);
    }

    fn anc(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::Immediate);
        let data = bus.read(addr);
        self.a &= data;
        self.update_zero_negative_flags(self.a);
        if (self.st & 0x80) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
    }

    fn arr(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::Immediate);
        let data = bus.read(addr);
        self.a &= data;

        let carry_in = if (self.st & 0x01) != 0 { 0x80 } else { 0 };
        let result = (self.a >> 1) | carry_in;

        // ARR has unique flag updates
        // Carry is bit 6 of result
        if (result & 0x40) != 0 {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }

        // Overflow is XOR of bit 6 and bit 5
        let bit6 = (result >> 6) & 0x01;
        let bit5 = (result >> 5) & 0x01;
        if (bit6 ^ bit5) != 0 {
            self.st |= 0x40;
        } else {
            self.st &= !0x40;
        }

        self.a = result;
        self.update_zero_negative_flags(self.a);
    }

    fn axs(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::Immediate);
        let data = bus.read(addr);
        let val = self.a & self.x;
        let result = val.wrapping_sub(data);

        if val >= data {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }
        self.x = result;
        self.update_zero_negative_flags(self.x);
    }

    fn las(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::AbsoluteY);
        let data = bus.read(addr);
        let result = data & self.sp;
        self.a = result;
        self.x = result;
        self.sp = result;
        self.update_zero_negative_flags(result);
    }

    fn xaa(&mut self, bus: &mut Bus) {
        let addr = self.get_operand_address(bus, &AddressingMode::Immediate);
        let data = bus.read(addr);
        self.a = self.x & data;
        self.update_zero_negative_flags(self.a);
    }

    fn ahx(&mut self, bus: &mut Bus, mode: &AddressingMode) {
        // AHX AbsoluteY (0x9F) and IndirectY (0x93)
        // Store (A & X & (H + 1))
        // This is a complex opcode with some variation in behavior.
        // We'll use a common implementation for nestest compatibility.
        // First we need the address, but AHX needs the HIGH byte of the address.
        // Since get_operand_address already advanced PC, we have to follow its logic carefully.

        match mode {
            AddressingMode::AbsoluteY => {
                let base = self.fetch_word(bus);
                let hi = (base >> 8) as u8;
                let final_addr = base.wrapping_add(self.y as u16);
                let val = self.a & self.x & hi.wrapping_add(1);
                bus.write(final_addr, val);
            }
            AddressingMode::IndirectY => {
                let ptr = self.fetch_byte(bus);
                let lo = bus.read(ptr as u16) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u16) as u16;
                let base = lo | (hi << 8);
                let final_addr = base.wrapping_add(self.y as u16);
                let val = self.a & self.x & (hi as u8).wrapping_add(1);
                bus.write(final_addr, val);
            }
            _ => {}
        }
    }

    fn shy(&mut self, bus: &mut Bus) {
        // SHY AbsoluteX (0x9C)
        // Store (Y & (H + 1))
        let base = self.fetch_word(bus);
        let hi = (base >> 8) as u8;
        let final_addr = base.wrapping_add(self.x as u16);
        let val = self.y & hi.wrapping_add(1);
        // Note: SHY can be unstable if page boundary is crossed.
        // For simplicity, we implement the basic logic.
        bus.write(final_addr, val);
    }

    fn shx(&mut self, bus: &mut Bus) {
        // SHX AbsoluteY (0x9E)
        // Store (X & (H + 1))
        let base = self.fetch_word(bus);
        let hi = (base >> 8) as u8;
        let final_addr = base.wrapping_add(self.y as u16);
        let val = self.x & hi.wrapping_add(1);
        bus.write(final_addr, val);
    }

    fn tas(&mut self, bus: &mut Bus) {
        // TAS AbsoluteY (0x9B)
        // S = A & X, Store (S & (H + 1))
        self.sp = self.a & self.x;
        let base = self.fetch_word(bus);
        let hi = (base >> 8) as u8;
        let final_addr = base.wrapping_add(self.y as u16);
        let val = self.sp & hi.wrapping_add(1);
        bus.write(final_addr, val);
    }

    fn add_to_accumulator(&mut self, value: u8) {
        let carry_in = if (self.st & 0x01) != 0 { 1 } else { 0 };
        let sum = (self.a as u16) + (value as u16) + carry_in;

        if sum > 0xFF {
            self.st |= 0x01;
        } else {
            self.st &= !0x01;
        }

        let result = sum as u8;
        let overflow = (!(self.a ^ value) & (self.a ^ result)) & 0x80;
        if overflow != 0 {
            self.st |= 0x40;
        } else {
            self.st &= !0x40;
        }

        self.a = result;
        self.update_zero_negative_flags(self.a);
    }

    fn get_operand_address(&mut self, bus: &mut Bus, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            AddressingMode::ZeroPage => self.fetch_byte(bus) as u16,
            AddressingMode::ZeroPageX => {
                let pos = self.fetch_byte(bus);
                let addr = pos.wrapping_add(self.x) as u16;
                addr
            }
            AddressingMode::ZeroPageY => {
                let pos = self.fetch_byte(bus);
                let addr = pos.wrapping_add(self.y) as u16;
                addr
            }
            AddressingMode::Absolute => self.fetch_word(bus),
            AddressingMode::AbsoluteX => {
                let base = self.fetch_word(bus);
                let addr = base.wrapping_add(self.x as u16);
                if (base & 0xFF00) != (addr & 0xFF00) {
                    self.page_crossed = true;
                }
                addr
            }
            AddressingMode::AbsoluteY => {
                let base = self.fetch_word(bus);
                let addr = base.wrapping_add(self.y as u16);
                if (base & 0xFF00) != (addr & 0xFF00) {
                    self.page_crossed = true;
                }
                addr
            }
            AddressingMode::IndirectX => {
                let base = self.fetch_byte(bus);
                let ptr = base.wrapping_add(self.x);
                let lo = bus.read(ptr as u16) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u16) as u16;
                lo | (hi << 8)
            }
            AddressingMode::IndirectY => {
                let base = self.fetch_byte(bus);
                let lo = bus.read(base as u16) as u16;
                let hi = bus.read(base.wrapping_add(1) as u16) as u16;
                let deref_base = lo | (hi << 8);
                let addr = deref_base.wrapping_add(self.y as u16);
                if (deref_base & 0xFF00) != (addr & 0xFF00) {
                    self.page_crossed = true;
                }
                addr
            }
            AddressingMode::Indirect => {
                // JMP Indirect
                let addr = self.fetch_word(bus);
                let lo = bus.read(addr) as u16;

                // Hardware bug: if addr low byte is 0xFF, hi byte is read from XX00, not XX00+100
                let hi_addr = if (addr & 0x00FF) == 0x00FF {
                    addr & 0xFF00
                } else {
                    addr.wrapping_add(1)
                };

                let hi = bus.read(hi_addr) as u16;
                lo | (hi << 8)
            }
            AddressingMode::NoneAddressing => 0,
            AddressingMode::Accumulator => 0,
        }
    }

    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch_byte(bus) as u16;
        let hi = self.fetch_byte(bus) as u16;
        lo | (hi << 8)
    }

    pub fn get_absolute_address(&mut self, bus: &mut Bus, mode: &AddressingMode, addr: u16) -> u16 {
        match mode {
            AddressingMode::ZeroPage => bus.read(addr) as u16,
            AddressingMode::Accumulator => 0,
            AddressingMode::Absolute => {
                let lo = bus.read(addr) as u16;
                let hi = bus.read(addr + 1) as u16;
                lo | (hi << 8)
            }
            AddressingMode::ZeroPageX => {
                let pos = bus.read(addr);
                pos.wrapping_add(self.x) as u16
            }
            AddressingMode::ZeroPageY => {
                let pos = bus.read(addr);
                pos.wrapping_add(self.y) as u16
            }
            AddressingMode::AbsoluteX => {
                let lo = bus.read(addr) as u16;
                let hi = bus.read(addr + 1) as u16;
                let base = lo | (hi << 8);
                base.wrapping_add(self.x as u16)
            }
            AddressingMode::AbsoluteY => {
                let lo = bus.read(addr) as u16;
                let hi = bus.read(addr + 1) as u16;
                let base = lo | (hi << 8);
                base.wrapping_add(self.y as u16)
            }
            AddressingMode::IndirectX => {
                let base = bus.read(addr);
                let ptr = base.wrapping_add(self.x);
                let lo = bus.read(ptr as u16) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u16) as u16;
                lo | (hi << 8)
            }
            AddressingMode::IndirectY => {
                let base = bus.read(addr);
                let lo = bus.read(base as u16) as u16;
                let hi = bus.read(base.wrapping_add(1) as u16) as u16;
                let deref_base = lo | (hi << 8);
                deref_base.wrapping_add(self.y as u16)
            }
            _ => 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
    Indirect, // For JMP
    Accumulator,
    NoneAddressing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;
    use crate::cartridge::Mirroring;
    use crate::ppu::Ppu;

    fn create_bus() -> Bus {
        let ppu = Ppu::new(Mirroring::Horizontal, vec![0; 2048]);
        let rom = vec![0; 0x8000]; // Dummy 32KB ROM
        Bus::new(ppu, rom, 0, 8192, false)
    }

    #[test]
    fn test_0xa9_lda_immediate_load_data() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // Load program: LDA #0x05
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0x05);
        bus.write(0x0002, 0x00); // BRK

        // Set PC to 0
        cpu.pc = 0;

        // Execute instructions
        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x05);
        assert!(cpu.st & 0x02 == 0); // Zero flag should be clear
        assert!(cpu.st & 0x80 == 0); // Negative flag should be clear
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #0x00
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0x00);

        cpu.pc = 0;
        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x00);
        assert!(cpu.st & 0x02 != 0); // Zero flag should be set
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #10, TAX
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0x0A);
        bus.write(0x0002, 0xAA);

        cpu.pc = 0;
        cpu.step(&mut bus); // LDA
        cpu.step(&mut bus); // TAX

        assert_eq!(cpu.x, 0x0A);
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #0xC0
        // TAX
        // INX
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0xC0);
        bus.write(0x0002, 0xAA);
        bus.write(0x0003, 0xE8);

        cpu.pc = 0;
        cpu.step(&mut bus); // LDA
        cpu.step(&mut bus); // TAX
        cpu.step(&mut bus); // INX

        assert_eq!(cpu.x, 0xC1);
    }

    #[test]
    fn test_inx_overflow() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #0xFF
        // TAX
        // INX
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0xFF);
        bus.write(0x0002, 0xAA);
        bus.write(0x0003, 0xE8); // Overflow to 0x00 via registers

        cpu.pc = 0;
        cpu.step(&mut bus); // LDA
        cpu.step(&mut bus); // TAX
        cpu.step(&mut bus); // INX (0xFF -> 0x00)

        assert_eq!(cpu.x, 0x00);
        assert!(cpu.st & 0x02 != 0); // Zero flag set

        // INX again -> 0x01
        bus.write(0x0004, 0xE8);
        cpu.step(&mut bus);
        assert_eq!(cpu.x, 0x01);
        assert!(cpu.st & 0x02 == 0); // Zero flag clear
    }

    #[test]
    fn test_adc_no_carry() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #10
        // ADC #20
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0x0A);
        bus.write(0x0002, 0x69);
        bus.write(0x0003, 0x14);

        cpu.pc = 0;
        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.a, 30);
        assert!(cpu.st & 0x01 == 0); // Carry clear
    }

    #[test]
    fn test_adc_carry() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();

        // LDA #0xFF
        // ADC #0x01
        bus.write(0x0000, 0xA9);
        bus.write(0x0001, 0xFF);
        bus.write(0x0002, 0x69);
        bus.write(0x0003, 0x01);

        cpu.pc = 0;
        cpu.step(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x00);

        // Check flags: result is 0, so Zero flag should be set. Carry should be set.
        assert_eq!(cpu.st & 0x02, 0x02, "Zero flag should be set");
        assert_eq!(cpu.st & 0x01, 0x01, "Carry flag should be set");
    }

    #[test]
    fn test_alr() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();
        // A = 0xFF, ALR #0xFF -> A = 0x7F, C = 1
        cpu.a = 0xFF;
        bus.write(0, 0x4B);
        bus.write(1, 0xFF);
        cpu.pc = 0;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x7F);
        assert!(cpu.st & 0x01 != 0); // Carry set
    }

    #[test]
    fn test_anc() {
        let mut bus = create_bus();
        let mut cpu = Cpu::new();
        // A = 0x80, ANC #0x80 -> A = 0x80, C = 1 (since bit 7 is 1)
        cpu.a = 0x80;
        bus.write(0, 0x0B);
        bus.write(1, 0x80);
        cpu.pc = 0;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x80);
        assert!(cpu.st & 0x01 != 0); // Carry set from Neg flag
    }
}
