use crate::bus::MemoryBus;
use crate::error::EmuError;
use crate::interface::Cpu;

pub trait CodeHook {
    fn on_code(&mut self, cpu: &mut dyn Cpu, bus: &mut MemoryBus, pc: u64) -> Result<(), EmuError>;
}

impl<F> CodeHook for F
where
    F: FnMut(&mut dyn Cpu, &mut MemoryBus, u64) -> Result<(), EmuError>,
{
    fn on_code(&mut self, cpu: &mut dyn Cpu, bus: &mut MemoryBus, pc: u64) -> Result<(), EmuError> {
        self(cpu, bus, pc)
    }
}

pub trait InterruptHook {
    fn on_interrupt(
        &mut self,
        cpu: &mut dyn Cpu,
        bus: &mut MemoryBus,
        int_no: u32,
    ) -> Result<bool, EmuError>;
}

impl<F> InterruptHook for F
where
    F: FnMut(&mut dyn Cpu, &mut MemoryBus, u32) -> Result<bool, EmuError>,
{
    fn on_interrupt(
        &mut self,
        cpu: &mut dyn Cpu,
        bus: &mut MemoryBus,
        int_no: u32,
    ) -> Result<bool, EmuError> {
        self(cpu, bus, int_no)
    }
}

pub struct HookRegistry {
    /// Code hooks run on every instruction fetch. They cannot short-circuit each other,
    /// because you might want a Tracer, a Profiler, and a Debugger all watching the same PC.
    pub code_hooks: Vec<Box<dyn CodeHook>>,

    /// Interrupt hooks run when the CPU hits an exception (SVC, BKPT).
    /// They return `bool`. If a hook returns `true`, it has "consumed" the interrupt,
    /// and subsequent hooks will NOT be executed.
    pub interrupt_hooks: Vec<Box<dyn InterruptHook>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    pub const fn new() -> Self {
        Self {
            code_hooks: Vec::new(),
            interrupt_hooks: Vec::new(),
        }
    }

    pub fn add_code_hook<Hook>(&mut self, hook: Hook)
    where
        Hook: CodeHook + 'static,
    {
        self.code_hooks.push(Box::new(hook));
    }

    pub fn add_interrupt_hook<Hook>(&mut self, hook: Hook)
    where
        Hook: InterruptHook + 'static,
    {
        self.interrupt_hooks.push(Box::new(hook));
    }

    #[inline(always)]
    pub fn trigger_code(
        &mut self,
        cpu: &mut dyn Cpu,
        bus: &mut MemoryBus,
        pc: u64,
    ) -> Result<(), EmuError> {
        for hook in &mut self.code_hooks {
            hook.on_code(cpu, bus, pc)?;
        }
        Ok(())
    }

    /// Triggers the interrupt hooks sequentially.
    /// Returns `Ok(true)` if a hook consumed the interrupt, or `Ok(false)` if it was unhandled.
    #[inline(always)]
    pub fn trigger_interrupt(
        &mut self,
        cpu: &mut dyn Cpu,
        bus: &mut MemoryBus,
        int_no: u32,
    ) -> Result<bool, EmuError> {
        for hook in &mut self.interrupt_hooks {
            let handled = hook.on_interrupt(cpu, bus, int_no)?;
            if handled {
                return Ok(true); // Short-circuit: The hook consumed the event
            }
        }
        Ok(false) // No hook claimed this interrupt
    }
}
