use usb_device::class_prelude::*;
use usb_device::device::UsbDevice;
use usbd_serial::SerialPort;
use crate::intr::InterruptHandle;


pub struct SerialUsbDevice<'a, B: UsbBus> {
    device: UsbDevice<'a, B>,
    serial: SerialPort<'a, B>,
    interrupt: InterruptHandle,
}

impl<'a, B: UsbBus> SerialUsbDevice<'a, B> {
    pub fn new(device: UsbDevice<'a, B>, serial: SerialPort<'a, B>, interrupt: InterruptHandle) -> Self {
        Self {
            device,
            serial,
            interrupt,
        }
    }

    async fn poll(&mut self) {
        if self.device.poll(&mut [&mut self.serial]) {
            return;
        }
        self.interrupt.wait().await;
    }

    pub async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, UsbError> {
        loop {
            match self.serial.read(buffer) {
                Ok(size) => return Ok(size),
                Err(UsbError::WouldBlock) => {
                    self.poll().await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), UsbError> {
        let mut offset = 0;
        loop {
            match self.serial.write(&data[offset..]) {
                Ok(size) => {
                    offset += size;
                    if offset == data.len() {
                        return Ok(());
                    }
                },
                Err(UsbError::WouldBlock) => {
                    self.poll().await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
