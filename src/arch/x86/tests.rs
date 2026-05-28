use super::X86Cpu;
use super::execute::execute_instr;
use super::instr::*;
use super::registers::*;
use crate::bus::{MemoryBus, Perms};
use crate::config::CpuMode;
use crate::device::ram::Ram;
use crate::hook::HookRegistry;
use crate::interface::Cpu;

fn setup_test_env() -> (X86Cpu, MemoryBus, HookRegistry) {
    let cpu = X86Cpu::init(CpuMode::MODE_32).unwrap();
    let mut bus = MemoryBus::new();
    let hooks = HookRegistry::new();
    bus.map(0x1000, 0x1000, Perms::RWX, Ram::new(0x1000).into());
    (cpu, bus, hooks)
}

// RAW BINARY EXECUTION TESTS

#[test]
fn test_x86_binary_call_ret_stack() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 12] = [
        0xE8, 0x04, 0x00, 0x00, 0x00, // CALL +4
        0x90, 0x90, 0x90, 0x90, // NOP x4
        0x31, 0xC0, // XOR EAX, EAX
        0xC3, // RET
    ];
    bus.write_bytes(0x1000, &code).unwrap();

    cpu.regs[REG_EIP] = 0x1000;
    cpu.regs[REG_ESP] = 0x1F00;

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1009, "CALL should jump to 0x1009");
    assert_eq!(
        cpu.regs[REG_ESP], 0x1EFC,
        "CALL should push 4 bytes to stack"
    );

    cpu.regs[REG_EAX] = 0xDEADBEEF;
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EAX], 0,
        "XOR EAX, EAX should zero the register"
    );

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1005,
        "RET should pop return address into EIP"
    );
}

#[test]
fn test_x86_binary_jcc_branching() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 11] = [
        0x31, 0xC0, // XOR EAX, EAX
        0x74, 0x02, // JZ +2
        0x90, 0x90, // NOP, NOP
        0xB8, 0x63, 0x00, 0x00, 0x00, // MOV EAX, 99
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.step(&mut bus, &mut hooks).unwrap(); // XOR
    cpu.step(&mut bus, &mut hooks).unwrap(); // JZ
    assert_eq!(cpu.regs[REG_EIP], 0x1006, "JZ should have taken the branch");
}

#[test]
fn test_x86_binary_negative_jump() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 4] = [
        0x90, // NOP
        0x90, // NOP
        0xEB, 0xFC, // JMP -4
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.step(&mut bus, &mut hooks).unwrap(); // NOP
    cpu.step(&mut bus, &mut hooks).unwrap(); // NOP
    cpu.step(&mut bus, &mut hooks).unwrap(); // JMP -4

    assert_eq!(
        cpu.regs[REG_EIP], 0x1000,
        "Negative relative jump failed to wrap backwards"
    );
}

#[test]
fn test_x86_hot_loop_batching() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let nops = vec![0x90u8; 20];
    bus.write_bytes(0x1000, &nops).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    let executed = cpu.step_batch(&mut bus, &mut hooks, 20).unwrap();
    assert_eq!(executed, 20);
    assert_eq!(cpu.regs[REG_EIP], 0x1000 + 20);
}

// DIRECT ENUM EXECUTION TESTS (AST)

#[test]
fn test_x86_alu_add_flags() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr = Instr::Add {
        dest: Operand::Reg32(GpReg32::Eax),
        src: Operand::Reg32(GpReg32::Ebx),
    };

    cpu.regs[REG_EAX] = 0xFFFFFFFF;
    cpu.regs[REG_EBX] = 1;
    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[REG_EAX], 0);
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::ZF));
    assert!(f.contains(EFlags::CF), "Carry flag should be set");
}

#[test]
fn test_x86_parity_flag() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr = Instr::Xor {
        dest: Operand::Reg32(GpReg32::Eax),
        src: Operand::Reg32(GpReg32::Eax),
    };
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::PF), "Parity should be even for 0");
}

#[test]
fn test_x86_enum_lea_complex_sib() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let mem = MemoryAddr {
        base: Some(GpReg32::Eax),
        index: Some(GpReg32::Eax),
        scale: 2,
        disp: 0x10,
    };
    let instr = Instr::Lea {
        dest: GpReg32::Ecx,
        src: mem,
    };

    cpu.regs[REG_EAX] = 5;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_ECX], 31, "LEA failed complex SIB math");
}

#[test]
fn test_x86_enum_adc_sbb_carry_chain() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[REG_EAX] = 0xFFFFFFFF;
    cpu.regs[REG_EBX] = 0x00000001;

    let add_low = Instr::Add {
        dest: Operand::Reg32(GpReg32::Eax),
        src: Operand::Reg32(GpReg32::Ebx),
    };
    execute_instr(&mut cpu, add_low, &mut bus, &mut hooks).unwrap();

    cpu.regs[REG_ECX] = 1;
    cpu.regs[REG_EDX] = 2;

    let adc_high = Instr::Adc {
        dest: Operand::Reg32(GpReg32::Ecx),
        src: Operand::Reg32(GpReg32::Edx),
    };
    execute_instr(&mut cpu, adc_high, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[REG_ECX], 4, "ADC should include Carry Flag");
}

// HOOK & SYSCALL INTERCEPTION TESTS

#[test]
fn test_x86_syscall_interception() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // INT 0x80
    bus.write_bytes(0x1000, &[0xCD, 0x80]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    bus.write_bytes(0x1100, b"Hello").unwrap();

    cpu.regs[REG_EAX] = 4; // Syscall No: SYS_WRITE
    cpu.regs[REG_EBX] = 1; // Arg 1: FD (stdout)
    cpu.regs[REG_ECX] = 0x1100; // Arg 2: Buffer Address
    cpu.regs[REG_EDX] = 5; // Arg 3: Length

    let captured_string = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured_string_clone = std::sync::Arc::clone(&captured_string);

    hooks.add_interrupt_hook(
        move |hook_cpu: &mut dyn Cpu, hook_bus: &mut MemoryBus, vec: u32| {
            if vec == 0x80 {
                let eax = hook_cpu.read_reg(REG_EAX)?;
                if eax == 4 {
                    let buf_ptr = hook_cpu.read_reg(REG_ECX)?;
                    let length = hook_cpu.read_reg(REG_EDX)?;
                    let mut string_buf = vec![0u8; length as usize];
                    hook_bus.read_bytes(buf_ptr, &mut string_buf)?;
                    *captured_string_clone.lock().unwrap() =
                        String::from_utf8_lossy(&string_buf).into_owned();
                    hook_cpu.write_reg(REG_EAX, length)?;
                }
                Ok(true)
            } else {
                Ok(false)
            }
        },
    );

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(*captured_string.lock().unwrap(), "Hello");
    assert_eq!(cpu.regs[REG_EAX], 5, "Syscall should return bytes written");
}

// EDGE CASE

#[test]
fn test_x86_pop_esp_quirk() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[REG_ESP] = 0x1EFC;
    bus.write_32(0x1EFC, 0xABCD1234).unwrap();

    // 0x5C = POP ESP
    bus.write_bytes(0x1000, &[0x5C]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_ESP], 0xABCD1234,
        "POP ESP failed to overwrite stack pointer increment!"
    );
}

#[test]
fn test_x86_group1_immediate_math() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 15] = [
        0x83, 0xC4, 0x04, // ADD ESP, 4
        0x81, 0xF8, 0x78, 0x56, 0x34, 0x12, // CMP EAX, 0x12345678
        0xC7, 0x03, 0xEF, 0xBE, 0xAD, 0xDE, // MOV DWORD PTR [EBX], 0xDEADBEEF
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_ESP] = 0x1000;
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_ESP], 0x1004, "ADD ESP, 4 (0x83) failed");

    cpu.regs[REG_EAX] = 0x12345678;
    cpu.step(&mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        f.contains(EFlags::ZF),
        "CMP EAX, Imm32 (0x81) failed to set Zero Flag"
    );

    cpu.regs[REG_EBX] = 0x1500;
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        bus.read_32(0x1500).unwrap(),
        0xDEADBEEF,
        "MOV [EBX], Imm32 (0xC7) failed"
    );
}

#[test]
fn test_x86_inc_dec_carry_preservation() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    bus.write_bytes(0x1000, &[0x40, 0x4B]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0xFFFFFFFF;
    let mut flags = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    flags.insert(EFlags::CF);
    cpu.regs[REG_EFLAGS] = flags.bits();

    cpu.step(&mut bus, &mut hooks).unwrap(); // INC

    assert_eq!(cpu.regs[REG_EAX], 0, "INC failed to wrap to 0");
    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f1.contains(EFlags::ZF), "INC should set Zero Flag here");
    assert!(
        f1.contains(EFlags::CF),
        "INC MUST PRESERVE the Carry Flag! (CF should still be 1)"
    );

    cpu.regs[REG_EBX] = 0;
    let mut flags2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    flags2.remove(EFlags::CF);
    cpu.regs[REG_EFLAGS] = flags2.bits();

    cpu.step(&mut bus, &mut hooks).unwrap(); // DEC

    assert_eq!(
        cpu.regs[REG_EBX], 0xFFFFFFFF,
        "DEC failed to wrap to 0xFFFFFFFF"
    );
    let f2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f2.contains(EFlags::SF), "DEC should set Sign Flag here");
    assert!(
        !f2.contains(EFlags::CF),
        "DEC MUST PRESERVE the Carry Flag! (CF should still be 0)"
    );
}

#[test]
fn test_x86_test_instruction() {
    let (mut cpu, mut bus, _) = setup_test_env();

    let instr = Instr::Test {
        dest: Operand::Reg32(GpReg32::Eax),
        src: Operand::Reg32(GpReg32::Eax),
    };

    cpu.regs[REG_EAX] = 0;
    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut HookRegistry::new()).unwrap();
    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f1.contains(EFlags::ZF), "TEST 0, 0 should set Zero Flag");

    cpu.regs[REG_EAX] = 0x80000000;
    execute_instr(&mut cpu, instr, &mut bus, &mut HookRegistry::new()).unwrap();
    let f2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        !f2.contains(EFlags::ZF),
        "TEST 0x80000000, 0x80000000 should clear Zero Flag"
    );
    assert!(
        f2.contains(EFlags::SF),
        "TEST 0x80000000, 0x80000000 should set Sign Flag"
    );
}

#[test]
fn test_x86_leave_instruction() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    bus.write_bytes(0x1000, &[0xC9]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EBP] = 0x1F00;
    cpu.regs[REG_ESP] = 0x1EF0;
    bus.write_32(0x1F00, 0x1F80).unwrap();

    cpu.step(&mut bus, &mut hooks).unwrap();

    assert_eq!(
        cpu.regs[REG_EBP], 0x1F80,
        "LEAVE failed to pop caller's EBP"
    );
    assert_eq!(
        cpu.regs[REG_ESP], 0x1F04,
        "LEAVE failed to restore and increment ESP"
    );
}

#[test]
fn test_x86_mul_div() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 4] = [0xF7, 0xE3, 0xF7, 0xF3];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0x80000000;
    cpu.regs[REG_EBX] = 2;

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EAX], 0,
        "Lower 32-bits of MUL should overflow to 0"
    );
    assert_eq!(cpu.regs[REG_EDX], 1, "Upper 32-bits of MUL should hold 1");

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EAX], 0x80000000,
        "Quotient should be 0x80000000"
    );
    assert_eq!(cpu.regs[REG_EDX], 0, "Remainder should be 0");
}

#[test]
fn test_x86_shifts() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 6] = [0xC1, 0xE0, 0x02, 0xC1, 0xF8, 0x02];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0xC0000000;
    cpu.step(&mut bus, &mut hooks).unwrap(); // SHL
    assert_eq!(cpu.regs[REG_EAX], 0);

    cpu.regs[REG_EAX] = 0x80000000;
    cpu.step(&mut bus, &mut hooks).unwrap(); // SAR
    assert_eq!(cpu.regs[REG_EAX], 0xE0000000, "SAR must duplicate sign bit");
}

#[test]
fn test_x86_zero_and_sign_extensions() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 6] = [0x0F, 0xB6, 0xC3, 0x0F, 0xBE, 0xC3];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EBX] = 0xFF;

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOVZX
    assert_eq!(cpu.regs[REG_EAX], 0x000000FF, "MOVZX failed to zero-pad");

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOVSX
    assert_eq!(cpu.regs[REG_EAX], 0xFFFFFFFF, "MOVSX failed to sign-extend");
}

#[test]
fn test_x86_accumulator_and_standard_math() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 12] = [
        0x05, 0x78, 0x56, 0x34, 0x12, // ADD EAX, 0x12345678
        0x2B, 0xD9, // SUB EBX, ECX
        0x25, 0xFF, 0x00, 0x00, 0x00, // AND EAX, 0x000000FF
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0x00000000;
    cpu.regs[REG_EBX] = 100;
    cpu.regs[REG_ECX] = 40;

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0x12345678, "0x05 ADD EAX failed");

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EBX], 60, "0x2B SUB r32, r/m32 failed");

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0x78, "0x25 AND EAX failed");
}

#[test]
fn test_x86_neg_and_not() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 4] = [
        0xF7, 0xD0, // NOT EAX
        0xF7, 0xDB, // NEG EBX
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0x00000000;
    cpu.regs[REG_EBX] = 0x00000001;

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0xFFFFFFFF, "NOT failed to invert bits");

    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EBX], 0xFFFFFFFF, "NEG 1 should result in -1");
}

#[test]
fn test_x86_8bit_mov_and_math() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 7] = [0x8A, 0x45, 0x00, 0x04, 0xFF, 0x88, 0x03];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EBP] = 0x1500;
    bus.write_8(0x1500, 0x02).unwrap();
    cpu.regs[REG_EBX] = 0x1600;

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOV AL, [EBP]
    assert_eq!(
        cpu.regs[REG_EAX] & 0xFF,
        0x02,
        "Failed to load 8-bit AL from memory"
    );

    cpu.step(&mut bus, &mut hooks).unwrap(); // ADD AL, 0xFF
    assert_eq!(
        cpu.regs[REG_EAX] & 0xFF,
        0x01,
        "Failed 8-bit math wraparound"
    );

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOV [EBX], AL
    assert_eq!(
        bus.read_8(0x1600).unwrap(),
        0x01,
        "Failed to store 8-bit AL into memory"
    );
}

#[test]
fn test_x86_16bit_override_prefix() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let code: [u8; 10] = [0x66, 0xB8, 0x34, 0x12, 0x66, 0x01, 0xD8, 0x66, 0x89, 0x01];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.regs[REG_EAX] = 0xDEAD0000;
    cpu.regs[REG_EBX] = 0xBEEFFFFF;
    cpu.regs[REG_ECX] = 0x1500;

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOV AX, 0x1234
    assert_eq!(
        cpu.regs[REG_EAX], 0xDEAD1234,
        "0x66 MOV AX failed to preserve upper 16 bits"
    );

    cpu.step(&mut bus, &mut hooks).unwrap(); // ADD AX, BX
    assert_eq!(
        cpu.regs[REG_EAX], 0xDEAD1233,
        "16-bit ADD AX, BX math failed"
    );

    cpu.step(&mut bus, &mut hooks).unwrap(); // MOV [ECX], AX
    assert_eq!(
        bus.read_16(0x1500).unwrap(),
        0x1233,
        "16-bit memory write failed"
    );
}

// String Ops, Conversions, Flags, etc.
#[test]
fn test_x86_string_ops_and_flags() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Setup
    cpu.regs[REG_EAX] = 0xAABBCCDD;
    cpu.regs[REG_EDI] = 0x1000;
    cpu.regs[REG_ESI] = 0x1004;
    bus.write_32(0x1004, 0x11223344).unwrap();

    // STOSD (Write EAX to [EDI], EDI += 4)
    execute_instr(
        &mut cpu,
        Instr::Stos(OpSize::Dword, None),
        &mut bus,
        &mut hooks,
    )
    .unwrap();
    assert_eq!(bus.read_32(0x1000).unwrap(), 0xAABBCCDD);
    assert_eq!(cpu.regs[REG_EDI], 0x1004);

    // LODSD (Read [ESI] to EAX, ESI += 4)
    execute_instr(
        &mut cpu,
        Instr::Lods(OpSize::Dword, None),
        &mut bus,
        &mut hooks,
    )
    .unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0x11223344);
    assert_eq!(cpu.regs[REG_ESI], 0x1008);

    // STD (Set Direction Flag)
    execute_instr(&mut cpu, Instr::Std, &mut bus, &mut hooks).unwrap();
    assert!((cpu.regs[REG_EFLAGS] & EFlags::DF.bits()) != 0);

    // MOVSB (Copy [ESI] to [EDI], ESI -= 1, EDI -= 1)
    cpu.regs[REG_ESI] = 0x1004; // 0x44 (little endian of 0x11223344)
    cpu.regs[REG_EDI] = 0x1008;
    execute_instr(
        &mut cpu,
        Instr::Movs(OpSize::Byte, None),
        &mut bus,
        &mut hooks,
    )
    .unwrap();
    assert_eq!(bus.read_8(0x1008).unwrap(), 0x44);
    assert_eq!(cpu.regs[REG_ESI], 0x1003);
    assert_eq!(cpu.regs[REG_EDI], 0x1007);
}

#[test]
fn test_x86_rep_scas_cmps() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Write a string: "Hello"
    bus.write_bytes(0x1000, b"Hello").unwrap();

    // REPNE SCASB (Search for 'l' in "Hello")
    cpu.regs[REG_EDI] = 0x1000;
    cpu.regs[REG_ECX] = 5;
    cpu.regs[REG_EAX] = b'l' as u32; // Search for 'l' (0x6C)

    // CLD (Ensure DF=0)
    execute_instr(&mut cpu, Instr::Cld, &mut bus, &mut hooks).unwrap();
    assert!((cpu.regs[REG_EFLAGS] & EFlags::DF.bits()) == 0);

    execute_instr(
        &mut cpu,
        Instr::Scas(OpSize::Byte, Some(RepPrefix::Repne)),
        &mut bus,
        &mut hooks,
    )
    .unwrap();

    // Execution trace:
    // "H" (1000) != 'l' -> dec ECX (4), inc EDI (1001)
    // "e" (1001) != 'l' -> dec ECX (3), inc EDI (1002)
    // "l" (1002) == 'l' -> dec ECX (2), inc EDI (1003), STOP (ZF=1)
    assert_eq!(cpu.regs[REG_ECX], 2);
    assert_eq!(cpu.regs[REG_EDI], 0x1003);
    assert!(
        (cpu.regs[REG_EFLAGS] & EFlags::ZF.bits()) != 0,
        "Zero flag should be set on match"
    );
}

#[test]
fn test_x86_binary_rep_movs() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // F3 A4 : REP MOVSB
    let code = [0xF3, 0xA4];
    bus.write_bytes(0x1000, &code).unwrap();

    bus.write_bytes(0x1500, b"ABC").unwrap();
    cpu.regs[REG_ESI] = 0x1500;
    cpu.regs[REG_EDI] = 0x1600;
    cpu.regs[REG_ECX] = 3;
    cpu.regs[REG_EIP] = 0x1000;

    cpu.step(&mut bus, &mut hooks).unwrap();

    let mut out = vec![0u8; 3];
    bus.read_bytes(0x1600, &mut out).unwrap();
    assert_eq!(&out, b"ABC");
    assert_eq!(cpu.regs[REG_ECX], 0);
    assert_eq!(cpu.regs[REG_ESI], 0x1503);
    assert_eq!(cpu.regs[REG_EDI], 0x1603);
}

#[test]
fn test_x86_conversions() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // CBW (AL -> AX)
    cpu.regs[REG_EAX] = 0x000000FE; // -2 in AL
    execute_instr(&mut cpu, Instr::Cbw(OpSize::Word), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0x0000FFFE); // -2 in AX

    // CWDE (AX -> EAX), maps to Cbw(OpSize::Dword)
    execute_instr(&mut cpu, Instr::Cbw(OpSize::Dword), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0xFFFFFFFE); // -2 in EAX

    // CWD (AX -> DX:AX)
    cpu.regs[REG_EAX] = 0x00008000; // -32768 in AX
    cpu.regs[REG_EDX] = 0x00000000;
    execute_instr(&mut cpu, Instr::Cwd(OpSize::Word), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EDX], 0x0000FFFF); // DX is all 1s

    // CDQ (EAX -> EDX:EAX), maps to Cwd(OpSize::Dword)
    cpu.regs[REG_EAX] = 0x7FFFFFFF; // Positive
    cpu.regs[REG_EDX] = 0xFFFFFFFF;
    execute_instr(&mut cpu, Instr::Cwd(OpSize::Dword), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EDX], 0x00000000); // EDX is all 0s
}

#[test]
fn test_x86_unimplemented_io_and_system() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // IN AL, 0x40
    let res = execute_instr(
        &mut cpu,
        Instr::In(OpSize::Byte, Operand::Imm8(0x40)),
        &mut bus,
        &mut hooks,
    );
    assert!(matches!(
        res,
        Err(crate::error::EmuError::NotImplemented(_))
    ));

    // SYSCALL
    let res2 = execute_instr(&mut cpu, Instr::Syscall, &mut bus, &mut hooks);
    assert!(matches!(
        res2,
        Err(crate::error::EmuError::NotImplemented(_))
    ));
}
