use crate::device::Device;
use crate::error::EmuError;
use std::sync::{Arc, Mutex};

/// A simulated GPIO (General Purpose Input/Output) port controlling 8 LEDs.
pub struct GpioPort {
    /// We wrap the LED states in an Arc<Mutex> so that the emulator
    /// can write to it, and a separate GUI thread can read from it to draw the screen!
    pub leds: Arc<Mutex<u8>>,
}

impl GpioPort {
    pub fn new(led_state: Arc<Mutex<u8>>) -> Self {
        Self { leds: led_state }
    }
}

impl Device for GpioPort {
    fn read_8(&mut self, offset: u64) -> Result<u8, EmuError> {
        match offset {
            // Address offset 0: Return the current state of the LEDs
            0 => {
                let state = *self.leds.lock().unwrap();
                Ok(state)
            }
            _ => Err(EmuError::DeviceError(format!(
                "Invalid read from GPIO at offset {}",
                offset
            ))),
        }
    }

    fn write_8(&mut self, offset: u64, val: u8) -> Result<(), EmuError> {
        match offset {
            // Address offset 0: Update the LEDs!
            0 => {
                let mut state = self.leds.lock().unwrap();

                *state = val;
                Ok(())
            }
            _ => Err(EmuError::DeviceError(format!(
                "Invalid write to GPIO at offset {}",
                offset
            ))),
        }
    }
}
