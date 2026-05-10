use crate::device::Device;
use crate::error::EmuError;

pub struct Stm32Gpio {
    registers: [u32; 8],
}

impl Stm32Gpio {
    pub const fn new() -> Self {
        Self::new_with_pins([0; _])
    }

    pub const fn new_with_pins(pins: [u32; 8]) -> Self {
        Self { registers: pins }
    }

    pub const fn is_odr_pin_high(&self, pin: u8) -> bool {
        (self.registers[5] & (1 << pin)) != (1 << pin)
    }

    pub const fn is_moder_pin_high(&self, pin: u8) -> bool {
        (self.registers[0] & (1 << pin)) != (1 << pin)
    }

    pub const fn is_led_on(&self) -> bool {
        // MODER (offset 0x00 / index 0) bit 10 must be 1 (0x400)
        // ODR (offset 0x14 / index 5) bit 5 must be 1 (0x20)
        // self.registers[5] & 0x20 == 0x20 && self.registers[0] & 0x400 == 0x400
        self.is_moder_pin_high(10) && self.is_odr_pin_high(5)
    }

    pub const fn set_idr_pin(&mut self, pin: u8, is_high: bool) {
        // IDR is offset 0x10, which is index 4 in your array
        if is_high {
            self.registers[4] |= 1 << pin;
        } else {
            self.registers[4] &= !(1 << pin);
        }
    }

    pub const fn is_idr_pin_high(&self, pin: u8) -> bool {
        (self.registers[4] & (1 << pin)) != (1 << pin)
    }
}

impl Device for Stm32Gpio {
    #[inline(always)]
    fn read_32(&mut self, offset: u64) -> Result<u32, EmuError> {
        let index = (offset / 4) as usize;
        Ok(self.registers.get(index).copied().unwrap_or(0))
    }

    #[inline(always)]
    fn write_32(&mut self, offset: u64, val: u32) -> Result<(), EmuError> {
        let index = (offset / 4) as usize;
        if let Some(reg) = self.registers.get_mut(index) {
            *reg = val;
        }
        Ok(())
    }

    #[inline(always)]
    fn read_16(&mut self, offset: u64) -> Result<u16, EmuError> {
        let index = (offset / 4) as usize;
        if index < self.registers.len() {
            let reg_val = self.registers[index];
            // Determine if they are reading the lower or upper 16 bits
            let bit_shift = (offset % 4) * 8;
            Ok((reg_val >> bit_shift) as u16)
        } else {
            Ok(0)
        }
    }

    #[inline(always)]
    fn write_16(&mut self, offset: u64, val: u16) -> Result<(), EmuError> {
        let index = (offset / 4) as usize;
        if index < self.registers.len() {
            let bit_shift = (offset % 4) * 8;
            let mask = !(0xFFFFu32 << bit_shift); // Clear the 16 bits we are targeting

            self.registers[index] = (self.registers[index] & mask) | ((val as u32) << bit_shift);
        }
        Ok(())
    }

    #[inline(always)]
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        let index = (offset / 4) as usize;
        if index < self.registers.len() {
            let reg_val = self.registers[index];
            // Determine exactly which byte (0, 1, 2, or 3) is being requested
            let bit_shift = (offset % 4) * 8;
            Ok((reg_val >> bit_shift) as u8)
        } else {
            Ok(0)
        }
    }

    #[inline(always)]
    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        let index = (offset / 4) as usize;
        if index < self.registers.len() {
            let bit_shift = (offset % 4) * 8;
            let mask = !(0xFFu32 << bit_shift); // Clear the 8 bits we are targeting 

            self.registers[index] = (self.registers[index] & mask) | ((val as u32) << bit_shift);
        }
        Ok(())
    }
}
