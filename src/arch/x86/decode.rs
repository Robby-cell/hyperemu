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

    fn read_u32(&mut self) -> u32 {
        let val = u32::from_le_bytes(self.data[self.ptr..self.ptr + 4].try_into().unwrap());
        self.ptr += 4;
        val
    }

    fn map_reg(&self, r: u8) -> GpReg {
        match r & 0x7 {
            0 => GpReg::Eax,
            1 => GpReg::Ecx,
            2 => GpReg::Edx,
            3 => GpReg::Ebx,
            4 => GpReg::Esp,
            5 => GpReg::Ebp,
            6 => GpReg::Esi,
            7 => GpReg::Edi,
            _ => unreachable!(),
        }
    }

    fn decode_modrm(&mut self) -> (Operand, u8) {
        let modrm = self.read_u8();
        let mode = (modrm >> 6) & 0b11;
        let reg_op = (modrm >> 3) & 0b111;
        let rm = modrm & 0b111;

        if mode == 0b11 {
            return (Operand::Reg(self.map_reg(rm)), reg_op);
        }

        let mut addr = MemoryAddr {
            base: None,
            index: None,
            scale: 1,
            disp: 0,
        };

        match rm {
            4 => {
                // SIB Byte
                let sib = self.read_u8();
                let scale = 1 << ((sib >> 6) & 0b11);
                let index = (sib >> 3) & 0b111;
                let base = sib & 0b111;

                if index != 4 {
                    addr.index = Some(self.map_reg(index));
                }
                addr.scale = scale;

                match mode {
                    0 if base == 5 => addr.disp = self.read_u32() as i32,
                    0 => addr.base = Some(self.map_reg(base)),
                    1 => {
                        addr.base = Some(self.map_reg(base));
                        addr.disp = self.read_u8() as i8 as i32;
                    }
                    2 => {
                        addr.base = Some(self.map_reg(base));
                        addr.disp = self.read_u32() as i32;
                    }
                    _ => unreachable!(),
                }
            }
            5 if mode == 0 => addr.disp = self.read_u32() as i32,
            _ => {
                addr.base = Some(self.map_reg(rm));
                if mode == 1 {
                    addr.disp = self.read_u8() as i8 as i32;
                } else if mode == 2 {
                    addr.disp = self.read_u32() as i32;
                }
            }
        }

        (Operand::Mem(addr), reg_op)
    }

    /// Maps the binary layout of x86 math opcodes to the AST
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
        let opcode = self.read_u8();
        match opcode {
            0x90 => Instr::Nop,

            // Opcode EAX, imm32 (e.g. 0x05 is ADD EAX, imm32)
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let val = self.read_u32();
                self.build_alu(opcode, Operand::Reg(GpReg::Eax), Operand::Imm32(val))
            }

            // Opcode r/m32, r32 (e.g. 0x01 is ADD r/m32, r32)
            0x01 | 0x09 | 0x11 | 0x19 | 0x21 | 0x29 | 0x31 | 0x39 => {
                let (dest, reg) = self.decode_modrm();
                self.build_alu(opcode, dest, Operand::Reg(self.map_reg(reg)))
            }

            // Opcode r32, r/m32 (e.g. 0x03 is ADD r32, r/m32)
            0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B => {
                let (src, reg) = self.decode_modrm();
                self.build_alu(opcode, Operand::Reg(self.map_reg(reg)), src)
            }

            // Group 1: Math r/m32, imm32/imm8 (e.g. 0x81 0xC3 0x05 is ADD EBX, 5)
            0x81 | 0x83 => {
                let (dest, reg_op) = self.decode_modrm();
                let imm = if opcode == 0x83 {
                    self.read_u8() as i8 as i32 as u32
                } else {
                    self.read_u32()
                };
                self.build_alu(reg_op << 3, dest, Operand::Imm32(imm))
            }

            0x85 => {
                // TEST r/m32, r32
                let (dest, reg) = self.decode_modrm();
                Instr::Test {
                    dest,
                    src: Operand::Reg(self.map_reg(reg)),
                }
            }
            0xA9 => {
                // TEST EAX, imm32
                Instr::Test {
                    dest: Operand::Reg(GpReg::Eax),
                    src: Operand::Imm32(self.read_u32()),
                }
            }

            0x0F => {
                let op2 = self.read_u8();
                match op2 {
                    0x80..=0x8F => {
                        // Jcc rel32 (Critical for long loops)
                        let cond = unsafe { std::mem::transmute(op2 - 0x80) };
                        Instr::Jcc(cond, self.read_u32() as i32)
                    }
                    0xAF => {
                        // IMUL r32, r/m32
                        let (src, reg) = self.decode_modrm();
                        Instr::Imul(Operand::Reg(self.map_reg(reg)), src)
                    }
                    0xB6 => {
                        // MOVZX r32, r/m8
                        let (src, reg) = self.decode_modrm();
                        Instr::Movzx8 {
                            dest: Operand::Reg(self.map_reg(reg)),
                            src,
                        }
                    }
                    0xBE => {
                        // MOVSX r32, r/m8
                        let (src, reg) = self.decode_modrm();
                        Instr::Movsx8 {
                            dest: Operand::Reg(self.map_reg(reg)),
                            src,
                        }
                    }
                    _ => Instr::Unknown(0x0F),
                }
            }

            // Group 2: Shifts (0xC1 = r/m32, imm8 | 0xD3 = r/m32, CL)
            0xC1 | 0xD3 => {
                let (dest, reg_op) = self.decode_modrm();
                let count = if opcode == 0xC1 {
                    Operand::Imm8(self.read_u8())
                } else {
                    Operand::Reg(GpReg::Ecx) // Hardware masks ECX to just CL
                };
                match reg_op {
                    4 | 6 => Instr::Shl { dest, count }, // SAL/SHL
                    5 => Instr::Shr { dest, count },
                    7 => Instr::Sar { dest, count },
                    _ => Instr::Unknown(opcode),
                }
            }

            // Group 3 Extensions (0xF7)
            0xF7 => {
                let (dest, reg_op) = self.decode_modrm();
                match reg_op {
                    0 | 1 => Instr::Test {
                        dest,
                        src: Operand::Imm32(self.read_u32()),
                    },
                    2 => Instr::Not(dest),
                    3 => Instr::Neg(dest),
                    4 => Instr::Mul(dest),
                    6 => Instr::Div(dest),
                    _ => Instr::Unknown(opcode),
                }
            }

            0x40..=0x47 => Instr::Inc(Operand::Reg(self.map_reg(opcode - 0x40))),
            0x48..=0x4F => Instr::Dec(Operand::Reg(self.map_reg(opcode - 0x48))),

            0x50..=0x57 => Instr::Push(Operand::Reg(self.map_reg(opcode - 0x50))),
            0x58..=0x5F => Instr::Pop(Operand::Reg(self.map_reg(opcode - 0x58))),

            0x89 => {
                let (dest, reg) = self.decode_modrm();
                Instr::Mov {
                    dest,
                    src: Operand::Reg(self.map_reg(reg)),
                }
            }
            0x8B => {
                let (src, reg) = self.decode_modrm();
                Instr::Mov {
                    dest: Operand::Reg(self.map_reg(reg)),
                    src,
                }
            }
            0x8D => {
                // LEA
                let (src, reg) = self.decode_modrm();
                match src {
                    Operand::Mem(addr) => Instr::Lea {
                        dest: self.map_reg(reg),
                        src: addr,
                    },
                    _ => Instr::Unknown(opcode),
                }
            }
            0xB8..=0xBF => {
                let reg = self.map_reg(opcode - 0xB8);
                Instr::Mov {
                    dest: Operand::Reg(reg),
                    src: Operand::Imm32(self.read_u32()),
                }
            }
            0xC7 => {
                let (dest, _) = self.decode_modrm();
                Instr::Mov {
                    dest,
                    src: Operand::Imm32(self.read_u32()),
                }
            }

            0xE8 => Instr::Call(self.read_u32() as i32),
            0xC3 => Instr::Ret,
            0xC9 => Instr::Leave,
            0xCD => Instr::Int(self.read_u8()),

            0x70..=0x7F => {
                let cond = unsafe { std::mem::transmute(opcode - 0x70) };
                Instr::Jcc(cond, self.read_u8() as i8 as i32)
            }
            0xEB => Instr::Jmp(self.read_u8() as i8 as i32),
            0xE9 => Instr::Jmp(self.read_u32() as i32),

            _ => Instr::Unknown(opcode),
        }
    }
}
