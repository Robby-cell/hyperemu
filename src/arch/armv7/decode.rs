use super::instr::*;

pub fn decode_arm(raw: u32) -> Instr {
    let cond = Condition::from_u32(raw >> 28);

    if cond == Condition::Nv {
        return Instr::Unknown(raw);
    }

    // NOP (0x0320F000 encoded precisely)
    if (raw & 0x0FFFFFFF) == 0x0320F000 {
        return Instr::Nop { cond };
    }

    let op_type = (raw >> 25) & 0b111;

    match op_type {
        0b000 => decode_data_proc_or_misc(cond, raw, false),
        0b001 => decode_data_proc_or_misc(cond, raw, true),
        0b010 => decode_load_store(cond, raw, false),
        0b011 => {
            // Media Instructions use op_type 011 with bit 4 set
            if (raw & (1 << 4)) != 0 {
                decode_media(cond, raw)
            } else {
                decode_load_store(cond, raw, true)
            }
        }
        0b100 => decode_load_store_multiple(cond, raw),
        0b101 => decode_branch(cond, raw),
        0b111 => decode_coproc_or_svc(cond, raw),
        _ => Instr::Unknown(raw),
    }
}

fn decode_media(cond: Condition, raw: u32) -> Instr {
    // BFC / BFI
    if (raw & 0x0FF000F0) == 0x07C00010 {
        let rd = ((raw >> 12) & 0xF) as u8;
        let rn = (raw & 0xF) as u8;
        let lsb = (raw >> 7) & 0x1F;
        let msb = (raw >> 16) & 0x1F;
        let width = msb.saturating_sub(lsb) + 1;
        if rn == 15 {
            return Instr::Bfc {
                cond,
                rd,
                lsb,
                width,
            };
        } else {
            return Instr::Bfi {
                cond,
                rd,
                rn,
                lsb,
                width,
            };
        }
    }

    // UBFX / SBFX
    if (raw & 0x0FA00070) == 0x07A00050 {
        let rd = ((raw >> 12) & 0xF) as u8;
        let rn = (raw & 0xF) as u8;
        let lsb = (raw >> 7) & 0x1F;
        let widthm1 = (raw >> 16) & 0x1F;
        let width = widthm1 + 1;
        if (raw & (1 << 22)) != 0 {
            return Instr::Ubfx {
                cond,
                rd,
                rn,
                lsb,
                width,
            };
        } else {
            return Instr::Sbfx {
                cond,
                rd,
                rn,
                lsb,
                width,
            };
        }
    }

    // Byte Reversals
    if (raw & 0x0FF00FF0) == 0x06B00F30 {
        return Instr::Rev {
            cond,
            rd: ((raw >> 12) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }
    if (raw & 0x0FF00FF0) == 0x06B00FB0 {
        return Instr::Rev16 {
            cond,
            rd: ((raw >> 12) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }
    if (raw & 0x0FF00FF0) == 0x06F00FB0 {
        return Instr::Revsh {
            cond,
            rd: ((raw >> 12) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }

    // Sign/Zero Extensions
    let ext_op = (raw >> 20) & 0xFF;
    if ext_op == 0x6A || ext_op == 0x6B || ext_op == 0x6E || ext_op == 0x6F {
        let rd = ((raw >> 12) & 0xF) as u8;
        let rm = (raw & 0xF) as u8;
        let rn = ((raw >> 16) & 0xF) as u8;
        let rot = ((raw >> 10) & 0x3) as u8;
        let rn_opt = if rn == 15 { None } else { Some(rn) };

        match ext_op {
            0x6A => {
                return Instr::Sxtb {
                    cond,
                    rd,
                    rm,
                    rot,
                    rn: rn_opt,
                };
            }
            0x6B => {
                return Instr::Sxth {
                    cond,
                    rd,
                    rm,
                    rot,
                    rn: rn_opt,
                };
            }
            0x6E => {
                return Instr::Uxtb {
                    cond,
                    rd,
                    rm,
                    rot,
                    rn: rn_opt,
                };
            }
            0x6F => {
                return Instr::Uxth {
                    cond,
                    rd,
                    rm,
                    rot,
                    rn: rn_opt,
                };
            }
            _ => unreachable!(),
        }
    }

    Instr::Unknown(raw)
}

fn decode_data_proc_or_misc(cond: Condition, raw: u32, is_immediate_op2: bool) -> Instr {
    let opcode = (raw >> 21) & 0xF;
    let s = (raw >> 20) & 1 == 1;
    let rn = ((raw >> 16) & 0xF) as u8;
    let rd = ((raw >> 12) & 0xF) as u8;

    // Count Leading Zeros
    if (raw & 0x0FFF0FF0) == 0x016F0F10 {
        return Instr::Clz {
            cond,
            rd: ((raw >> 12) & 0xF) as u8,
            rm: (raw & 0xF) as u8,
        };
    }

    // Breakpoint (BKPT)
    if (raw & 0x0FF000F0) == 0x01200070 {
        let imm16 = (((raw >> 4) & 0xFFF0) | (raw & 0xF)) as u16;
        return Instr::Bkpt { imm16 };
    }

    if !is_immediate_op2 {
        if (raw & 0x0FFFFFF0) == 0x012FFF10 {
            return Instr::Bx {
                cond,
                rm: (raw & 0xF) as u8,
            };
        }
        if (raw & 0x0FFFFFF0) == 0x012FFF30 {
            return Instr::Blx {
                cond,
                rm: (raw & 0xF) as u8,
            };
        }
    }

    if is_immediate_op2 {
        // MOVW (0x03000000 base)
        if (raw & 0x0FF00000) == 0x03000000 {
            let imm4 = (raw >> 16) & 0xF;
            let imm12 = raw & 0xFFF;
            let imm16 = ((imm4 << 12) | imm12) as u16;
            return Instr::Movw { cond, rd, imm16 };
        }
        // MOVT (0x03400000 base)
        if (raw & 0x0FF00000) == 0x03400000 {
            let imm4 = (raw >> 16) & 0xF;
            let imm12 = raw & 0xFFF;
            let imm16 = ((imm4 << 12) | imm12) as u16;
            return Instr::Movt { cond, rd, imm16 };
        }
    }

    if !is_immediate_op2 && ((raw & 0x00000090) == 0x00000090) {
        let rm = (raw & 0xF) as u8;
        let rs = ((raw >> 8) & 0xF) as u8;
        let rn_ext = ((raw >> 12) & 0xF) as u8;
        let rd_ext = ((raw >> 16) & 0xF) as u8;

        // 32-bit Multiplies
        if (raw & 0x0F800000) == 0x00000000 {
            let a = (raw >> 21) & 1 == 1;
            if a {
                return Instr::Mla {
                    cond,
                    s,
                    rd: rd_ext,
                    rm,
                    rs,
                    rn: rn_ext,
                };
            } else {
                return Instr::Mul {
                    cond,
                    s,
                    rd: rd_ext,
                    rm,
                    rs,
                };
            }
        }

        // 64-bit Multiplies
        if (raw & 0x0F800000) == 0x00800000 {
            let a = (raw >> 21) & 1 == 1;
            if a {
                return Instr::Umlal {
                    cond,
                    s,
                    rd_lo: rn_ext,
                    rd_hi: rd_ext,
                    rm,
                    rs,
                };
            } else {
                return Instr::Umull {
                    cond,
                    s,
                    rd_lo: rn_ext,
                    rd_hi: rd_ext,
                    rm,
                    rs,
                };
            }
        }
        if (raw & 0x0F800000) == 0x00C00000 {
            let a = (raw >> 21) & 1 == 1;
            if a {
                return Instr::Smlal {
                    cond,
                    s,
                    rd_lo: rn_ext,
                    rd_hi: rd_ext,
                    rm,
                    rs,
                };
            } else {
                return Instr::Smull {
                    cond,
                    s,
                    rd_lo: rn_ext,
                    rd_hi: rd_ext,
                    rm,
                    rs,
                };
            }
        }

        // Extra Load/Stores
        if (raw & 0x0E000000) == 0x00000000 {
            let p = (raw >> 24) & 1 == 1;
            let u = (raw >> 23) & 1 == 1;
            let i = (raw >> 22) & 1 == 1;
            let w = (raw >> 21) & 1 == 1;
            let l = (raw >> 20) & 1 == 1;
            let op = (raw >> 5) & 0b11;

            let offset = if i {
                let imm8 = ((raw >> 4) & 0xF0) | (raw & 0xF);
                Operand2::Immediate {
                    val: imm8,
                    carry_out: None,
                }
            } else {
                Operand2::Register {
                    rm,
                    shift: Shift::Immediate {
                        shift_type: ShiftType::Lsl,
                        amount: 0,
                    },
                }
            };

            match op {
                0b01 => {
                    if l {
                        return Instr::Ldrh {
                            cond,
                            rd,
                            rn,
                            offset,
                            pre: p,
                            writeback: w,
                            up: u,
                        };
                    } else {
                        return Instr::Strh {
                            cond,
                            rd,
                            rn,
                            offset,
                            pre: p,
                            writeback: w,
                            up: u,
                        };
                    }
                }
                0b10 => {
                    if l {
                        return Instr::Ldrsb {
                            cond,
                            rd,
                            rn,
                            offset,
                            pre: p,
                            writeback: w,
                            up: u,
                        };
                    }
                }
                0b11 => {
                    if l {
                        return Instr::Ldrsh {
                            cond,
                            rd,
                            rn,
                            offset,
                            pre: p,
                            writeback: w,
                            up: u,
                        };
                    }
                }
                _ => {}
            }
        }
    }

    if (raw & 0x0FBF0FFF) == 0x010F0000 {
        let use_spsr = (raw >> 22) & 1 == 1;
        return Instr::Mrs { cond, rd, use_spsr };
    }

    if !is_immediate_op2 && (raw & 0x0FB00FF0) == 0x01200000 {
        let use_spsr = (raw >> 22) & 1 == 1;
        let mask = ((raw >> 16) & 0xF) as u8;
        let rm = (raw & 0xF) as u8;
        let op2 = Operand2::Register {
            rm,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsl,
                amount: 0,
            },
        };
        return Instr::Msr {
            cond,
            use_spsr,
            mask,
            op2,
        };
    } else if is_immediate_op2 && (raw & 0x0FB00000) == 0x03200000 {
        let use_spsr = (raw >> 22) & 1 == 1;
        let mask = ((raw >> 16) & 0xF) as u8;
        let imm = raw & 0xFF;
        let rotate = ((raw >> 8) & 0xF) * 2;
        let val = imm.rotate_right(rotate);
        let op2 = Operand2::Immediate {
            val,
            carry_out: None,
        };
        return Instr::Msr {
            cond,
            use_spsr,
            mask,
            op2,
        };
    }

    let op2 = if is_immediate_op2 {
        let imm = raw & 0xFF;
        let rotate = ((raw >> 8) & 0xF) * 2;
        let val = imm.rotate_right(rotate);
        let carry_out = if rotate == 0 {
            None
        } else {
            Some((val >> 31) == 1)
        };
        Operand2::Immediate { val, carry_out }
    } else {
        let rm = (raw & 0xF) as u8;
        let shift_type = match (raw >> 5) & 0x3 {
            0 => ShiftType::Lsl,
            1 => ShiftType::Lsr,
            2 => ShiftType::Asr,
            3 => ShiftType::Ror,
            _ => unreachable!(),
        };
        let r_bit = (raw >> 4) & 1;
        let shift = if r_bit == 1 {
            Shift::Register {
                shift_type,
                rs: ((raw >> 8) & 0xF) as u8,
            }
        } else {
            Shift::Immediate {
                shift_type,
                amount: (raw >> 7) & 0x1F,
            }
        };
        Operand2::Register { rm, shift }
    };

    match opcode {
        0x0 => Instr::And {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x1 => Instr::Eor {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x2 => Instr::Sub {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x3 => Instr::Rsb {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x4 => Instr::Add {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x5 => Instr::Adc {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x6 => Instr::Sbc {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x7 => Instr::Rsc {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0x8 => Instr::Tst { cond, rn, op2 },
        0x9 => Instr::Teq { cond, rn, op2 },
        0xA => Instr::Cmp { cond, rn, op2 },
        0xB => Instr::Cmn { cond, rn, op2 },
        0xC => Instr::Orr {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0xD => Instr::Mov { cond, s, rd, op2 },
        0xE => Instr::Bic {
            cond,
            s,
            rd,
            rn,
            op2,
        },
        0xF => Instr::Mvn { cond, s, rd, op2 },
        _ => Instr::Unknown(raw),
    }
}

fn decode_load_store(cond: Condition, raw: u32, is_shifted_reg: bool) -> Instr {
    let p = (raw >> 24) & 1 == 1;
    let u = (raw >> 23) & 1 == 1;
    let b = (raw >> 22) & 1 == 1;
    let w = (raw >> 21) & 1 == 1;
    let l = (raw >> 20) & 1 == 1;
    let rn = ((raw >> 16) & 0xF) as u8;
    let rd = ((raw >> 12) & 0xF) as u8;

    let offset = if !is_shifted_reg {
        Operand2::Immediate {
            val: raw & 0xFFF,
            carry_out: None,
        }
    } else {
        let rm = (raw & 0xF) as u8;
        let shift_type = match (raw >> 5) & 0x3 {
            0 => ShiftType::Lsl,
            1 => ShiftType::Lsr,
            2 => ShiftType::Asr,
            3 => ShiftType::Ror,
            _ => unreachable!(),
        };
        let amount = (raw >> 7) & 0x1F;
        Operand2::Register {
            rm,
            shift: Shift::Immediate { shift_type, amount },
        }
    };

    if l {
        if b {
            Instr::Ldrb {
                cond,
                rd,
                rn,
                offset,
                pre: p,
                writeback: w,
                up: u,
            }
        } else {
            Instr::Ldr {
                cond,
                rd,
                rn,
                offset,
                pre: p,
                writeback: w,
                up: u,
            }
        }
    } else {
        if b {
            Instr::Strb {
                cond,
                rd,
                rn,
                offset,
                pre: p,
                writeback: w,
                up: u,
            }
        } else {
            Instr::Str {
                cond,
                rd,
                rn,
                offset,
                pre: p,
                writeback: w,
                up: u,
            }
        }
    }
}

fn decode_load_store_multiple(cond: Condition, raw: u32) -> Instr {
    let p = (raw >> 24) & 1 == 1;
    let u = (raw >> 23) & 1 == 1;
    let w = (raw >> 21) & 1 == 1;
    let l = (raw >> 20) & 1 == 1;
    let rn = ((raw >> 16) & 0xF) as u8;
    let reg_list = (raw & 0xFFFF) as u16;

    if l {
        Instr::Ldm {
            cond,
            rn,
            reg_list,
            p,
            u,
            w,
        }
    } else {
        Instr::Stm {
            cond,
            rn,
            reg_list,
            p,
            u,
            w,
        }
    }
}

fn decode_branch(cond: Condition, raw: u32) -> Instr {
    let l = (raw >> 24) & 1 == 1;
    let imm24 = raw & 0xFF_FFFF;
    let target = ((imm24 << 8) as i32) >> 6;

    if l {
        Instr::Bl { cond, target }
    } else {
        Instr::B { cond, target }
    }
}

fn decode_coproc_or_svc(cond: Condition, raw: u32) -> Instr {
    let op = (raw >> 24) & 0xF;
    if op == 0b1111 {
        Instr::Svc {
            cond,
            imm: raw & 0x00FFFFFF,
        }
    } else {
        Instr::Unknown(raw)
    }
}
