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

    pub fn decode_instr(&mut self) -> Instr {
        let opcode = self.read_u8();
        match opcode {
            0x90 => Instr::Nop,
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
            0xB8..=0xBF => {
                let reg = self.map_reg(opcode - 0xB8);
                Instr::Mov {
                    dest: Operand::Reg(reg),
                    src: Operand::Imm32(self.read_u32()),
                }
            }
            0x31 => {
                let (dest, reg) = self.decode_modrm();
                Instr::Xor {
                    dest,
                    src: Operand::Reg(self.map_reg(reg)),
                }
            }
            0xE8 => Instr::Call(self.read_u32() as i32),
            0xC3 => Instr::Ret,
            0xCD => Instr::Int(self.read_u8()),
            0x70..=0x7F => {
                let cond = unsafe { std::mem::transmute(opcode - 0x70) };
                Instr::Jcc(cond, self.read_u8() as i8 as i32)
            }
            _ => Instr::Unknown(opcode),
        }
    }
}
