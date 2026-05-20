use super::instr::Instr;

#[inline(always)]
pub fn decode_riscv(raw: u32) -> Instr {
    let opcode = raw & 0x7F;
    let rd = ((raw >> 7) & 0x1F) as u8;
    let funct3 = (raw >> 12) & 0x7;
    let rs1 = ((raw >> 15) & 0x1F) as u8;
    let rs2 = ((raw >> 20) & 0x1F) as u8;
    let funct7 = (raw >> 25) & 0x7F;

    match opcode {
        0x37 => Instr::Lui {
            rd,
            imm: raw & 0xFFFFF000,
        },
        0x17 => Instr::Auipc {
            rd,
            imm: raw & 0xFFFFF000,
        },
        0x6F => {
            // J-Type
            let imm20 = (raw as i32) >> 31;
            let imm19_12 = (raw >> 12) & 0xFF;
            let imm11 = (raw >> 20) & 1;
            let imm10_1 = (raw >> 21) & 0x3FF;
            let imm = (imm20 << 20)
                | ((imm19_12 as i32) << 12)
                | ((imm11 as i32) << 11)
                | ((imm10_1 as i32) << 1);
            Instr::Jal { rd, imm }
        }
        0x67 => {
            // I-Type
            let imm = ((raw as i32) >> 20) as i32;
            Instr::Jalr { rd, rs1, imm }
        }
        0x63 => {
            // B-Type
            let imm12 = (raw as i32) >> 31;
            let imm11 = (raw >> 7) & 1;
            let imm10_5 = (raw >> 25) & 0x3F;
            let imm4_1 = (raw >> 8) & 0xF;
            let imm = (imm12 << 12)
                | ((imm11 as i32) << 11)
                | ((imm10_5 as i32) << 5)
                | ((imm4_1 as i32) << 1);
            match funct3 {
                0 => Instr::Beq { rs1, rs2, imm },
                1 => Instr::Bne { rs1, rs2, imm },
                4 => Instr::Blt { rs1, rs2, imm },
                5 => Instr::Bge { rs1, rs2, imm },
                6 => Instr::Bltu { rs1, rs2, imm },
                7 => Instr::Bgeu { rs1, rs2, imm },
                _ => Instr::Unknown(raw),
            }
        }
        0x03 => {
            // I-Type (Loads)
            let imm = ((raw as i32) >> 20) as i32;
            match funct3 {
                0 => Instr::Lb { rd, rs1, imm },
                1 => Instr::Lh { rd, rs1, imm },
                2 => Instr::Lw { rd, rs1, imm },
                4 => Instr::Lbu { rd, rs1, imm },
                5 => Instr::Lhu { rd, rs1, imm },
                _ => Instr::Unknown(raw),
            }
        }
        0x23 => {
            // S-Type (Stores)
            let imm11_5 = (raw as i32) >> 25;
            let imm4_0 = ((raw >> 7) & 0x1F) as i32;
            let imm = (imm11_5 << 5) | imm4_0;
            match funct3 {
                0 => Instr::Sb { rs1, rs2, imm },
                1 => Instr::Sh { rs1, rs2, imm },
                2 => Instr::Sw { rs1, rs2, imm },
                _ => Instr::Unknown(raw),
            }
        }
        0x13 => {
            // I-Type (ALU Imm)
            let imm = ((raw as i32) >> 20) as i32;
            let shamt = (imm & 0x1F) as u8;
            match funct3 {
                0 => Instr::Addi { rd, rs1, imm },
                1 => Instr::Slli { rd, rs1, shamt },
                2 => Instr::Slti { rd, rs1, imm },
                3 => Instr::Sltiu { rd, rs1, imm },
                4 => Instr::Xori { rd, rs1, imm },
                5 => match funct7 {
                    0x00 => Instr::Srli { rd, rs1, shamt },
                    0x20 => Instr::Srai { rd, rs1, shamt },
                    _ => Instr::Unknown(raw),
                },
                6 => Instr::Ori { rd, rs1, imm },
                7 => Instr::Andi { rd, rs1, imm },
                _ => Instr::Unknown(raw),
            }
        }
        0x33 => {
            // R-Type (ALU Reg)
            match (funct3, funct7) {
                (0, 0x00) => Instr::Add { rd, rs1, rs2 },
                (0, 0x20) => Instr::Sub { rd, rs1, rs2 },
                (1, 0x00) => Instr::Sll { rd, rs1, rs2 },
                (2, 0x00) => Instr::Slt { rd, rs1, rs2 },
                (3, 0x00) => Instr::Sltu { rd, rs1, rs2 },
                (4, 0x00) => Instr::Xor { rd, rs1, rs2 },
                (5, 0x00) => Instr::Srl { rd, rs1, rs2 },
                (5, 0x20) => Instr::Sra { rd, rs1, rs2 },
                (6, 0x00) => Instr::Or { rd, rs1, rs2 },
                (7, 0x00) => Instr::And { rd, rs1, rs2 },
                _ => Instr::Unknown(raw),
            }
        }
        0x73 => {
            let imm = (raw >> 20) & 0xFFF;
            match imm {
                0 => Instr::Ecall,
                1 => Instr::Ebreak,
                _ => Instr::Unknown(raw),
            }
        }
        _ => Instr::Unknown(raw),
    }
}
