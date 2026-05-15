use super::instr::*;

pub struct X86Decoder<'a> {
    data: &'a [u8],
    ptr: usize,
}

impl<'a> X86Decoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, ptr: 0 }
    }
    pub fn consumed(&self) -> usize {
        self.ptr
    }
    fn read_u8(&mut self) -> u8 {
        let b = self.data[self.ptr];
        self.ptr += 1;
        b
    }

    fn read_u16(&mut self) -> u16 {
        let val = u16::from_le_bytes(self.data[self.ptr..self.ptr + 2].try_into().unwrap());
        self.ptr += 2;
        val
    }

    fn read_u32(&mut self) -> u32 {
        let val = u32::from_le_bytes(self.data[self.ptr..self.ptr + 4].try_into().unwrap());
        self.ptr += 4;
        val
    }

    fn map_reg(&self, r: u8, size: OpSize) -> Operand {
        let clean = r & 7;
        match size {
            OpSize::Byte => Operand::Reg8(unsafe { std::mem::transmute(clean) }),
            OpSize::Word => Operand::Reg16(unsafe { std::mem::transmute(clean) }),
            OpSize::Dword => Operand::Reg32(unsafe { std::mem::transmute(clean) }),
        }
    }

    fn map_reg32(&self, r: u8) -> GpReg32 {
        unsafe { std::mem::transmute(r & 7) }
    }

    fn decode_modrm(&mut self, size: OpSize) -> (Operand, u8) {
        let modrm = self.read_u8();
        let mode = (modrm >> 6) & 0b11;
        let reg_op = (modrm >> 3) & 0b111;
        let rm = modrm & 0b111;

        if mode == 0b11 {
            return (self.map_reg(rm, size), reg_op);
        }

        let mut addr = MemoryAddr {
            base: None,
            index: None,
            scale: 1,
            disp: 0,
        };

        match rm {
            4 => {
                // SIB
                let sib = self.read_u8();
                let scale = 1 << ((sib >> 6) & 0b11);
                let index = (sib >> 3) & 0b111;
                let base = sib & 0b111;

                if index != 4 {
                    addr.index = Some(self.map_reg32(index));
                }
                addr.scale = scale;

                match mode {
                    0 if base == 5 => addr.disp = self.read_u32() as i32,
                    0 => addr.base = Some(self.map_reg32(base)),
                    1 => {
                        addr.base = Some(self.map_reg32(base));
                        addr.disp = self.read_u8() as i8 as i32;
                    }
                    2 => {
                        addr.base = Some(self.map_reg32(base));
                        addr.disp = self.read_u32() as i32;
                    }
                    _ => unreachable!(),
                }
            }
            5 if mode == 0 => addr.disp = self.read_u32() as i32,
            _ => {
                addr.base = Some(self.map_reg32(rm));
                if mode == 1 {
                    addr.disp = self.read_u8() as i8 as i32;
                } else if mode == 2 {
                    addr.disp = self.read_u32() as i32;
                }
            }
        }

        let op = match size {
            OpSize::Byte => Operand::Mem8(addr),
            OpSize::Word => Operand::Mem16(addr),
            OpSize::Dword => Operand::Mem32(addr),
        };
        (op, reg_op)
    }

    fn build_alu(&self, op_code: u8, dest: Operand, src: Operand) -> Instr {
        match op_code & 0xF8 {
            0x00 => Instr::Add { dest, src },
            0x08 => Instr::Or { dest, src },
            0x10 => Instr::Adc { dest, src },
            0x18 => Instr::Sbb { dest, src },
            0x20 => Instr::And { dest, src },
            0x28 => Instr::Sub { dest, src },
            0x30 => Instr::Xor { dest, src },
            0x38 => Instr::Cmp { dest, src },
            _ => unreachable!(),
        }
    }

    pub fn decode_instr(&mut self) -> Instr {
        let mut has_66 = false;
        let mut opcode = self.read_u8();

        while opcode == 0x66 {
            has_66 = true;
            opcode = self.read_u8();
        }

        match opcode {
            0x90 => Instr::Nop,

            0x0F => {
                let op2 = self.read_u8();
                let size = if has_66 { OpSize::Word } else { OpSize::Dword };
                match op2 {
                    0x80..=0x8F => Instr::Jcc(
                        unsafe { std::mem::transmute(op2 - 0x80) },
                        self.read_u32() as i32,
                    ),
                    0xAF => {
                        let (src, reg) = self.decode_modrm(size);
                        Instr::Imul(self.map_reg(reg, size), src)
                    }
                    0xB6 => {
                        let (src, reg) = self.decode_modrm(OpSize::Byte);
                        Instr::Movzx8 {
                            dest: self.map_reg(reg, size),
                            src,
                        }
                    }
                    0xBE => {
                        let (src, reg) = self.decode_modrm(OpSize::Byte);
                        Instr::Movsx8 {
                            dest: self.map_reg(reg, size),
                            src,
                        }
                    }
                    _ => Instr::Unknown(0x0F),
                }
            }

            0x00..=0x3F => {
                let op_base = opcode & 0xF8;
                let is_8bit = (opcode & 1) == 0;
                let size = if is_8bit {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };

                match opcode & 7 {
                    0 | 1 => {
                        let (dest, reg) = self.decode_modrm(size);
                        self.build_alu(op_base, dest, self.map_reg(reg, size))
                    }
                    2 | 3 => {
                        let (src, reg) = self.decode_modrm(size);
                        self.build_alu(op_base, self.map_reg(reg, size), src)
                    }
                    4 => {
                        let val = self.read_u8();
                        self.build_alu(op_base, Operand::Reg8(GpReg8::Al), Operand::Imm8(val))
                    }
                    5 => {
                        let src = if size == OpSize::Word {
                            Operand::Imm16(self.read_u16())
                        } else {
                            Operand::Imm32(self.read_u32())
                        };
                        self.build_alu(op_base, self.map_reg(0, size), src)
                    }
                    _ => Instr::Unknown(opcode),
                }
            }

            0x40..=0x47 => Instr::Inc(self.map_reg(
                opcode - 0x40,
                if has_66 { OpSize::Word } else { OpSize::Dword },
            )),
            0x48..=0x4F => Instr::Dec(self.map_reg(
                opcode - 0x48,
                if has_66 { OpSize::Word } else { OpSize::Dword },
            )),

            0x50..=0x57 => Instr::Push(self.map_reg(
                opcode - 0x50,
                if has_66 { OpSize::Word } else { OpSize::Dword },
            )),
            0x58..=0x5F => Instr::Pop(self.map_reg(
                opcode - 0x58,
                if has_66 { OpSize::Word } else { OpSize::Dword },
            )),

            0x80..=0x83 => {
                let size = if opcode == 0x80 || opcode == 0x82 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (dest, reg_op) = self.decode_modrm(size);
                let imm = match opcode {
                    0x80 | 0x82 => Operand::Imm8(self.read_u8()),
                    0x81 => {
                        if size == OpSize::Word {
                            Operand::Imm16(self.read_u16())
                        } else {
                            Operand::Imm32(self.read_u32())
                        }
                    }
                    0x83 => {
                        if size == OpSize::Word {
                            Operand::Imm16(self.read_u8() as i8 as i16 as u16)
                        } else {
                            Operand::Imm32(self.read_u8() as i8 as i32 as u32)
                        }
                    }
                    _ => unreachable!(),
                };
                self.build_alu(reg_op << 3, dest, imm)
            }

            // TEST r/m, r
            0x84 | 0x85 => {
                let size = if opcode == 0x84 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (dest, reg) = self.decode_modrm(size);
                Instr::Test {
                    dest,
                    src: self.map_reg(reg, size),
                }
            }

            0x88 | 0x89 => {
                let size = if opcode == 0x88 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (dest, reg) = self.decode_modrm(size);
                Instr::Mov {
                    dest,
                    src: self.map_reg(reg, size),
                }
            }
            0x8A | 0x8B => {
                let size = if opcode == 0x8A {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (src, reg) = self.decode_modrm(size);
                Instr::Mov {
                    dest: self.map_reg(reg, size),
                    src,
                }
            }
            0x8D => {
                let size = if has_66 { OpSize::Word } else { OpSize::Dword };
                let (src, reg) = self.decode_modrm(size);
                match src {
                    Operand::Mem32(addr) | Operand::Mem16(addr) | Operand::Mem8(addr) => {
                        let dest_reg = match self.map_reg(reg, size) {
                            Operand::Reg32(r) => r,
                            Operand::Reg16(r) => unsafe { std::mem::transmute(r as u8) },
                            _ => unreachable!(),
                        };
                        Instr::Lea {
                            dest: dest_reg,
                            src: addr,
                        }
                    }
                    _ => Instr::Unknown(opcode),
                }
            }

            // TEST AL/EAX, imm
            0xA8 | 0xA9 => {
                let size = if opcode == 0xA8 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let src = match size {
                    OpSize::Byte => Operand::Imm8(self.read_u8()),
                    OpSize::Word => Operand::Imm16(self.read_u16()),
                    OpSize::Dword => Operand::Imm32(self.read_u32()),
                };
                // map_reg(0) is AL / AX / EAX
                Instr::Test {
                    dest: self.map_reg(0, size),
                    src,
                }
            }

            0xB8..=0xBF => {
                let size = if has_66 { OpSize::Word } else { OpSize::Dword };
                let reg = self.map_reg(opcode - 0xB8, size);
                let src = if size == OpSize::Word {
                    Operand::Imm16(self.read_u16())
                } else {
                    Operand::Imm32(self.read_u32())
                };
                Instr::Mov { dest: reg, src }
            }

            0xC1 | 0xD3 => {
                let size = if has_66 { OpSize::Word } else { OpSize::Dword };
                let (dest, reg_op) = self.decode_modrm(size);
                let count = if opcode == 0xC1 {
                    Operand::Imm8(self.read_u8())
                } else {
                    Operand::Reg8(GpReg8::Cl)
                };
                match reg_op {
                    4 | 6 => Instr::Shl { dest, count },
                    5 => Instr::Shr { dest, count },
                    7 => Instr::Sar { dest, count },
                    _ => Instr::Unknown(opcode),
                }
            }

            0xC6 | 0xC7 => {
                let size = if opcode == 0xC6 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (dest, _) = self.decode_modrm(size);
                let src = match size {
                    OpSize::Byte => Operand::Imm8(self.read_u8()),
                    OpSize::Word => Operand::Imm16(self.read_u16()),
                    OpSize::Dword => Operand::Imm32(self.read_u32()),
                };
                Instr::Mov { dest, src }
            }

            0xF6 | 0xF7 => {
                let size = if opcode == 0xF6 {
                    OpSize::Byte
                } else if has_66 {
                    OpSize::Word
                } else {
                    OpSize::Dword
                };
                let (dest, reg_op) = self.decode_modrm(size);
                match reg_op {
                    0 | 1 => {
                        let src = match size {
                            OpSize::Byte => Operand::Imm8(self.read_u8()),
                            OpSize::Word => Operand::Imm16(self.read_u16()),
                            OpSize::Dword => Operand::Imm32(self.read_u32()),
                        };
                        Instr::Test { dest, src }
                    }
                    2 => Instr::Not(dest),
                    3 => Instr::Neg(dest),
                    4 => Instr::Mul(dest),
                    6 => Instr::Div(dest),
                    _ => Instr::Unknown(opcode),
                }
            }

            0xE8 => Instr::Call(self.read_u32() as i32),
            0xC3 => Instr::Ret,
            0xC9 => Instr::Leave,
            0xCD => Instr::Int(self.read_u8()),

            0x70..=0x7F => Instr::Jcc(
                unsafe { std::mem::transmute(opcode - 0x70) },
                self.read_u8() as i8 as i32,
            ),
            0xEB => Instr::Jmp(self.read_u8() as i8 as i32),
            0xE9 => Instr::Jmp(self.read_u32() as i32),
            _ => Instr::Unknown(opcode),
        }
    }
}
