#[cfg(feature = "_ble")]
use core::cell::Cell;

#[cfg(feature = "_ble")]
use embassy_sync::blocking_mutex::Mutex;
use embedded_hal::digital::InputPin;
use rmk_macro::{input_device, processor};
#[cfg(feature = "_ble")]
use rmk_types::battery::{BatteryStatus, ChargeState};

#[cfg(feature = "_ble")]
use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::event::BatteryStatusEvent;
use crate::event::{BatteryAdcEvent, ChargingStateEvent, publish_event};

/// Cached battery status, updated by [`BatteryProcessor::commit`] alongside every
/// [`BatteryStatusEvent`] publish so host services can read the current value
/// synchronously without subscribing to the event stream.
#[cfg(feature = "_ble")]
pub(crate) static BATTERY_STATUS: Mutex<RawMutex, Cell<BatteryStatus>> =
    Mutex::new(Cell::new(BatteryStatus::Unavailable));

#[cfg(feature = "_ble")]
pub(crate) fn current_battery_status() -> BatteryStatus {
    BATTERY_STATUS.lock(|c| c.get())
}

/// Reads charging state from a GPIO pin and publishes ChargingStateEvent.
///
/// This input device monitors a charging state pin and publishes events when
/// the charging state changes.
#[input_device(publish = ChargingStateEvent)]
pub struct ChargingStateReader<I: InputPin> {
    // Charging state pin or standby pin
    state_input: I,
    // True: low represents charging, False: high represents charging
    low_active: bool,
    // True: charging, False: not charging
    current_charging_state: bool,
    // First read done
    first_read: bool,
}

impl<I: InputPin> ChargingStateReader<I> {
    pub fn new(state_input: I, low_active: bool) -> Self {
        Self {
            state_input,
            low_active,
            current_charging_state: false,
            first_read: false,
        }
    }

    /// Read the charging state and return an event.
    /// This method waits until there's a state change to report.
    async fn read_charging_state_event(&mut self) -> ChargingStateEvent {
        // For the first read, don't check whether the charging state is changed
        if !self.first_read {
            // Wait 2s before reading the first value
            embassy_time::Timer::after_secs(2).await;
            let charging_state = if self.low_active {
                self.state_input.is_low().unwrap_or(false)
            } else {
                self.state_input.is_high().unwrap_or(false)
            };
            self.current_charging_state = charging_state;
            self.first_read = true;
            return ChargingStateEvent {
                charging: charging_state,
            };
        }

        loop {
            // Check charging state every 5 seconds
            embassy_time::Timer::after_secs(5).await;

            // Detect charging state
            let charging_state = if self.low_active {
                self.state_input.is_low().unwrap_or(false)
            } else {
                self.state_input.is_high().unwrap_or(false)
            };

            // Only return event when charging state changes
            if charging_state != self.current_charging_state {
                self.current_charging_state = charging_state;
                return ChargingStateEvent {
                    charging: charging_state,
                };
            }
        }
    }
}

/// BatteryProcessor processes battery adc value and charging state,
/// emits `BatteryStatusEvent` when battery status changes.
#[processor(subscribe = [BatteryAdcEvent, ChargingStateEvent])]
pub struct BatteryProcessor {
    adc_divider_measured: u32,
    adc_divider_total: u32,
    /// Current battery status
    battery_status: BatteryStatus,
    /// PARIX PATCH: the cell voltage, smoothed, in millivolts times 16.
    filtered_mv16: Option<u32>,
}

/// PARIX PATCH: a lithium-polymer cell's resting voltage against the charge
/// left in it, millivolts to percent, highest first. Upstream maps 3.6 V to
/// 4.2 V in a straight line, which reads a third at half charge (3.8 V) and
/// zero with a tenth still in the cell: the curve is steep at both ends and
/// nearly flat through the middle. A generic table; cells differ by a few
/// points.
const LIPO_CURVE: [(u16, u8); 21] = [
    (4200, 100),
    (4150, 95),
    (4110, 90),
    (4080, 85),
    (4020, 80),
    (3980, 75),
    (3950, 70),
    (3910, 65),
    (3870, 60),
    (3850, 55),
    (3840, 50),
    (3820, 45),
    (3800, 40),
    (3790, 35),
    (3770, 30),
    (3750, 25),
    (3730, 20),
    (3710, 15),
    (3690, 10),
    (3610, 5),
    (3300, 0),
];

/// Percent of charge for a cell voltage, interpolated along [`LIPO_CURVE`].
fn lipo_percent(mv: u32) -> u8 {
    if mv >= LIPO_CURVE[0].0 as u32 {
        return 100;
    }
    let mut i = 1;
    while i < LIPO_CURVE.len() {
        let (hi_mv, hi_pct) = LIPO_CURVE[i - 1];
        let (lo_mv, lo_pct) = LIPO_CURVE[i];
        if mv >= lo_mv as u32 {
            let span = (hi_mv - lo_mv) as u32;
            let above = mv - lo_mv as u32;
            return lo_pct + ((hi_pct - lo_pct) as u32 * above / span) as u8;
        }
        i += 1;
    }
    0
}

/// PARIX PATCH: whether this half is on USB power, which on these boards
/// means the cell is charging. Read from the chip's own VBUS detector, so no
/// charge-state pin is needed.
#[cfg(feature = "_nrf_ble")]
fn usb_power_present() -> Option<bool> {
    Some(embassy_nrf::pac::POWER.usbregstatus().read().vbusdetect())
}

#[cfg(not(feature = "_nrf_ble"))]
fn usb_power_present() -> Option<bool> {
    None
}

impl BatteryProcessor {
    pub fn new(adc_divider_measured: u32, adc_divider_total: u32) -> Self {
        BatteryProcessor {
            adc_divider_measured,
            adc_divider_total,
            battery_status: BatteryStatus::Unavailable,
            filtered_mv16: None,
        }
    }

    /// Apply a new battery status: persist on the processor, mirror into
    /// [`BATTERY_STATUS`] for synchronous readers, and broadcast via
    /// [`BatteryStatusEvent`].
    #[cfg(feature = "_ble")]
    fn commit(&mut self, status: BatteryStatus) {
        self.battery_status = status;
        BATTERY_STATUS.lock(|c| c.set(status));
        publish_event(BatteryStatusEvent::from(status));
    }

    #[cfg(feature = "_ble")]
    /// The cell voltage in millivolts for a raw ADC value.
    fn battery_millivolts(&self, val: u16) -> Option<u32> {
        // According to nRF52840's datasheet, for single_ended saadc:
        // val = v_adc * (gain / reference) * 2^(resolution)
        // With the default gain 1/6, reference 0.6 V and 12 bits:
        // val = v_adc * 1137.8, and v_adc = v_bat * measured / total.
        let val = val as u64;
        let mut measured = self.adc_divider_measured as u64;
        let mut total = self.adc_divider_total as u64;
        if 500 < val && val < 1000 {
            // The ADC's VDDH input is VDDH / 5; this rough range detects it.
            measured = 1;
            total = 5;
        }
        if measured == 0 || total == 0 {
            error!("Battery ADC divider values must be greater than zero");
            return None;
        }
        Some((val * 10_000 * total / (11_378 * measured)) as u32)
    }

    fn get_battery_percent(&self, val: u16) -> u8 {
        match self.battery_millivolts(val) {
            Some(mv) => lipo_percent(mv),
            None => 0,
        }
    }
}

#[cfg(all(test, feature = "_ble"))]
mod tests {
    use super::BatteryProcessor;

    #[test]
    fn invalid_adc_dividers_do_not_panic() {
        assert_eq!(BatteryProcessor::new(1, 0).get_battery_percent(2000), 0);
        assert_eq!(BatteryProcessor::new(0, 1).get_battery_percent(2000), 0);
    }

    #[test]
    fn divider_rounding_boundary_does_not_underflow() {
        assert!(BatteryProcessor::new(2000, 2806).get_battery_percent(2890) <= 100);
    }
}

impl BatteryProcessor {
    async fn on_battery_adc_event(&mut self, event: BatteryAdcEvent) {
        let val = event.0;
        trace!("Detected battery ADC value: {:?}", val);

        // PARIX PATCH: the level is read from a smoothed voltage along the
        // cell's discharge curve, it keeps updating while charging (upstream
        // froze it), and the charge state comes from USB power where the chip
        // can tell.
        #[cfg(feature = "_ble")]
        {
            let Some(mv) = self.battery_millivolts(val) else {
                return;
            };
            let previous = match self.battery_status {
                BatteryStatus::Available { charge_state, level } => Some((charge_state, level)),
                BatteryStatus::Unavailable => None,
            };
            let charge_state = match usb_power_present() {
                Some(true) => ChargeState::Charging,
                Some(false) => ChargeState::Discharging,
                None => previous.map(|(c, _)| c).unwrap_or(ChargeState::Unknown),
            };
            // The lights pull the voltage down while lit and it recovers when
            // they go dark, so single readings wobble by several points. A
            // slow average (1/16 per sample) steadies it. Plugging in or out
            // moves the voltage at once, so the average starts again there.
            let restart = previous.map(|(c, _)| c != charge_state).unwrap_or(true);
            let filtered = match self.filtered_mv16 {
                Some(f) if !restart => f - f / 16 + mv,
                _ => mv * 16,
            };
            self.filtered_mv16 = Some(filtered);
            let level = Some(lipo_percent(filtered / 16));
            if previous != Some((charge_state, level)) {
                self.commit(BatteryStatus::Available { charge_state, level });
            }
        }
    }

    async fn on_charging_state_event(&mut self, event: ChargingStateEvent) {
        let charging = event.charging;
        info!("Charging state changed: {:?}", charging);

        #[cfg(feature = "_ble")]
        {
            let status = if charging {
                // Keep current level when charging
                let level = match self.battery_status {
                    BatteryStatus::Available { level, .. } => level,
                    BatteryStatus::Unavailable => None,
                };
                BatteryStatus::Available {
                    charge_state: ChargeState::Charging,
                    level,
                }
            } else {
                // When unplugged, mark the level unknown and mark status as discharging
                BatteryStatus::Available {
                    charge_state: ChargeState::Discharging,
                    level: None,
                }
            };

            self.commit(status);
        }
    }
}
