#[macro_use]
mod sus;
mod timer;

use core::{cell::UnsafeCell, panic::PanicInfo, sync::atomic::{AtomicBool, Ordering}, task::Context};

use critical_section::RestoreState;
use defmt::{Encoder, Logger};
use embassy_executor::{task, Spawner};
use embassy_nrf::{gpio::{Level, Output, OutputDrive}, peripherals::{P0_06, P0_08, P1_09, USBD}, usb::vbus_detect::HardwareVbusDetect};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe};
use timer::Timer;
use embassy_usb::{class::cdc_acm::{CdcAcmClass, State}, Builder, UsbDevice};
use static_cell::ConstStaticCell;

use crate::defmt_serial::timer::NOOP_WAKER;

type Driver = embassy_nrf::usb::Driver<'static, USBD, HardwareVbusDetect>;

/// Adds a CDC-ACM interface, runs the USB device and initializes the logger.
pub fn init(spawner: &mut Spawner, mut builder: Builder<'static, Driver>) {
    static STATE: ConstStaticCell<State> = ConstStaticCell::new(State::new());
    let state = STATE.take();

    let class = CdcAcmClass::new(&mut builder, state, 64);
    let usb = builder.build();

    // Run the USB device.
    unsafe {
        USB_FUT.init(run_usb(usb));
        spawner.must_spawn(usb_task());
        LOG_FUT.init(log_loop(class));
        spawner.must_spawn(log_task());
    }
}

static PIPE: Pipe<CriticalSectionRawMutex, 1024> = Pipe::new();
async fn log_loop(mut class: CdcAcmClass<'static, Driver>) {
    class.wait_connection().await;

    let mut buf = [0; 64];
    let (mut send, _recv, ctl) = class.split_with_control();
    while !send.dtr() {
        ctl.control_changed().await;
    }

    Timer::after_millis(50).await;

    loop {
        let mut n = PIPE.read(&mut buf).await;
        loop {
            while !send.dtr() {
                ctl.control_changed().await;
            }
            let _ = send.write_packet(&buf[..n]).await;
            if n == 0 {
                break;
            }
            n = match PIPE.try_read(&mut buf) {
                Ok(n) => n,
                Err(embassy_sync::pipe::TryReadError::Empty) => 0,
            };
        }
    }
}

async fn run_usb(mut usb: UsbDevice<'static, Driver>) {
    usb.run().await
}

#[task]
async fn log_task() {
    unsafe {
        LOG_FUT.get().as_mut().unwrap_unchecked().await;
    }
}

#[task]
async fn usb_task() {
    unsafe {
        USB_FUT.get().as_mut().unwrap_unchecked().await;
    }
}

declare_static_future!(LOG_FUT = log_loop; _log_loop);
declare_static_future!(USB_FUT = run_usb; _run_usb);

struct LoggerState {
    restore_state: UnsafeCell<RestoreState>,
    encoder: UnsafeCell<Encoder>,
}

impl LoggerState {
    const fn new() -> Self {
        Self {
            restore_state: UnsafeCell::new(RestoreState::invalid()),
            encoder: UnsafeCell::new(Encoder::new()),
        }
    }
}

unsafe impl Sync for LoggerState {}

static LOGGER_STATE: LoggerState = LoggerState::new();
#[defmt::global_logger]
struct SerialLogger;

unsafe impl Logger for SerialLogger {
    fn acquire() {
        unsafe {
            LOGGER_STATE.restore_state.get().write(critical_section::acquire());
            (*LOGGER_STATE.encoder.get()).start_frame(push_log_bytes);
        }
    }

    unsafe fn flush() {
    }

    unsafe fn release() {
        unsafe {
            critical_section::release(*LOGGER_STATE.restore_state.get());
            (*LOGGER_STATE.encoder.get()).end_frame(push_log_bytes);
        }
    }

    unsafe fn write(bytes: &[u8]) {
        unsafe {
            (*LOGGER_STATE.encoder.get()).write(bytes, push_log_bytes);
        }
    }
}

fn push_log_bytes(mut bytes: &[u8]) {
    loop {
        let Ok(n) = PIPE.try_write(bytes) else {
            return;
        };
        bytes = &bytes[n..];
        if bytes.is_empty() {
            break;
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    static PANICKED: AtomicBool = AtomicBool::new(false);

    cortex_m::interrupt::disable();

    // Guard against infinite recursion, just in case.
    if !PANICKED.load(Ordering::Relaxed) {
        PANICKED.store(true, Ordering::Relaxed);

        defmt::error!("{}", defmt::Display2Format(info));

        // Try to keep usb alive for logs
        let mut cx = Context::from_waker(NOOP_WAKER);
        loop {
            unsafe {
                match LOG_FUT.get() {
                    None => (),
                    Some(f) => _ = f.poll(&mut cx),
                }
                match USB_FUT.get() {
                    None => (),
                    Some(f) => _ = f.poll(&mut cx),
                }
                crate::USBD();
            }
        }
    } else {
        cortex_m::asm::udf();
    }
}
