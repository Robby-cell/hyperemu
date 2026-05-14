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

    // Map 4KB of RAM at 0x1000
    bus.map(0x1000, 0x1000, Perms::RWX, Ram::new(0x1000).into());

    (cpu, bus, hooks)
}

#[test]
fn test_x86_alu_add_flags() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // ADD eax, ebx
    let instr = Instr::Add {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Reg(GpReg::Ebx),
    };

    // Case 1: Simple addition, no flags
    cpu.regs[REG_EAX] = 10;
    cpu.regs[REG_EBX] = 20;
    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 30);
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(!f.contains(EFlags::ZF));
    assert!(!f.contains(EFlags::CF));

    // Case 2: Zero Flag
    cpu.regs[REG_EAX] = 0xFFFFFFFF;
    cpu.regs[REG_EBX] = 1;
    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 0);
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::ZF));
    assert!(f.contains(EFlags::CF), "Carry flag should be set");

    // Case 3: Overflow Flag (Positive + Positive = Negative)
    cpu.regs[REG_EAX] = 0x7FFFFFFF;
    cpu.regs[REG_EBX] = 1;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::OF), "Overflow flag should be set");
    assert!(f.contains(EFlags::SF), "Sign flag should be set");
}

#[test]
fn test_x86_parity_flag() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // XOR eax, eax -> result 0. Low byte is 00000000 (even number of 1s)
    let instr = Instr::Xor {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Reg(GpReg::Eax),
    };
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::PF), "Parity should be even for 0");

    // MOV eax, 3 -> XOR eax, 2 -> result 1. Low byte is 00000001 (odd number of 1s)
    cpu.regs[REG_EAX] = 3;
    let instr2 = Instr::Xor {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Imm32(2),
    };
    execute_instr(&mut cpu, instr2, &mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(!f.contains(EFlags::PF), "Parity should be odd for result 1");
}

#[test]
fn test_x86_addressing_modes_sib() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Goal: MOV [eax + ebx*4 + 0x10], ecx
    let mem = MemoryAddr {
        base: Some(GpReg::Eax),
        index: Some(GpReg::Ebx),
        scale: 4,
        disp: 0x10,
    };
    let instr = Instr::Mov {
        dest: Operand::Mem(mem),
        src: Operand::Reg(GpReg::Ecx),
    };

    cpu.regs[REG_EAX] = 0x1000;
    cpu.regs[REG_EBX] = 2;
    cpu.regs[REG_ECX] = 0xDEADBEEF;

    // Target addr: 0x1000 + (2 * 4) + 16 = 0x1000 + 8 + 16 = 0x1018
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    let val = bus.read_32(0x1018).unwrap();
    assert_eq!(val, 0xDEADBEEF);
}

#[test]
fn test_x86_stack_ops() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[REG_ESP] = 0x1F00;
    cpu.regs[REG_EAX] = 0x12345678;

    // PUSH eax
    execute_instr(
        &mut cpu,
        Instr::Push(Operand::Reg(GpReg::Eax)),
        &mut bus,
        &mut hooks,
    )
    .unwrap();
    assert_eq!(cpu.regs[REG_ESP], 0x1EFC);
    assert_eq!(bus.read_32(0x1EFC).unwrap(), 0x12345678);

    // POP ebx
    execute_instr(
        &mut cpu,
        Instr::Pop(Operand::Reg(GpReg::Ebx)),
        &mut bus,
        &mut hooks,
    )
    .unwrap();
    assert_eq!(cpu.regs[REG_ESP], 0x1F00);
    assert_eq!(cpu.regs[REG_EBX], 0x12345678);
}

#[test]
fn test_x86_call_ret() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[REG_EIP] = 0x1000;
    cpu.regs[REG_ESP] = 0x1F00;

    // CALL 0x50 (Relative) -> Target is 0x1000 + 0x50 = 0x1050
    execute_instr(&mut cpu, Instr::Call(0x50), &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[REG_EIP], 0x1050);
    assert_eq!(cpu.regs[REG_ESP], 0x1F00 - 4);
    assert_eq!(
        bus.read_32(0x1EFC).unwrap(),
        0x1000,
        "Return address on stack should be 0x1000"
    );

    // RET
    execute_instr(&mut cpu, Instr::Ret, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1000);
    assert_eq!(cpu.regs[REG_ESP], 0x1F00);
}

#[test]
fn test_x86_conditional_jumps() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[REG_EIP] = 0x1000;

    // JE 0x20 (Jump if Equal/Zero)
    let instr = Instr::Jcc(Condition::E, 0x20);

    // Scenario 1: ZF = 0 (No Jump)
    let mut f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    f.remove(EFlags::ZF);
    cpu.regs[REG_EFLAGS] = f.bits();

    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1000, "Should not jump if ZF=0");

    // Scenario 2: ZF = 1 (Jump)
    f.insert(EFlags::ZF);
    cpu.regs[REG_EFLAGS] = f.bits();

    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1020, "Should jump if ZF=1");
}

#[test]
fn test_x86_variable_length_decode() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Write binary to memory:
    // 0x90             : NOP (1 byte)
    // 0xB8 78 56 34 12 : MOV EAX, 0x12345678 (5 bytes)
    let code: [u8; 6] = [0x90, 0xB8, 0x78, 0x56, 0x34, 0x12];
    bus.write_bytes(0x1000, &code).unwrap();

    cpu.regs[REG_EIP] = 0x1000;

    // Step 1: NOP
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1001,
        "EIP should advance 1 byte for NOP"
    );

    // Step 2: MOV EAX, Imm32
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1006,
        "EIP should advance 5 bytes for MOV Imm32"
    );
    assert_eq!(cpu.regs[REG_EAX], 0x12345678);
}

#[test]
fn test_x86_hot_loop_batching() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Write 20 NOPs (0x90)
    let nops = vec![0x90u8; 20];
    bus.write_bytes(0x1000, &nops).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    // Run batch of 20
    let executed = cpu.step_batch(&mut bus, &mut hooks, 20).unwrap();

    assert_eq!(executed, 20);
    assert_eq!(cpu.regs[REG_EIP], 0x1000 + 20);
}

#[test]
fn test_x86_complex_logic_cmp() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // CMP eax, 100
    let instr = Instr::Cmp {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Imm32(100),
    };

    // Case: EAX is 50 (50 - 100) -> Negative, Borrow (Carry)
    cpu.regs[REG_EAX] = 50;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::CF), "Carry flag should be set (Borrow)");
    assert!(f.contains(EFlags::SF), "Sign flag should be set (Negative)");
    assert!(!f.contains(EFlags::ZF), "Zero flag should be clear");
    assert_eq!(
        cpu.regs[REG_EAX], 50,
        "CMP must not modify destination register"
    );
}

#[test]
fn test_x86_binary_call_ret_stack() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // X86 Assembly:
    // 0x1000: E8 04 00 00 00   CALL 0x1009 (Relative +4 from next EIP 0x1005)
    // 0x1005: 90               NOP
    // 0x1006: 90               NOP
    // 0x1007: 90               NOP
    // 0x1008: 90               NOP
    // 0x1009: 31 C0            XOR EAX, EAX
    // 0x100B: C3               RET
    let code: [u8; 12] = [
        0xE8, 0x04, 0x00, 0x00, 0x00, // CALL +4
        0x90, 0x90, 0x90, 0x90, // NOP x4
        0x31, 0xC0, // XOR EAX, EAX
        0xC3, // RET
    ];
    bus.write_bytes(0x1000, &code).unwrap();

    cpu.regs[REG_EIP] = 0x1000;
    cpu.regs[REG_ESP] = 0x1F00;

    // Step 1: CALL
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1009, "CALL should jump to 0x1009");
    assert_eq!(
        cpu.regs[REG_ESP], 0x1EFC,
        "CALL should push 4 bytes to stack"
    );
    assert_eq!(
        bus.read_32(0x1EFC).unwrap(),
        0x1005,
        "Return address should be 0x1005"
    );

    // Step 2: XOR EAX, EAX
    cpu.regs[REG_EAX] = 0xDEADBEEF; // Dirty the register
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EAX], 0,
        "XOR EAX, EAX should zero the register"
    );
    assert_eq!(cpu.regs[REG_EIP], 0x100B, "EIP should advance to RET");

    // Step 3: RET
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1005,
        "RET should pop return address into EIP"
    );
    assert_eq!(cpu.regs[REG_ESP], 0x1F00, "Stack should be balanced");

    // Step 4: NOP
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1006,
        "EIP should increment by 1 for NOP"
    );
}

#[test]
fn test_x86_binary_jcc_branching() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // X86 Assembly:
    // 0x1000: 31 C0            XOR EAX, EAX   (Sets Zero Flag to 1)
    // 0x1002: 74 02            JZ +2          (Jumps over the next 2 bytes)
    // 0x1004: 90               NOP
    // 0x1005: 90               NOP
    // 0x1006: B8 63 00 00 00   MOV EAX, 99
    let code: [u8; 11] = [
        0x31, 0xC0, // XOR EAX, EAX
        0x74, 0x02, // JZ +2
        0x90, 0x90, // NOP, NOP
        0xB8, 0x63, 0x00, 0x00, 0x00, // MOV EAX, 99
    ];
    bus.write_bytes(0x1000, &code).unwrap();

    cpu.regs[REG_EIP] = 0x1000;

    // Step 1: XOR
    cpu.step(&mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f.contains(EFlags::ZF), "XOR must set Zero Flag");

    // Step 2: JZ (Jump if Zero)
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1006, "JZ should have taken the branch");

    // Step 3: MOV
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EAX], 99, "MOV EAX, 99 executed");
}

#[test]
fn test_x86_enum_lea_complex_sib() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Goal: LEA ECX, [EAX*2 + EAX + 0x10]
    // This is equivalent to ECX = EAX * 3 + 0x10.
    let mem = MemoryAddr {
        base: Some(GpReg::Eax),
        index: Some(GpReg::Eax),
        scale: 2,
        disp: 0x10,
    };
    let instr = Instr::Lea {
        dest: GpReg::Ecx,
        src: mem,
    };

    cpu.regs[REG_EAX] = 5;
    cpu.regs[REG_ECX] = 0; // Initialize to 0

    // Execution should NOT read from memory. It just does the math.
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    // EAX(5) * 2 = 10. Base EAX = 5. Disp = 16 (0x10). Total = 10 + 5 + 16 = 31.
    assert_eq!(cpu.regs[REG_ECX], 31, "LEA failed complex SIB math");
}

#[test]
fn test_x86_enum_adc_sbb_carry_chain() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // We are simulating a 64-bit addition:
    // 0x00000001_FFFFFFFF + 0x00000002_00000001 = 0x00000004_00000000

    // Step 1: ADD the low 32-bits (EAX = 0xFFFFFFFF, EBX = 0x00000001)
    cpu.regs[REG_EAX] = 0xFFFFFFFF;
    cpu.regs[REG_EBX] = 0x00000001;

    let add_low = Instr::Add {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Reg(GpReg::Ebx),
    };
    execute_instr(&mut cpu, add_low, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[REG_EAX], 0, "Low 32-bits should overflow to 0");
    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        f1.contains(EFlags::CF),
        "Carry Flag MUST be set after overflow"
    );

    // Step 2: ADC the high 32-bits (ECX = 1, EDX = 2)
    cpu.regs[REG_ECX] = 1;
    cpu.regs[REG_EDX] = 2;

    let adc_high = Instr::Adc {
        dest: Operand::Reg(GpReg::Ecx),
        src: Operand::Reg(GpReg::Edx),
    };
    execute_instr(&mut cpu, adc_high, &mut bus, &mut hooks).unwrap();

    // ECX(1) + EDX(2) + Carry(1) = 4
    assert_eq!(
        cpu.regs[REG_ECX], 4,
        "ADC should include Carry Flag from previous operation"
    );
    let f2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        !f2.contains(EFlags::CF),
        "Carry Flag should be cleared after no-overflow ADC"
    );
}

#[test]
fn test_x86_enum_memory_displacements() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Goal: Store EAX to memory using EBP as base and a negative displacement.
    // MOV [EBP - 0x08], EAX
    let mem = MemoryAddr {
        base: Some(GpReg::Ebp),
        index: None,
        scale: 1,
        disp: -8, // Negative displacement (common for local stack variables)
    };
    let instr_store = Instr::Mov {
        dest: Operand::Mem(mem),
        src: Operand::Reg(GpReg::Eax),
    };

    cpu.regs[REG_EBP] = 0x1500;
    cpu.regs[REG_EAX] = 0xCAFEBABE;

    execute_instr(&mut cpu, instr_store, &mut bus, &mut hooks).unwrap();

    // Check that memory was written to 0x1500 - 8 = 0x14F8
    let val = bus.read_32(0x14F8).unwrap();
    assert_eq!(val, 0xCAFEBABE, "Negative displacement memory write failed");

    // Goal: Load it back into EBX using just a raw displacement (Global Variable)
    // MOV EBX, [0x14F8]
    let mem_global = MemoryAddr {
        base: None,
        index: None,
        scale: 1,
        disp: 0x14F8,
    };
    let instr_load = Instr::Mov {
        dest: Operand::Reg(GpReg::Ebx),
        src: Operand::Mem(mem_global),
    };

    execute_instr(&mut cpu, instr_load, &mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EBX], 0xCAFEBABE,
        "Global memory displacement read failed"
    );
}

#[test]
fn test_x86_binary_negative_jump() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // X86 Assembly:
    // 0x1000: 90       NOP
    // 0x1001: 90       NOP
    // 0x1002: EB FC    JMP -4 (Relative to next EIP 0x1004) -> Jumps to 0x1000
    let code: [u8; 4] = [
        0x90, // NOP
        0x90, // NOP
        0xEB, 0xFC, // JMP -4
    ];
    bus.write_bytes(0x1000, &code).unwrap();

    cpu.regs[REG_EIP] = 0x1000;

    // Step 1: NOP
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1001);

    // Step 2: NOP
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_EIP], 0x1002);

    // Step 3: JMP -4
    // The decoder consumes 2 bytes, advancing EIP to 0x1004.
    // 0x1004 + (-4) = 0x1000.
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(
        cpu.regs[REG_EIP], 0x1000,
        "Negative relative jump failed to wrap backwards"
    );
}

#[test]
fn test_x86_enum_lea_no_base() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Goal: LEA EAX, [EBX*4 + 0x20]
    // Notice `base` is None. This tests the logic where an index exists but no base.
    let mem = MemoryAddr {
        base: None,
        index: Some(GpReg::Ebx),
        scale: 4,
        disp: 0x20,
    };

    let instr = Instr::Lea {
        dest: GpReg::Eax,
        src: mem,
    };

    cpu.regs[REG_EBX] = 10;
    cpu.regs[REG_EAX] = 0; // Initialize destination to 0

    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    // EBX(10) * 4 = 40. Disp = 32 (0x20). Total = 72.
    assert_eq!(
        cpu.regs[REG_EAX], 72,
        "LEA with index and displacement but NO base failed"
    );
}

#[test]
fn test_x86_enum_sub_sbb_borrow_chain() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Simulating 64-bit subtraction:
    // 0x00000000_00000000 - 0x00000000_00000001

    // Step 1: SUB low 32-bits (EAX = 0, EBX = 1)
    cpu.regs[REG_EAX] = 0;
    cpu.regs[REG_EBX] = 1;

    let sub_low = Instr::Sub {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Reg(GpReg::Ebx),
    };
    execute_instr(&mut cpu, sub_low, &mut bus, &mut hooks).unwrap();

    assert_eq!(
        cpu.regs[REG_EAX], 0xFFFFFFFF,
        "0 - 1 should wrap to 0xFFFFFFFF"
    );
    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f1.contains(EFlags::CF), "Carry Flag (Borrow) MUST be set");

    // Step 2: SBB high 32-bits (ECX = 0, EDX = 0)
    cpu.regs[REG_ECX] = 0;
    cpu.regs[REG_EDX] = 0;

    let sbb_high = Instr::Sbb {
        dest: Operand::Reg(GpReg::Ecx),
        src: Operand::Reg(GpReg::Edx),
    };
    execute_instr(&mut cpu, sbb_high, &mut bus, &mut hooks).unwrap();

    // ECX(0) - EDX(0) - Borrow(1) = 0xFFFFFFFF
    assert_eq!(
        cpu.regs[REG_ECX], 0xFFFFFFFF,
        "SBB should subtract the Borrow Flag from previous operation"
    );
}

#[test]
fn test_x86_syscall_interception() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Simulating a Linux i386 Syscall for `sys_write` (eax = 4)
    // Code: INT 0x80
    bus.write_bytes(0x1000, &[0xCD, 0x80]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    // Put "Hello" in RAM at address 0x1100
    bus.write_bytes(0x1100, b"Hello").unwrap();

    // Standard Linux 32-bit Syscall Calling Convention:
    cpu.regs[REG_EAX] = 4; // Syscall No: SYS_WRITE
    cpu.regs[REG_EBX] = 1; // Arg 1: FD (stdout)
    cpu.regs[REG_ECX] = 0x1100; // Arg 2: Buffer Address
    cpu.regs[REG_EDX] = 5; // Arg 3: Length

    // Create an OS Hook to intercept the INT
    let captured_string = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured_string_clone = std::sync::Arc::clone(&captured_string);

    hooks.add_interrupt_hook(
        move |hook_cpu: &mut dyn Cpu, hook_bus: &mut MemoryBus, vec: u32| {
            if vec == 0x80 {
                // Intercept Linux Syscall Vector
                let eax = hook_cpu.read_reg(REG_EAX)?;

                if eax == 4 {
                    // sys_write
                    let buf_ptr = hook_cpu.read_reg(REG_ECX)?;
                    let length = hook_cpu.read_reg(REG_EDX)?;

                    let mut string_buf = vec![0u8; length as usize];
                    hook_bus.read_bytes(buf_ptr, &mut string_buf)?;

                    let result_str = String::from_utf8_lossy(&string_buf).into_owned();
                    *captured_string_clone.lock().unwrap() = result_str;

                    // Write return code (bytes written) to EAX
                    hook_cpu.write_reg(REG_EAX, length)?;
                }
                Ok(true) // Consumed by the OS!
            } else {
                Ok(false)
            }
        },
    );

    // Execute the INT 0x80 instruction
    cpu.step(&mut bus, &mut hooks).unwrap();

    // Verify Hook Execution
    assert_eq!(*captured_string.lock().unwrap(), "Hello");
    assert_eq!(cpu.regs[REG_EAX], 5, "Syscall should return bytes written");
}

#[test]
fn test_x86_pop_esp_quirk() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // The x86 Quirks manual states:
    // "POP ESP reads the value from the stack, increments the internal ESP,
    //  and THEN stores the read value into ESP, discarding the increment."

    // We will place the target value (0xABCD1234) on the stack at 0x1EFC.
    cpu.regs[REG_ESP] = 0x1EFC;
    bus.write_32(0x1EFC, 0xABCD1234).unwrap();

    // 0x5C = POP ESP
    bus.write_bytes(0x1000, &[0x5C]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    cpu.step(&mut bus, &mut hooks).unwrap();

    // If the emulator did `load -> store to ESP -> ESP += 4`, ESP would incorrectly be 0xABCD1238.
    // If correct, it must exactly equal the popped value.
    assert_eq!(
        cpu.regs[REG_ESP], 0xABCD1234,
        "POP ESP failed to overwrite stack pointer increment!"
    );
}

#[test]
fn test_x86_group1_immediate_math() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // We test 3 vital compiler instructions:
    // 1. ADD ESP, 4            (Opcode 0x83, ModRM 0xC4, Imm8 0x04) -> Sign-extended 8-bit math
    // 2. CMP EAX, 0x12345678   (Opcode 0x81, ModRM 0xF8, Imm32 0x12345678) -> Full 32-bit immediate math
    // 3. MOV [EBX], 0xDEADBEEF (Opcode 0xC7, ModRM 0x03, Imm32 0xDEADBEEF) -> Mem store immediate
    let code: [u8; 15] = [
        0x83, 0xC4, 0x04, // ADD ESP, 4
        0x81, 0xF8, 0x78, 0x56, 0x34, 0x12, // CMP EAX, 0x12345678
        0xC7, 0x03, 0xEF, 0xBE, 0xAD, 0xDE, // MOV DWORD PTR [EBX], 0xDEADBEEF
    ];
    bus.write_bytes(0x1000, &code).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    // Step 1: ADD ESP, 4
    cpu.regs[REG_ESP] = 0x1000;
    cpu.step(&mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[REG_ESP], 0x1004, "ADD ESP, 4 (0x83) failed");

    // Step 2: CMP EAX, 0x12345678
    cpu.regs[REG_EAX] = 0x12345678;
    cpu.step(&mut bus, &mut hooks).unwrap();
    let f = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        f.contains(EFlags::ZF),
        "CMP EAX, Imm32 (0x81) failed to set Zero Flag"
    );

    // Step 3: MOV [EBX], 0xDEADBEEF
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

    // INC and DEC are famous in x86 for modifying Zero, Sign, Parity, and Overflow,
    // but INTENTIONALLY LEAVING CARRY (CF) UNTOUCHED. This allows them to be used in ADC/SBB loops.

    // 0x40 = INC EAX
    // 0x4B = DEC EBX
    bus.write_bytes(0x1000, &[0x40, 0x4B]).unwrap();
    cpu.regs[REG_EIP] = 0x1000;

    // Test INC wrapping 0xFFFFFFFF -> 0. Normally this causes a Carry.
    cpu.regs[REG_EAX] = 0xFFFFFFFF;

    // Set CF = 1 manually
    let mut flags = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    flags.insert(EFlags::CF);
    cpu.regs[REG_EFLAGS] = flags.bits();

    cpu.step(&mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[REG_EAX], 0, "INC failed to wrap to 0");
    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f1.contains(EFlags::ZF), "INC should set Zero Flag here");
    assert!(
        f1.contains(EFlags::CF),
        "INC MUST PRESERVE the Carry Flag! (CF should still be 1)"
    );

    // Test DEC wrapping 0 -> 0xFFFFFFFF. Normally this causes a Borrow (Carry).
    cpu.regs[REG_EBX] = 0;

    // Set CF = 0 manually
    let mut flags2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    flags2.remove(EFlags::CF);
    cpu.regs[REG_EFLAGS] = flags2.bits();

    cpu.step(&mut bus, &mut hooks).unwrap();

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

    // TEST EAX, EAX
    // This performs EAX & EAX, updating flags but discarding the result.
    let instr = Instr::Test {
        dest: Operand::Reg(GpReg::Eax),
        src: Operand::Reg(GpReg::Eax),
    };

    cpu.regs[REG_EAX] = 0;

    execute_instr(&mut cpu, instr.clone(), &mut bus, &mut HookRegistry::new()).unwrap();

    let f1 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(f1.contains(EFlags::ZF), "TEST 0, 0 should set Zero Flag");
    assert!(!f1.contains(EFlags::SF), "TEST 0, 0 should clear Sign Flag");

    cpu.regs[REG_EAX] = 0x80000000;

    execute_instr(&mut cpu, instr, &mut bus, &mut HookRegistry::new()).unwrap();

    let f2 = EFlags::from_bits_retain(cpu.regs[REG_EFLAGS]);
    assert!(
        !f2.contains(EFlags::ZF),
        "TEST 0x80000000, 0x80000000 should clear Zero Flag"
    );
    assert!(
        f2.contains(EFlags::SF),
        "TEST 0x80000000, 0x80000000 should set Sign Flag (MSB is 1)"
    );
    assert_eq!(
        cpu.regs[REG_EAX], 0x80000000,
        "TEST must not modify the destination register"
    );
}
