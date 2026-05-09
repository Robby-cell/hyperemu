use super::Armv7Cpu;
use super::decode::decode_arm;
use super::execute::execute_instr;
use super::instr::{Condition, Instr, Operand2, Shift, ShiftType};
use crate::bus::{MemoryBus, Perms};
use crate::config::CpuMode;
use crate::device::ram::Ram;
use crate::hook::HookRegistry;
use crate::interface::Cpu;

fn setup_test_env() -> (Armv7Cpu, MemoryBus, HookRegistry) {
    let cpu = Armv7Cpu::init(CpuMode::MODE_32).unwrap();
    let mut bus = MemoryBus::new();
    let hooks = HookRegistry::new();

    bus.map(0x1000, 0x1000, Perms::RWX, Ram::new(0x1000).into());

    (cpu, bus, hooks)
}

#[test]
fn test_alu_add_flags() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr = Instr::Add {
        cond: Condition::Al,
        s: true,
        rd: 0,
        rn: 1,
        op2: Operand2::Register {
            rm: 2,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsl,
                amount: 0,
            },
        },
    };

    cpu.regs[1] = 5;
    cpu.regs[2] = 10;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 15);
    assert_eq!(cpu.get_z(), false);
    assert_eq!(cpu.get_n(), false);
    assert_eq!(cpu.get_c(), false);
    assert_eq!(cpu.get_v(), false);

    let instr_carry = Instr::Add {
        cond: Condition::Al,
        s: true,
        rd: 0,
        rn: 1,
        op2: Operand2::Immediate {
            val: 2,
            carry_out: None,
        },
    };
    cpu.regs[1] = 0xFFFFFFFF;
    execute_instr(&mut cpu, instr_carry, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 1);
    assert_eq!(cpu.get_c(), true, "Carry flag should be set");
    assert_eq!(cpu.get_z(), false);

    let instr_overflow = Instr::Add {
        cond: Condition::Al,
        s: true,
        rd: 0,
        rn: 1,
        op2: Operand2::Immediate {
            val: 1,
            carry_out: None,
        },
    };
    cpu.regs[1] = 0x7FFFFFFF;
    execute_instr(&mut cpu, instr_overflow, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0x80000000);
    assert_eq!(cpu.get_v(), true, "Overflow flag should be set");
    assert_eq!(cpu.get_n(), true, "Negative flag should be set");
}

#[test]
fn test_alu_sub_flags() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr = Instr::Sub {
        cond: Condition::Al,
        s: true,
        rd: 0,
        rn: 1,
        op2: Operand2::Immediate {
            val: 5,
            carry_out: None,
        },
    };

    cpu.regs[1] = 10;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 5);
    assert_eq!(cpu.get_c(), true, "Carry should be 1 (No borrow occurred)");

    let instr_borrow = Instr::Sub {
        cond: Condition::Al,
        s: true,
        rd: 0,
        rn: 1,
        op2: Operand2::Immediate {
            val: 10,
            carry_out: None,
        },
    };
    cpu.regs[1] = 5;
    execute_instr(&mut cpu, instr_borrow, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0xFFFFFFFB);
    assert_eq!(cpu.get_n(), true, "Negative flag should be set");
    assert_eq!(cpu.get_c(), false, "Carry should be 0 (Borrow occurred)");
}

#[test]
fn test_shifts() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr_lsl = Instr::Mov {
        cond: Condition::Al,
        s: true,
        rd: 0,
        op2: Operand2::Register {
            rm: 1,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsl,
                amount: 2,
            },
        },
    };

    cpu.regs[1] = 0x00000003;
    execute_instr(&mut cpu, instr_lsl, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[0], 12);

    let instr_lsr_0 = Instr::Mov {
        cond: Condition::Al,
        s: true,
        rd: 0,
        op2: Operand2::Register {
            rm: 1,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsr,
                amount: 0,
            },
        },
    };
    cpu.regs[1] = 0x80000000;
    execute_instr(&mut cpu, instr_lsr_0, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[0], 0);
    assert_eq!(cpu.get_c(), true, "Carry out should be original MSB (1)");
    assert_eq!(cpu.get_z(), true, "Result is 0, so Z flag set");
}

#[test]
fn test_push_pop_stack() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    cpu.regs[13] = 0x1100;

    cpu.regs[0] = 0xAAAA;
    cpu.regs[1] = 0xBBBB;
    cpu.regs[2] = 0xCCCC;

    let push_instr = Instr::Stm {
        cond: Condition::Al,
        rn: 13,
        reg_list: 0b0111,
        p: true,
        u: false,
        w: true,
    };

    execute_instr(&mut cpu, push_instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[13], 0x1100 - 0xC);

    assert_eq!(bus.read_32(0x1100 - 0xC).unwrap(), 0xAAAA);
    assert_eq!(bus.read_32(0x1100 - 0x8).unwrap(), 0xBBBB);
    assert_eq!(bus.read_32(0x1100 - 0x4).unwrap(), 0xCCCC);

    cpu.regs[0] = 0;
    cpu.regs[1] = 0;
    cpu.regs[2] = 0;

    let pop_instr = Instr::Ldm {
        cond: Condition::Al,
        rn: 13,
        reg_list: 0b0111,
        p: false,
        u: true,
        w: true,
    };

    execute_instr(&mut cpu, pop_instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0xAAAA);
    assert_eq!(cpu.regs[1], 0xBBBB);
    assert_eq!(cpu.regs[2], 0xCCCC);
    assert_eq!(
        cpu.regs[13], 0x1100,
        "SP should be restored to original address"
    );
}

#[test]
fn test_conditional_execution() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // ADDEQ r0, r1, r2 (Add ONLY if the Zero flag is set)
    let instr_eq = Instr::Add {
        cond: Condition::Eq,
        s: false,
        rd: 0,
        rn: 1,
        op2: Operand2::Register {
            rm: 2,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsl,
                amount: 0,
            },
        },
    };

    cpu.regs[1] = 10;
    cpu.regs[2] = 20;

    // Run with Z = 0 (Condition Fails)
    cpu.set_z(false);
    execute_instr(&mut cpu, instr_eq, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[0], 0, "Instruction should NOT have executed");

    // Re-create instruction (since it was moved), Run with Z = 1 (Condition Passes)
    let instr_eq = Instr::Add {
        cond: Condition::Eq,
        s: false,
        rd: 0,
        rn: 1,
        op2: Operand2::Register {
            rm: 2,
            shift: Shift::Immediate {
                shift_type: ShiftType::Lsl,
                amount: 0,
            },
        },
    };
    cpu.set_z(true);
    execute_instr(&mut cpu, instr_eq, &mut bus, &mut hooks).unwrap();
    assert_eq!(cpu.regs[0], 30, "Instruction SHOULD have executed");
}

#[test]
fn test_umull_64bit_multiply() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // UMULL r0, r1, r2, r3 (r1:r0 = r2 * r3)
    let instr = Instr::Umull {
        cond: Condition::Al,
        s: false,
        rd_lo: 0,
        rd_hi: 1,
        rm: 2,
        rs: 3,
    };

    // 0xFFFFFFFF * 0xFFFFFFFF = 0xFFFFFFFE_00000001
    cpu.regs[2] = 0xFFFFFFFF;
    cpu.regs[3] = 0xFFFFFFFF;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0x00000001, "Low 32-bits incorrect");
    assert_eq!(cpu.regs[1], 0xFFFFFFFE, "High 32-bits incorrect");
}

#[test]
fn test_rev_endian_swap() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    let instr = Instr::Rev {
        cond: Condition::Al,
        rd: 0,
        rm: 1,
    };

    cpu.regs[1] = 0x11223344;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0x44332211, "Byte reversal failed");
}

#[test]
fn test_ubfx_bitfield_extract() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // UBFX r0, r1, #4, #4 (Extract 4 bits starting at bit 4)
    let instr = Instr::Ubfx {
        cond: Condition::Al,
        rd: 0,
        rn: 1,
        lsb: 4,
        width: 4,
    };

    // 0xABCD = 1010_1011_1100_1101
    // Bits 4 to 7 are 1100 (0xC)
    cpu.regs[1] = 0xABCD;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(
        cpu.regs[0], 0x0000000C,
        "Unsigned bitfield extraction failed"
    );
}

#[test]
fn test_sbfx_bitfield_extract_signed() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // SBFX r0, r1, #4, #4 (Extract 4 bits starting at bit 4, Sign Extend)
    let instr = Instr::Sbfx {
        cond: Condition::Al,
        rd: 0,
        rn: 1,
        lsb: 4,
        width: 4,
    };

    // 0xABFD = 1010_1011_1111_1101
    // Bits 4 to 7 are 1111 (0xF). As a 4-bit signed number, 0xF is -1.
    // Sign extended to 32 bits, this should become 0xFFFFFFFF.
    cpu.regs[1] = 0xABFD;
    execute_instr(&mut cpu, instr, &mut bus, &mut hooks).unwrap();

    assert_eq!(cpu.regs[0], 0xFFFFFFFF, "Signed bitfield extraction failed");
}

#[test]
fn test_real_world_syscall_interception() {
    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Let's pretend a C program wants to print "Hello" using SYS_WRITE (Syscall #4).
    // It places the string in RAM, sets R0 to the Syscall No, R1 to the string pointer,
    // and R2 to the length.

    // Put the string "Hello" in RAM at address 0x1100
    bus.write_bytes(0x1100, b"Hello").unwrap();

    // Set up the registers exactly as standard `newlib` does
    cpu.regs[0] = 4; // SYS_WRITE
    cpu.regs[1] = 0x1100; // Pointer to buffer
    cpu.regs[2] = 5; // Length of string

    // Write an SVC instruction into the execution path
    // SVC #0
    bus.write_32(0x1000, 0xEF000000).unwrap();
    cpu.regs[15] = 0x1000;

    // Create our OS Intercept Hook
    let captured_string = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured_string_clone = std::sync::Arc::clone(&captured_string);

    hooks.add_interrupt_hook(
        move |hook_cpu: &mut dyn Cpu, hook_bus: &mut MemoryBus, _imm: u32| {
            let r0 = hook_cpu.read_reg(0)?;

            if r0 == 4 {
                // Intercept SYS_WRITE
                let buf_ptr = hook_cpu.read_reg(1)?;
                let length = hook_cpu.read_reg(2)?;

                // Read the string directly out of the emulated memory
                let mut string_buf = vec![0u8; length as usize];
                hook_bus.read_bytes(buf_ptr, &mut string_buf)?;

                let result_str = String::from_utf8_lossy(&string_buf).into_owned();
                *captured_string_clone.lock().unwrap() = result_str;

                // Write the return code (success) back to r0
                hook_cpu.write_reg(0, 0)?;
            }
            Ok(true)
        },
    );

    // Execute the SVC instruction
    execute_instr(
        &mut cpu,
        Instr::Svc {
            cond: Condition::Al,
            imm: 0,
        },
        &mut bus,
        &mut hooks,
    )
    .unwrap();

    // Verify our OS Hook successfully extracted the C program's data
    assert_eq!(*captured_string.lock().unwrap(), "Hello");
    assert_eq!(cpu.regs[0], 0, "Syscall return value should be 0");
}

#[test]
fn test_gui_led_blinking() {
    use crate::device::gpio::GpioPort;
    use std::sync::{Arc, Mutex};

    let (mut cpu, mut bus, mut hooks) = setup_test_env();

    // Create the shared GUI State
    let gui_led_state = Arc::new(Mutex::new(0u8));

    // Map the GPIO device to memory address 0x4000_0000
    // We pass a clone of the Arc to the device so both the CPU and the GUI own it.
    let gpio_device = Box::new(GpioPort::new(Arc::clone(&gui_led_state)));
    bus.map(0x4000_0000, 0x1000, Perms::RW, gpio_device.into());

    // Write an Assembly Program to blink the 1st LED (bit 0)
    /*
        _start:
            LDR r1, =0x40000000  ; Base address of GPIO
            MOV r2, #1           ; LED ON (bit 0 = 1)
            MOV r3, #0           ; LED OFF (bit 0 = 0)

        _loop:
            STRB r2, [r1]        ; Turn LED ON
            STRB r3, [r1]        ; Turn LED OFF
            B _loop              ; Repeat
    */
    let code: [u32; 6] = [
        0xE3A01440, // MOV r1, #0x40000000 (Simplified LDR for testing: actually MOV r1, 0x40 rotated...)
        0xE3A02001, // MOV r2, #1
        0xE3A03000, // MOV r3, #0
        0xE5C12000, // STRB r2, [r1]
        0xE5C13000, // STRB r3, [r1]
        0xEAFFFFFC, // B _loop (Branch back 2 instructions)
    ];

    for (i, &word) in code.iter().enumerate() {
        bus.write_32(0x1000 + (i as u64 * 4), word).unwrap();
    }
    cpu.regs[15] = 0x1000;

    // Simulate a few clock cycles and watch the GUI state change

    // Setup instructions
    let instr = bus.read_32(cpu.regs[15] as u64).unwrap();
    execute_instr(&mut cpu, decode_arm(instr), &mut bus, &mut hooks).unwrap();
    cpu.regs[15] += 4;

    let instr = bus.read_32(cpu.regs[15] as u64).unwrap();
    execute_instr(&mut cpu, decode_arm(instr), &mut bus, &mut hooks).unwrap();
    cpu.regs[15] += 4;

    let instr = bus.read_32(cpu.regs[15] as u64).unwrap();
    execute_instr(&mut cpu, decode_arm(instr), &mut bus, &mut hooks).unwrap();
    cpu.regs[15] += 4;

    // Execute STRB r2, [r1] (Turn LED ON)
    let instr = bus.read_32(cpu.regs[15] as u64).unwrap();
    execute_instr(&mut cpu, decode_arm(instr), &mut bus, &mut hooks).unwrap();
    cpu.regs[15] += 4;

    // GUI thread checks the screen...
    assert_eq!(
        *gui_led_state.lock().unwrap(),
        1,
        "GUI should see LED turned ON!"
    );

    // Execute STRB r3, [r1] (Turn LED OFF)
    let instr = bus.read_32(cpu.regs[15] as u64).unwrap();
    execute_instr(&mut cpu, decode_arm(instr), &mut bus, &mut hooks).unwrap();
    cpu.regs[15] += 4;
    let _ = cpu.regs[15];

    // GUI thread checks the screen...
    assert_eq!(
        *gui_led_state.lock().unwrap(),
        0,
        "GUI should see LED turned OFF!"
    );
}
