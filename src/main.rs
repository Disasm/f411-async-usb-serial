#![no_main]
#![no_std]

pub mod intr;
mod serial;

use panic_rtt_target as _;

use async_embedded::task;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32f4xx_hal::prelude::*;
use stm32f4xx_hal::stm32;
use stm32f4xx_hal::stm32::{interrupt, Interrupt};
use crate::intr::InterruptObject;
use stm32f4xx_hal::otg_fs::{USB, UsbBus};
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};
use crate::serial::SerialUsbDevice;

static OTG_FS_OBJ: InterruptObject = InterruptObject::new(Interrupt::OTG_FS);

#[interrupt]
fn OTG_FS() {
    OTG_FS_OBJ.handle_interrupt();
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();

    // Apply sleep-mode workaround to enable RTT connection
    dp.DBGMCU.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });
    dp.RCC.ahb1enr.modify(|_, w| w.dma1en().enabled());

    let rcc = dp.RCC.constrain();
    // let _clocks = rcc
    //     .cfgr
    //     .use_hse(25.mhz())
    //     .sysclk(96.mhz())
    //     .require_pll48clk()
    //     .freeze();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(96.mhz())
        .require_pll48clk()
        .freeze();

    let gpioa = dp.GPIOA.split();
    let gpioc = dp.GPIOC.split();
    let mut led = gpioc.pc13.into_open_drain_output();
    led.set_high().ok();

    let usb_irq = OTG_FS_OBJ.get_handle().unwrap();

    let usb = USB {
        usb_global: dp.OTG_FS_GLOBAL,
        usb_device: dp.OTG_FS_DEVICE,
        usb_pwrclk: dp.OTG_FS_PWRCLK,
        pin_dm: gpioa.pa11.into_alternate_af10(),
        pin_dp: gpioa.pa12.into_alternate_af10(),
        hclk: clocks.hclk(),
    };

    static mut EP_MEMORY: [u32; 1024] = [0; 1024];
    let usb_bus = UsbBus::new(usb, unsafe { &mut EP_MEMORY });

    let serial = usbd_serial::SerialPort::new(&usb_bus);

    let usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

    let mut device = SerialUsbDevice::new(usb_dev, serial, usb_irq);

    rprintln!("init");
    task::block_on(async {
        let mut buf = [0u8; 64];

        loop {
            let count = device.read(&mut buf).await.unwrap();

            // Echo back in upper case
            for c in buf[0..count].iter_mut() {
                if 0x61 <= *c && *c <= 0x7a {
                    *c &= !0x20;
                }
            }

            device.write_all(&buf[..count]).await.unwrap();
        }
    })
}
