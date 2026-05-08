# Project `hyperemu`: Full-Fledged Architectural & Implementation Plan

This document serves as the absolute source of truth for the `hyperemu` project. It is designed to take a human developer (or an AI agent) from zero domain knowledge to a complete understanding of how to build a production-grade, multi-architecture hardware emulator in Rust. 

There is no room for guessing; every subsystem, trait, dependency, and execution flow is explicitly defined.

---

## Part 1: Crash Course in Emulation Concepts
*If you already understand CPU architecture, skip this section. This is for beginners or AI context priming.*

An emulator is a software program that pretends to be a hardware motherboard. It executes binary code intended for a different computer (e.g., running an ARM chip's code on an Intel chip). 

To build one, you must simulate the following physical components:
1. **The CPU (Central Processing Unit):** Contains internal memory slots called **Registers** (e.g., `r0`, `r1`). One special register is the **Program Counter (PC)**, which holds the memory address of the next instruction to run. The CPU fetches instructions, decodes what they mean (e.g., "Add `r1` to `r2`"), and executes them.
2. **The Memory Bus (Interconnect):** The CPU knows nothing about RAM chips or USB ports. It only knows how to ask the Bus: *"Read 4 bytes at address `0x1000`"*. The Bus looks at a map and routes that request to the correct physical device.
3. **Devices (RAM, ROM, Peripherals):** 
    *   **RAM** simply stores and returns data. 
    *   **MMIO (Memory-Mapped I/O):** Hardware peripherals (like a UART terminal or LEDs) are connected to the Bus just like RAM. If the CPU writes a byte to the specific address `0x4000_0000`, it doesn't store the byte in memory; instead, the UART hardware physically sends that byte over a wire.
4. **Syscalls (OS Emulation / HLE):** A CPU does not understand `printf` or `malloc`. When a C program calls `printf("Hello")`, the compiler translates this to: 1. Put the string's address in register `r1`. 2. Put a "Syscall Number" in `r0`. 3. Execute a special instruction called `SVC` (Supervisor Call). The CPU traps this, pauses, and asks the Operating System to handle it. In our emulator, we will intercept `SVC` to simulate the OS.

---

## Part 2: Third-Party Dependencies

To avoid reinventing the wheel and to keep the codebase focused on emulation, we will use the following Rust crates. All external crates are hidden behind feature flags except `thiserror` and `bitflags`.

| Crate Name | Version | Optional? | Justification for Inclusion |
| :--- | :--- | :--- | :--- |
| **`bitflags`** | `2.11` | No | Hardware relies heavily on bit manipulation (e.g., CPU Modes, Memory Permissions). This crate provides safe, C-compatible bitfield structures. |
| **`thiserror`** | `2.0` | No | Eliminates boilerplate when defining the standard `EmuError` enum. Makes error propagation `?` clean and idiomatic. |
| **`goblin`** | `0.10` | Yes (`feature="elf"`) | Parsing ELF/PE/Mach-O executable headers from scratch is notoriously error-prone. Goblin is the Rust standard for parsing binary file formats safely. |
| **`gdbstub`** | `0.7` | Yes (`feature="gdb"`) | Implements the GDB Remote Serial Protocol (RSP). Allows users to attach standard `gdb` debuggers to our emulator for free without us writing socket code. |
| **`log`** | `0.4` | No | Standard logging facade. Essential for tracing instruction execution, memory faults, and debugging the emulator. |

---

## Part 3: Core Architectural Blueprint

### 3.1 The Rust "Borrow Checker" Solution
In emulators, the CPU needs memory, memory needs peripherals, and sometimes peripherals need to interrupt the CPU. This creates circular references. **We will solve this by ensuring the CPU owns nothing but its internal registers.** 
The `HyperEmu` wrapper owns the CPU, the Bus, and the Hooks. During execution, it temporarily lends the Bus and Hooks to the CPU.

### 3.2 Directory Structure
```text
src/
├── lib.rs              # Public exports, #[cfg] feature rules
├── config.rs           # Arch, CpuMode (C-compatible u32 bitflags)
├── error.rs            # EmuError enum using thiserror
├── bus.rs              # MemoryBus, MemoryRegion, Permissions
├── device/             
│   ├── mod.rs          # Device trait 
│   ├── ram.rs          # Basic RAM implementation
|   ├── gpio.rs         # Basic GPIO 
│   └── uart.rs         # Basic MMIO UART console
├── hook.rs             # HookRegistry, CodeHook, MemHook definitions
├── loader/             
│   ├── mod.rs          # ExecutableLoader trait
│   ├── raw.rs          # Raw .bin loading into RAM
│   └── elf.rs          # #[cfg(feature = "elf")] Goblin implementation
├── interface.rs        # Cpu trait definition
├── emu.rs              # HyperEmu wrapper struct
├── gdb.rs              # #[cfg(feature = "gdb")] Target implementation
└── arch/               
    ├── mod.rs          
    └── armv7/             # #[cfg(feature = "armv7")]
        ├── mod.rs      
        ├── registers.rs   # R0-R15, CPSR state
        ├── instr.rs       # Instruction definitions
        ├── decode.rs      # Bit-masking instruction decoder
        ├── execute.rs     # Instruction execution implementations
        └── tests.rs       # ARMv7 CPU tests
```

---

## Part 4: Code Interfaces (The Strict Guidelines)

This section dictates exactly what the core traits and structs look like. **Do not deviate from these interfaces**, as they are designed for C-FFI compatibility and strict borrow-checker adherence.

### 4.1 Configuration (`config.rs`)
Must be heavily C-compatible (`#[repr(C)]`).
```rust
#[repr(C)]
pub enum Arch { Armv7 = 1, X86 = 2 }

bitflags::bitflags! {
    #[repr(C)]
    pub struct CpuMode: u32 {
        const LITTLE_ENDIAN = 0;
        const BIG_ENDIAN    = 1 << 30;
        const MODE_32       = 1 << 2;
        const THUMB         = 1 << 4; // ARM specific
    }
}
```

### 4.2 The Interconnect (`device/mod.rs` & `bus.rs`)
Devices receive an **offset**, not the absolute address. The Bus handles the translation.
```rust
pub trait Device {
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError>;
    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError>;
    // Includes default 16, 32, 64, and read_bytes implementations...
}

pub struct MemoryBus {
    regions: Vec<MemoryRegion>, // MemoryRegion contains `start, size, Box<dyn Device>`
}
impl MemoryBus {
    pub fn map(&mut self, start: u64, size: u64, device: Box<dyn Device>);
    // CPU calls these. Bus finds correct region, subtracts `start` to get `offset`, calls Device.
    pub fn read_32(&mut self, addr: u64) -> Result<u32, EmuError>; 
}
```

### 4.3 The CPU Interface (`interface.rs`)
```rust
pub trait Cpu {
    fn init(mode: CpuMode) -> Result<Self, EmuError> where Self: Sized;
    /// The most important method. Takes mutable references to the bus and hooks.
    fn step(&mut self, bus: &mut MemoryBus, hooks: &mut HookRegistry) -> Result<(), EmuError>;
    fn read_reg(&self, reg_id: usize) -> Result<u64, EmuError>;
    fn write_reg(&mut self, reg_id: usize, val: u64) -> Result<(), EmuError>;
    fn pc(&self) -> u64;
}
```

---

## Part 5: Handling Standard OS Features & Peripherals

### 5.1 High-Level Emulation: `malloc` & `printf`
We will not run a full Linux kernel in this emulator. Instead, we use **Syscall Interception**.
1. We define a hook: `hooks.add_interrupt_hook(...)`.
2. The user compiles their C code using the standard `arm-none-eabi-gcc` toolchain.
3. When the C code calls `printf("Hello")`, it reaches an `SVC` (Supervisor Call) instruction.
4. The `Armv7Cpu` executes `SVC`, which triggers the Interrupt Hook.
5. Inside the Rust hook:
    * We read the syscall number from Register `R0`.
    * If `R0 == SYS_WRITE`, we read Register `R1` (pointer to the string buffer), read the bytes from `MemoryBus`, and use Rust's native `println!()`.
    * If `R0 == SYS_SBRK` (used by `malloc` to ask for memory), we find the current end of RAM, use `bus.map()` to dynamically add more `Ram`, and return the new address in `R0`.

### 5.2 Peripheral Injection (LEDs, UART)
* **UART Console:** The library provides a `Uart` struct implementing `Device`. If mapped to `0x4000_0000`, any write to that address prints a character to the host terminal.
* **Custom LEDs / Hardware:** The consumer of our library will create a struct `MyBoardLeds`, implement the `Device` trait, and map it. 
  ```rust
  // Consumer's code
  emu.mem_map(0x2000, 4, Perms::RW, Box::new(MyBoardLeds::new()));
  ```
  If the C program does `*(volatile uint32_t*)0x2000 = 1;`, our `MemoryBus` routes it directly to the user's Rust code.

---

## Part 6: Step-by-Step Implementation Guide

If you are an AI agent or developer starting the code generation phase, execute the project strictly in the following phases. **Do not jump to CPU decoding before the bus is flawless.**

### Phase 1: The Foundation
1. Initialize Cargo project. Add `bitflags` and `thiserror`. Setup `[features]` in `Cargo.toml`.
2. Implement `error.rs`.
3. Implement `config.rs` (`Arch` and `CpuMode`).
4. Implement `device/mod.rs` (The `Device` trait).
5. Implement `device/ram.rs` (Standard memory).
6. Implement `bus.rs` (The `MemoryBus` and region mapping logic).

### Phase 2: Execution Wrapping & Hooks
1. Implement `hook.rs`. Create the `HookRegistry` capable of storing closures.
2. Implement `interface.rs` (The `Cpu` trait).
3. Implement `emu.rs` (The `HyperEmu`). It must wrap `Box<dyn Cpu>`, `MemoryBus`, and `HookRegistry`. Add helper functions like `emu.mem_map()`, `emu.reg_write()`.

### Phase 3: The Loader Subsystem
1. Include `goblin` dependency under the `elf` feature.
2. Implement `loader/mod.rs` (The `ExecutableLoader` trait returning Entry Point and Stack Point).
3. Implement `loader/raw.rs` (Basic binary blob copy).
4. Implement `loader/elf.rs`. Use `goblin` to parse `PT_LOAD` segments, dynamically map `Ram` devices on the `MemoryBus` at `p_vaddr`, and copy segment data. Return the `e_entry` address.

### Phase 4: ARMv7 Core Architecture
1. Create `arch/armv7/mod.rs` and `registers.rs`. Define `struct Armv7Cpu` with `regs: [u32; 16]` and `cpsr: u32`. Implement the `Cpu` trait.
2. Implement the `step()` function skeleton. It must:
    * Trigger `hooks.trigger_code(self.pc())`.
    * Fetch: `let instr = bus.read_32(self.pc())?`.
    * Increment PC: `self.write_reg(15, self.pc() + 4)`.
3. Create `decode.rs`. Write bitmasking functions to identify standard ARM instructions (e.g., Data Processing, Branch, Load/Store).
4. Create `execute.rs`. Implement standard instructions (`MOV`, `ADD`, `SUB`, `CMP`, `B`, `LDR`, `STR`, `PUSH`, `POP`, `SVC`). *Only implement enough to run standard C logic, leaving complex coprocessor instructions for later.*

### Phase 5: GDB Integration (Final Polish)
1. Include `gdbstub` under the `gdb` feature.
2. Create `gdb.rs`. Implement `gdbstub::Target` for `HyperEmu`. 
3. Wire GDB's read/write memory requests to `emu.bus.read_bytes()`. Wire GDB's register requests to `emu.cpu.read_reg()`. Add a breakpoint `HashSet` to `HyperEmu` to pause the run loop.

---

## Part 7: Final Expectation Summary
By following this plan, you will produce a library that allows a user to write the following 10 lines of Rust code to emulate a complete system:

```rust
let mut emu = HyperEmu::new(Arch::Armv7, CpuMode::MODE_32 | CpuMode::LITTLE_ENDIAN)?;

// Load a compiled C program
let binary = std::fs::read("my_firmware.elf")?;
let load_info = emu.load_elf(&binary)?;

// Map a custom peripheral (e.g., LEDs)
emu.mem_map(0x4000_0000, 0x1000, Perms::RW, Box::new(MyLedBoard::new()));

// Run!
emu.start(load_info.entry_point, 0)?; 
```
