//! Crash reporter. Answers "why did the board reboot" without a debug probe.
//!
//! A panic or a hard fault writes its reason into a RAM record that the
//! reset does not clear (nRF52 RAM is retained through every reset short of
//! power loss, which is also what the bootloader's double-tap magic relies
//! on). On the next boot `report_task` reads the record and the chip's own
//! RESETREAS register and logs both over the USB serial port, repeating for
//! a while so a terminal opened late still sees it.
//!
//! The record is deliberately not in .bss: cortex-m-rt zeroes .bss at every
//! start, .uninit it leaves alone.
use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use cortex_m_rt::{ExceptionFrame, exception};
use embassy_nrf::pac;
use embassy_time::{Duration, Timer};

const MAGIC: u32 = 0xC0DE_DEAD;
const BUF: usize = 240;

#[repr(C)]
struct Record {
    magic: u32,
    len: u32,
    buf: [u8; BUF],
}

#[unsafe(link_section = ".uninit.CRASH_RECORD")]
static mut RECORD: Record = Record { magic: 0, len: 0, buf: [0; BUF] };

struct Sink {
    at: usize,
}

impl Write for Sink {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let r = unsafe { &mut *core::ptr::addr_of_mut!(RECORD) };
        for &b in s.as_bytes() {
            if self.at >= BUF {
                break;
            }
            r.buf[self.at] = b;
            self.at += 1;
        }
        Ok(())
    }
}

/// Write a record unless one is already there: a panic ends in `udf`, which
/// raises a HardFault, and the fault handler must not overwrite the panic.
fn record(args: core::fmt::Arguments) {
    let r = unsafe { &mut *core::ptr::addr_of_mut!(RECORD) };
    if r.magic == MAGIC {
        return;
    }
    let mut sink = Sink { at: 0 };
    let _ = sink.write_fmt(args);
    r.len = sink.at as u32;
    r.magic = MAGIC;
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    record(format_args!("panic: {}", info));
    // Halt. The watchdog resets the chip within 10 s and the record survives.
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    record(format_args!(
        "hardfault pc={:#010x} lr={:#010x} xpsr={:#010x}",
        ef.pc(),
        ef.lr(),
        ef.xpsr()
    ));
    loop {
        cortex_m::asm::wfe();
    }
}

static REPORTED: AtomicBool = AtomicBool::new(false);

/// Read and clear the chip's reset reason. Bits per the nRF52840 POWER
/// RESETREAS register; DOG is the watchdog.
fn reset_reason() -> u32 {
    let reg = pac::POWER.resetreas();
    let v = reg.read().0;
    reg.write_value(pac::power::regs::Resetreas(0xFFFF_FFFF));
    v
}

fn describe(v: u32) -> &'static str {
    if v & (1 << 1) != 0 {
        "WATCHDOG"
    } else if v & (1 << 0) != 0 {
        "reset pin"
    } else if v & (1 << 2) != 0 {
        "soft reset"
    } else if v & (1 << 3) != 0 {
        "cpu lockup"
    } else if v == 0 {
        "power-on or brown-out"
    } else {
        "other (wake)"
    }
}

/// Spawned from main; logs the previous crash, if any, and the reset reason.
#[embassy_executor::task]
pub async fn report_task() {
    if REPORTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let reason = reset_reason();
    let r = unsafe { &mut *core::ptr::addr_of_mut!(RECORD) };
    let had = r.magic == MAGIC;
    let len = (r.len as usize).min(BUF);
    let mut msg = [0u8; BUF];
    msg[..len].copy_from_slice(&r.buf[..len]);
    r.magic = 0;
    // Repeat for two minutes so a terminal opened after boot still sees it.
    for _ in 0..8 {
        Timer::after(Duration::from_secs(3)).await;
        log::error!(
            "BOOT reason={:#010x} ({}) uptime={}ms",
            reason,
            describe(reason),
            embassy_time::Instant::now().as_millis()
        );
        if had {
            log::error!("LAST CRASH: {}", core::str::from_utf8(&msg[..len]).unwrap_or("<bad utf8>"));
        } else {
            log::error!("LAST CRASH: none recorded");
        }
        Timer::after(Duration::from_secs(12)).await;
    }
}
