#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{bind_interrupts, gpio, peripherals::USB, usb::InterruptHandler};
use embassy_time::Timer;
use gpio::{Level, Output};
use static_cell::StaticCell;
use xinput_device::{
    controller::XboxGamepad,
    xinput::{self, XInput},
};
use {defmt_rtt as _, panic_probe as _};

const CONTROLLER_STATE_INIT: xinput::State = xinput::State::new();
static CONTROLLER_STATE: [xinput::State; 4] = [CONTROLLER_STATE_INIT; 4];

type UsbDriver = embassy_rp::usb::Driver<'static, USB>;
type UsbDevice = embassy_usb::UsbDevice<'static, UsbDriver>;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn xinput_task(xinput_device: XInput<'static, UsbDriver>) -> ! {
    xinput_device.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Program start");
    let p = embassy_rp::init(Default::default());
    let _led = Output::new(p.PIN_25, Level::Low);

    let driver = embassy_rp::usb::Driver::new(p.USB, Irqs);

    let mut config = embassy_usb::Config::new(0x045E, 0x0719);

    config.composite_with_iads = false;
    config.device_class = 0xFF;
    config.device_sub_class = 0xFF;
    config.device_protocol = 0xFF;

    config.device_release = 0x0100;
    config.manufacturer = Some("9names");
    config.product = Some("wii-usb");
    config.serial_number = Some("FFFFFFFF");
    config.max_power = 260;
    config.max_packet_size_0 = 64;

    // The first 4 bytes should match the USB serial number descriptor.
    // Not required for the receiver to be detected by the windows driver.
    static SERIAL_NUMBER_HANDLER: StaticCell<xinput::SerialNumberHandler> = StaticCell::new();
    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 324]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 324]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    let x = xinput::SerialNumberHandler {
        0: [0xFF, 0xFF, 0xFF, 0xFF, 0x0a, 0x89, 0xB7],
    };
    builder.handler(SERIAL_NUMBER_HANDLER.init(x));

    let controller_0 = XInput::new_wireless(&mut builder, &CONTROLLER_STATE[0], false);

    let usb = builder.build();
    let _usb_task_token = spawner.spawn(usb_task(usb));
    let _xinput_task_token = spawner.spawn(xinput_task(controller_0));

    loop {
        let controller_state = XboxGamepad {
            btn_a: false,
            ..Default::default()
        };
        CONTROLLER_STATE[0].send_xinput(controller_state.into());
        Timer::after_secs(1).await;

        let controller_state = XboxGamepad {
            btn_a: true,
            ..Default::default()
        };
        CONTROLLER_STATE[0].send_xinput(controller_state.into());
        Timer::after_secs(1).await;
    }
}
