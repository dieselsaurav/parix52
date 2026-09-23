//! Settings-reset firmware — RMK equivalent of the ZMK `settings_reset`
//! shield, as a single role-free binary safe to flash on EITHER half.
//!
//! Erases RMK's storage region and nothing else: no BLE, no split role, no
//! matrix. Wipes host bonds, split-pairing state, Vial live remaps and the
//! selected profile. The firmware image itself is untouched — but flashing
//! this replaces the keyboard firmware, so reflash the normal central /
//! peripheral UF2 afterwards.
//!
//! Result on the nice!nano onboard blue LED (P0.15):
//!   slow blink (~1 s)  = storage erased, flash normal firmware back
//!   fast blink (~0.1 s) = erase FAILED, retry / investigate
//!
//! Storage region: RMK's nrf52840 default (rmk-config
//! `default_config/nrf52840.toml`): start_addr = 0xA0000, num_sectors = 32
//! (x 4 KiB) -> 0xA0000..0xC0000. Our keyboard.toml leaves `[storage]` at
//! defaults, so these MUST stay in sync with it: if `start_addr` /
//! `num_sectors` are ever overridden there, update these constants.

#![no_main]
#![no_std]

use cortex_m_rt::entry;
use defmt_rtt as _;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::nvmc::Nvmc;
use embedded_storage::nor_flash::NorFlash;
// Not used directly — links the critical-section implementation
// (feature "critical-section-impl") that embassy/defmt need.
use nrf_mpsl as _;
use panic_probe as _;

/// RMK storage region (absolute flash addresses) — see module docs.
const STORAGE_START: u32 = 0x000A_0000;
const STORAGE_END: u32 = 0x000C_0000;

/// ~1 s at the 64 MHz core clock.
const SLOW: u32 = 64_000_000;
/// ~0.1 s — failure indicator.
const FAST: u32 = 6_400_000;

#[entry]
fn main() -> ! {
    let p = embassy_nrf::init(Default::default());

    let mut nvmc = Nvmc::new(p.NVMC);
    let erased = nvmc.erase(STORAGE_START, STORAGE_END).is_ok();

    defmt::info!(
        "storage erase {:x}..{:x}: {}",
        STORAGE_START,
        STORAGE_END,
        if erased { "OK" } else { "FAILED" }
    );

    let mut led = Output::new(p.P0_15, Level::Low, OutputDrive::Standard);
    let period = if erased { SLOW } else { FAST };
    loop {
        led.set_high();
        cortex_m::asm::delay(period);
        led.set_low();
        cortex_m::asm::delay(period);
    }
}
