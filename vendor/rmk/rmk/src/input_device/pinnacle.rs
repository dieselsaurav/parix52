//! Cirque Pinnacle (GlidePoint) trackpad driver, SPI, relative mode.
//!
//! Covers the TM040040 / TM035035 / TM023023 circle trackpads. Register map
//! and the RAP (register access protocol) framing follow Cirque's
//! GT-AN-090620 application note as used by the Zephyr `input_pinnacle`
//! driver and the CirquePinnacle Arduino library.
//!
//! The pad samples at 100 Hz while touched and raises DR (and the SW_DR bit
//! of Status1) for every packet. In relative mode a packet is four bytes:
//! buttons + sign bits, X delta, Y delta, scroll. Only X and Y are used here;
//! taps and scrolling are disabled at the pad so the packet never carries a
//! button the host would not see anyway.

use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiBus;

use crate::input_device::pointing::{InitState, MotionData, PointingDevice, PointingDriver, PointingDriverError};

// RAP: register read = 0xA0 | addr, two filler bytes, then one filler per
// register wanted; the values arrive in the bytes clocked out with those
// fillers. Every filler is 0xFC, exactly as Cirque's reference code sends
// them (an 0xFB "last" filler, borrowed from another library, is suspected
// of garbling the status read, 2026-10-01). Write = 0x80 | addr, value.
const RAP_READ: u8 = 0xA0;
const RAP_WRITE: u8 = 0x80;
const FILLER: u8 = 0xFC;

// Registers
const REG_FIRMWARE_ID: u8 = 0x00;
const REG_STATUS1: u8 = 0x02;
const REG_SYS_CONFIG1: u8 = 0x03;
const REG_FEED_CONFIG1: u8 = 0x04;
const REG_FEED_CONFIG2: u8 = 0x05;
const REG_CAL_CONFIG1: u8 = 0x07;
const CAL_CALIBRATE: u8 = 0x01;
const REG_PACKET_BYTE0: u8 = 0x12;
const REG_ERA_VALUE: u8 = 0x1B;
const REG_ERA_ADDR_HIGH: u8 = 0x1C;
const REG_ERA_ADDR_LOW: u8 = 0x1D;
const REG_ERA_CONTROL: u8 = 0x1E;

// Status1
const STATUS_SW_DR: u8 = 0x04;
const STATUS_SW_CC: u8 = 0x08;

// SysConfig1
const SYS_RESET: u8 = 0x01;
const SYS_SHUTDOWN: u8 = 0x02;
const SYS_SLEEP_ENABLE: u8 = 0x04;

// FeedConfig1
const FEED_ENABLE: u8 = 0x01;
const FEED_ABSOLUTE: u8 = 0x02;
const FEED_INVERT_X: u8 = 0x40;
const FEED_INVERT_Y: u8 = 0x80;
const REG_Z_IDLE: u8 = 0x0A;

// Absolute-mode geometry (TM040040 spec: X 0..2047, Y 0..1535; active area
// X 128..1920, Y 64..1472). Normalised so the active edge is radius 1.0.
const ABS_CENTER_X: f32 = 1024.0;
const ABS_CENTER_Y: f32 = 768.0;
const ABS_HALF_W: f32 = 896.0;
const ABS_HALF_H: f32 = 704.0;

// FeedConfig2: everything the pad could decide for us is off. Bit 0
// (IntelliMouse) stays clear so the packet keeps the 8-bit relative layout.
const FEED2_ALL_TAPS_DISABLE: u8 = 0x02;
const FEED2_SECONDARY_TAP_DISABLE: u8 = 0x04;
const FEED2_SCROLL_DISABLE: u8 = 0x08;
const FEED2_GLIDE_EXTEND_DISABLE: u8 = 0x10;

// ERA (extended register access)
const ERA_CONTROL_READ: u8 = 0x01;
const ERA_CONTROL_WRITE: u8 = 0x02;
const ERA_ADC_ATTENUATION: u16 = 0x0187;
const ERA_X_AXIS_WIDE_Z_MIN: u16 = 0x0149;
const ERA_Y_AXIS_WIDE_Z_MIN: u16 = 0x0168;
const ADC_ATTENUATION_MASK: u8 = 0xC0;
const ADC_ATTENUATE_2X: u8 = 0x40;
// Cirque's values for their curved overlay (Pinnacle_tuneEdgeSensitivity).
const CURVED_X_WIDE_Z_MIN: u8 = 0x04;
const CURVED_Y_WIDE_Z_MIN: u8 = 0x03;

const FIRMWARE_ID_PINNACLE: u8 = 0x07;

// Pinnacle needs 50 us after a flag clear before the next RAP access.
const T_AFTER_CLEAR_US: u64 = 50;
// Power-on to SW_CC.
const T_POWER_ON_MS: u64 = 100;

/// Trackpad configuration.
#[derive(Debug, Clone, Copy)]
pub struct PinnacleConfig {
    /// Invert X at the pad (FeedConfig1 bit 6).
    pub invert_x: bool,
    /// Invert Y at the pad (FeedConfig1 bit 7). The pad's Y is inverted by
    /// default relative to HID; leave false and correct in the processor if
    /// the direction is wrong on the desk.
    pub invert_y: bool,
    /// Cirque's curved overlay (TM0x00x0-20xx-303): halve ADC gain and raise
    /// the edge Z threshold, as Cirque's own example does.
    pub curved_overlay: bool,
    /// Let the pad sleep after ~5 s without a finger (40 uA instead of 1.7 mA).
    pub sleep: bool,
    /// Scroll ring: the outer band of the pad turns into a wheel. Needs the
    /// pad in absolute mode, so cursor deltas are then computed here.
    pub scroll_ring: bool,
    /// Ring width as a fraction of the radius, in percent (15 = outer 15 %).
    pub ring_width_percent: u8,
    /// Degrees of travel around the ring per wheel tick.
    pub ring_degrees_per_tick: u8,
    /// Clockwise scrolls down by default (like a thumb on a wheel); flip here.
    pub ring_invert: bool,
    /// Absolute-mode cursor speed: pad units per cursor count. The pad's own
    /// relative mode is roughly this divisor's worth coarser than its
    /// absolute coordinates.
    pub cursor_divisor: u8,
}

impl Default for PinnacleConfig {
    fn default() -> Self {
        Self {
            invert_x: false,
            invert_y: false,
            curved_overlay: false,
            sleep: true,
            scroll_ring: false,
            ring_width_percent: 15,
            ring_degrees_per_tick: 15,
            ring_invert: false,
            cursor_divisor: 3,
        }
    }
}

/// Angle of (x, y) in degrees, 0..360, atan2 without libm: an octant-folded
/// polynomial, within 0.3 degrees, more than enough for a scroll ring.
fn angle_deg(x: f32, y: f32) -> f32 {
    let ax = if x < 0.0 { -x } else { x };
    let ay = if y < 0.0 { -y } else { y };
    let (num, den, swap) = if ay > ax { (ax, ay, true) } else { (ay, ax, false) };
    if den == 0.0 {
        return 0.0;
    }
    let z = num / den; // 0..1
    let a = z * (45.0 - (z - 1.0) * (14.02 + 3.80 * z)); // atan in degrees, 0..45
    let a = if swap { 90.0 - a } else { a };
    let a = if x < 0.0 { 180.0 - a } else { a };
    if y < 0.0 { 360.0 - a } else { a }
}

/// An input pin seen upside down: `is_low` when the pin is high, and so on.
///
/// `PointingDevice` treats the motion pin as active-low (`wait_for_low`),
/// which is what optical sensors do. Pinnacle's DR is active-high, so the
/// pin is wrapped in this before it is handed over.
pub struct ActiveHigh<P>(pub P);

impl<P: ErrorType> ErrorType for ActiveHigh<P> {
    type Error = P::Error;
}

impl<P: InputPin> InputPin for ActiveHigh<P> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        self.0.is_low()
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.0.is_high()
    }
}

impl<P: Wait> Wait for ActiveHigh<P> {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        self.0.wait_for_low().await
    }
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        self.0.wait_for_high().await
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        self.0.wait_for_falling_edge().await
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        self.0.wait_for_rising_edge().await
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        self.0.wait_for_any_edge().await
    }
}

/// A chip-select line that can also be let go.
///
/// While the pad's supply is off the line must not be driven at all: driven
/// high it feeds the dead rail through the pad's input protection, and the
/// pad then powers up slowly from that leak instead of from its supply;
/// driven low the pad sees it low as it powers up and does not start as an
/// SPI device. Released, the pad's own R1 holds it, which is exactly the
/// state of a cold start, when the controller's pins float through the
/// bootloader while the rail is already up.
pub trait ChipSelect: OutputPin {
    /// Stop driving the line.
    fn release(&mut self);
    /// Drive the line again, high (deselected).
    fn drive_high(&mut self);
}

/// [`ChipSelect`] on an nRF GPIO.
#[cfg(feature = "_nrf_ble")]
pub struct NrfChipSelect(embassy_nrf::gpio::Flex<'static>);

#[cfg(feature = "_nrf_ble")]
impl NrfChipSelect {
    pub fn new(pin: embassy_nrf::Peri<'static, impl embassy_nrf::gpio::Pin>) -> Self {
        let mut pin = embassy_nrf::gpio::Flex::new(pin);
        pin.set_high();
        pin.set_as_output(embassy_nrf::gpio::OutputDrive::Standard);
        Self(pin)
    }
}

#[cfg(feature = "_nrf_ble")]
impl ErrorType for NrfChipSelect {
    type Error = core::convert::Infallible;
}

#[cfg(feature = "_nrf_ble")]
impl OutputPin for NrfChipSelect {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.0.set_low();
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.0.set_high();
        Ok(())
    }
}

#[cfg(feature = "_nrf_ble")]
impl ChipSelect for NrfChipSelect {
    fn release(&mut self) {
        self.0.set_as_disconnected();
    }
    fn drive_high(&mut self) {
        self.0.set_high();
        self.0.set_as_output(embassy_nrf::gpio::OutputDrive::Standard);
    }
}

/// Pinnacle driver over an SPI bus (mode 1) with a chip-select output and an
/// optional data-ready input (already wrapped in [`ActiveHigh`]).
pub struct Pinnacle<SPI: SpiBus, CS: ChipSelect, DR: InputPin + Wait> {
    id: u8,
    spi: SPI,
    cs: CS,
    dr: Option<DR>,
    config: PinnacleConfig,
    /// Absolute mode: last finger position (pad units) and whether it was
    /// in the ring, to turn positions into cursor deltas and ring angles.
    last_pos: Option<(i32, i32)>,
    last_in_ring: bool,
    last_angle: f32,
    ring_acc: f32,
    cursor_rem: (i32, i32),
    wheel_pending: i16,
    last_packet: Instant,
    /// The keyboard is asleep and the pad's supply may be switched off.
    powered_down: bool,
    /// The pad came back from a power cut and has not taken its
    /// configuration yet; retried from the read loop until it does.
    needs_config: bool,
    last_config_try: Instant,
}

impl<SPI, CS, DR> Pinnacle<SPI, CS, DR>
where
    SPI: SpiBus,
    CS: ChipSelect,
    DR: InputPin + Wait,
{
    pub fn new(id: u8, spi: SPI, cs: CS, dr: Option<DR>, config: PinnacleConfig) -> Self {
        Self {
            id,
            spi,
            cs,
            dr,
            config,
            last_pos: None,
            last_in_ring: false,
            last_angle: 0.0,
            ring_acc: 0.0,
            cursor_rem: (0, 0),
            wheel_pending: 0,
            last_packet: Instant::MIN,
            powered_down: false,
            needs_config: false,
            last_config_try: Instant::MIN,
        }
    }

    /// One absolute packet (X, Y, Z) turned into cursor deltas, with the
    /// ring band producing wheel ticks instead. Z == 0 is lift-off.
    fn absolute_to_motion(&mut self, x: u16, y: u16, z: u8) -> MotionData {
        // Lift-off: Cirque's idle packets carry (0, 0); QMK keys on that, Z
        // alone proved unreliable. A long silence also ends the touch, so a
        // missed idle packet cannot leave the gesture latched.
        let now = Instant::now();
        let silent = now.saturating_duration_since(self.last_packet) > Duration::from_millis(150);
        self.last_packet = now;
        if z == 0 || (x == 0 && y == 0) || silent {
            self.last_pos = None;
            self.last_in_ring = false;
            self.ring_acc = 0.0;
            self.cursor_rem = (0, 0);
            if z == 0 || (x == 0 && y == 0) {
                return MotionData::default();
            }
        }
        let fx = (x as f32 - ABS_CENTER_X) / ABS_HALF_W;
        let fy = (y as f32 - ABS_CENTER_Y) / ABS_HALF_H;
        let r2 = fx * fx + fy * fy;
        let ring_inner = 1.0 - self.config.ring_width_percent as f32 / 100.0;
        // The gesture is decided where the touch begins and kept until
        // lift-off: a scroll stays a scroll when the finger dips inside the
        // band, and a cursor move stays a cursor move when it reaches the edge.
        let in_ring = match self.last_pos {
            None => self.config.scroll_ring && r2 >= ring_inner * ring_inner,
            Some(_) => self.last_in_ring,
        };

        let mut out = MotionData::default();
        if in_ring {
            // Screen y grows downward, so negate fy for a conventional angle.
            let angle = angle_deg(fx, -fy);
            if self.last_in_ring {
                let mut d = angle - self.last_angle;
                if d > 180.0 {
                    d -= 360.0;
                } else if d < -180.0 {
                    d += 360.0;
                }
                self.ring_acc += d;
                let step = self.config.ring_degrees_per_tick.max(1) as f32;
                while self.ring_acc >= step {
                    self.ring_acc -= step;
                    // anticlockwise = scroll up (+), clockwise = down (-)
                    self.wheel_pending += if self.config.ring_invert { -1 } else { 1 };
                }
                while self.ring_acc <= -step {
                    self.ring_acc += step;
                    self.wheel_pending += if self.config.ring_invert { 1 } else { -1 };
                }
            }
            self.last_angle = angle;
            self.cursor_rem = (0, 0);
        } else if let Some((lx, ly)) = self.last_pos
            && !self.last_in_ring
        {
            let div = self.config.cursor_divisor.max(1) as i32;
            let ax = x as i32 - lx + self.cursor_rem.0;
            let ay = y as i32 - ly + self.cursor_rem.1;
            out.dx = (ax / div) as i16;
            out.dy = (ay / div) as i16;
            self.cursor_rem = (ax % div, ay % div);
        }
        self.last_pos = Some((x as i32, y as i32));
        self.last_in_ring = in_ring;
        out
    }

    async fn transfer(&mut self, buf: &mut [u8]) -> Result<(), PointingDriverError> {
        let _ = self.cs.set_low();
        let r = self.spi.transfer_in_place(buf).await;
        let _ = self.cs.set_high();
        r.map_err(|_| PointingDriverError::Spi)
    }

    async fn read_reg(&mut self, addr: u8) -> Result<u8, PointingDriverError> {
        let mut buf = [RAP_READ | addr, FILLER, FILLER, FILLER];
        self.transfer(&mut buf).await?;
        Ok(buf[3])
    }

    /// Read `out.len()` consecutive registers starting at `addr`.
    async fn read_regs(&mut self, addr: u8, out: &mut [u8]) -> Result<(), PointingDriverError> {
        // 1 address + 2 fillers, then one filler per byte wanted.
        let mut buf = [FILLER; 3 + 6];
        let n = out.len().min(6);
        buf[0] = RAP_READ | addr;
        self.transfer(&mut buf[..3 + n]).await?;
        out[..n].copy_from_slice(&buf[3..3 + n]);
        Ok(())
    }

    async fn write_reg(&mut self, addr: u8, value: u8) -> Result<(), PointingDriverError> {
        let mut buf = [RAP_WRITE | addr, value];
        self.transfer(&mut buf).await
    }

    async fn clear_flags(&mut self) -> Result<(), PointingDriverError> {
        self.write_reg(REG_STATUS1, 0x00).await?;
        Timer::after(Duration::from_micros(T_AFTER_CLEAR_US)).await;
        Ok(())
    }

    async fn era_wait(&mut self) -> Result<(), PointingDriverError> {
        for _ in 0..100 {
            if self.read_reg(REG_ERA_CONTROL).await? == 0 {
                return Ok(());
            }
            Timer::after(Duration::from_micros(50)).await;
        }
        Err(PointingDriverError::InitFailed)
    }

    async fn era_read(&mut self, addr: u16) -> Result<u8, PointingDriverError> {
        self.write_reg(REG_ERA_ADDR_HIGH, (addr >> 8) as u8).await?;
        self.write_reg(REG_ERA_ADDR_LOW, addr as u8).await?;
        self.write_reg(REG_ERA_CONTROL, ERA_CONTROL_READ).await?;
        self.era_wait().await?;
        let v = self.read_reg(REG_ERA_VALUE).await?;
        self.clear_flags().await?;
        Ok(v)
    }

    async fn era_write(&mut self, addr: u16, value: u8) -> Result<(), PointingDriverError> {
        self.write_reg(REG_ERA_VALUE, value).await?;
        self.write_reg(REG_ERA_ADDR_HIGH, (addr >> 8) as u8).await?;
        self.write_reg(REG_ERA_ADDR_LOW, addr as u8).await?;
        self.write_reg(REG_ERA_CONTROL, ERA_CONTROL_WRITE).await?;
        self.era_wait().await?;
        self.clear_flags().await
    }

    /// Cirque's curved-overlay compensation: ADC attenuation 2x and a higher
    /// wide-Z minimum on both axes so the thicker overlay still reads edges.
    async fn tune_for_curved_overlay(&mut self) -> Result<(), PointingDriverError> {
        let atten = self.era_read(ERA_ADC_ATTENUATION).await?;
        self.era_write(ERA_ADC_ATTENUATION, (atten & !ADC_ATTENUATION_MASK) | ADC_ATTENUATE_2X)
            .await?;
        self.era_write(ERA_X_AXIS_WIDE_Z_MIN, CURVED_X_WIDE_Z_MIN).await?;
        self.era_write(ERA_Y_AXIS_WIDE_Z_MIN, CURVED_Y_WIDE_Z_MIN).await
    }

    async fn configure(&mut self) -> Result<(), PointingDriverError> {
        // Give the pad its power-on time, then check it is the chip we expect.
        Timer::after(Duration::from_millis(T_POWER_ON_MS)).await;
        let fw = self.read_reg(REG_FIRMWARE_ID).await?;
        if fw != FIRMWARE_ID_PINNACLE {
            error!("Pinnacle {}: firmware id {:#x}, expected {:#x}", self.id, fw, FIRMWARE_ID_PINNACLE);
            return Err(PointingDriverError::InvalidProductId(fw));
        }

        // Reset so every register starts from its default, wait for SW_CC.
        self.write_reg(REG_SYS_CONFIG1, SYS_RESET).await?;
        Timer::after(Duration::from_millis(30)).await;
        for _ in 0..20 {
            if self.read_reg(REG_STATUS1).await? & STATUS_SW_CC != 0 {
                break;
            }
            Timer::after(Duration::from_millis(5)).await;
        }
        self.clear_flags().await?;

        let sys = if self.config.sleep { SYS_SLEEP_ENABLE } else { 0 };
        self.write_reg(REG_SYS_CONFIG1, sys & !SYS_SHUTDOWN).await?;

        // Feed stays off while the ERA registers are written.
        self.write_reg(REG_FEED_CONFIG1, 0x00).await?;
        self.write_reg(
            REG_FEED_CONFIG2,
            FEED2_ALL_TAPS_DISABLE | FEED2_SECONDARY_TAP_DISABLE | FEED2_SCROLL_DISABLE | FEED2_GLIDE_EXTEND_DISABLE,
        )
        .await?;
        if self.config.curved_overlay {
            self.tune_for_curved_overlay().await?;
            // The baseline was captured at the old gain; Cirque (and QMK)
            // force a recalibration after changing the ADC attenuation.
            // It takes ~100 ms; the CALIBRATE bit clears when done.
            let cal = self.read_reg(REG_CAL_CONFIG1).await?;
            self.write_reg(REG_CAL_CONFIG1, cal | CAL_CALIBRATE).await?;
            for _ in 0..40 {
                Timer::after(Duration::from_millis(5)).await;
                if self.read_reg(REG_CAL_CONFIG1).await? & CAL_CALIBRATE == 0 {
                    break;
                }
            }
            self.clear_flags().await?;
        }

        let mut feed1 = FEED_ENABLE;
        if self.config.scroll_ring {
            // Absolute positions, and five empty packets after a lift-off
            // (default 30; QMK uses 5), enough to notice the lift.
            feed1 |= FEED_ABSOLUTE;
            self.write_reg(REG_Z_IDLE, 5).await?;
        }
        if self.config.invert_x {
            feed1 |= FEED_INVERT_X;
        }
        if self.config.invert_y {
            feed1 |= FEED_INVERT_Y;
        }
        self.write_reg(REG_FEED_CONFIG1, feed1).await?;
        self.clear_flags().await?;

        info!("Pinnacle {}: configured, {} mode", self.id, if self.config.scroll_ring { "absolute" } else { "relative" });
        Ok(())
    }
}

impl<SPI, CS, DR> PointingDriver for Pinnacle<SPI, CS, DR>
where
    SPI: SpiBus,
    CS: ChipSelect,
    DR: InputPin + Wait,
{
    type MOTION = DR;

    async fn init(&mut self) -> Result<(), PointingDriverError> {
        let _ = self.cs.set_high();
        Timer::after(Duration::from_millis(1)).await;
        self.configure().await
    }

    async fn read_motion(&mut self) -> Result<MotionData, PointingDriverError> {
        if self.powered_down {
            return Ok(MotionData::default());
        }
        if self.needs_config {
            // Not more than twice a second: each try takes ~150 ms.
            if self.last_config_try.elapsed() >= Duration::from_millis(500) {
                self.last_config_try = Instant::now();
                if self.configure().await.is_ok() {
                    self.needs_config = false;
                    info!("Pinnacle {}: reconfigured after power-up", self.id);
                }
            }
            return Ok(MotionData::default());
        }
        // Without a DR pin `motion_pending` cannot tell, so ask the pad.
        // 0xFF is what a floating or unwired SO line reads as; it is not a
        // status, and acting on it would replay the pad's last packet forever.
        if self.dr.is_none() {
            let status = self.read_reg(REG_STATUS1).await?;
            if status == 0xFF || status & STATUS_SW_DR == 0 {
                return Ok(MotionData::default());
            }
        }

        if self.config.scroll_ring {
            // Absolute packet, six bytes: [buttons, -, X low, Y low, X/Y high
            // nibbles, Z]. Layout per Cirque's reference code.
            let mut packet = [0u8; 6];
            self.read_regs(REG_PACKET_BYTE0, &mut packet).await?;
            self.clear_flags().await?;
            if packet == [0xFF; 6] {
                return Ok(MotionData::default());
            }
            let x = packet[2] as u16 | ((packet[4] & 0x0F) as u16) << 8;
            let y = packet[3] as u16 | ((packet[4] & 0xF0) as u16) << 4;
            let z = packet[5] & 0x3F;
            return Ok(self.absolute_to_motion(x, y, z));
        }

        let mut packet = [0u8; 4];
        self.read_regs(REG_PACKET_BYTE0, &mut packet).await?;
        self.clear_flags().await?;

        // Four 0xFF bytes are a floating bus, not a packet. (Bits 6/7 of the
        // flag byte are NOT a usable validity check: the pad sets them in
        // ordinary packets, and filtering on them killed all motion.)
        if packet == [0xFF; 4] {
            return Ok(MotionData::default());
        }

        // Bytes 1 and 2 are plain two's-complement 8-bit deltas, as Cirque's
        // reference code reads them. Byte 0 carries buttons and flags; its
        // bits 4/5 are NOT reliable sign bits: treating them as such turned
        // the zero-delta packets after a lift-off into -256 and sent the
        // cursor to the top of the screen on every touch (2026-10-01).
        let dx = packet[1] as i8 as i16;
        let dy = packet[2] as i8 as i16;

        Ok(MotionData { dx, dy })
    }

    fn take_wheel(&mut self) -> i16 {
        core::mem::take(&mut self.wheel_pending)
    }

    /// The keyboard sleeps or wakes. On this board the pad hangs off the
    /// module's switched VCC rail, which is turned off for the sleep, so:
    /// going down, chip select is parked low and the pad is left alone (a
    /// line held high would feed the dead rail through the pad's input
    /// protection); coming up, the pad is a freshly powered one and gets its
    /// whole configuration again. Harmless where the rail stays on: the pad
    /// is simply reset and reconfigured on every wake.
    async fn set_low_power(&mut self, enabled: bool) -> Result<(), PointingDriverError> {
        if enabled {
            self.powered_down = true;
            self.cs.release();
            return Ok(());
        }
        // "Awake" is also announced at start and on every reconnect; only a
        // pad that was actually put down needs anything done.
        if !self.powered_down {
            return Ok(());
        }
        // The rail is coming back (src/rgb.rs switches it on at this same
        // event). Chip select stays released while it does, so the pad powers
        // up the way it does on a cold start; see `ChipSelect`. Two earlier
        // orders both left the pad dead until the next power cycle: line low
        // until after the rail (always), line high before the rail (often).
        // The configuration itself is done from the read loop, which retries
        // until the pad answers.
        Timer::after(Duration::from_millis(100)).await;
        self.cs.drive_high();
        self.last_pos = None;
        self.last_in_ring = false;
        self.ring_acc = 0.0;
        self.cursor_rem = (0, 0);
        self.wheel_pending = 0;
        self.needs_config = true;
        self.last_config_try = Instant::MIN;
        self.powered_down = false;
        Ok(())
    }

    /// With DR: pending while the (inverted) pin reads low, i.e. DR is high.
    /// Without DR: always poll; `read_motion` checks SW_DR itself.
    fn motion_pending(&mut self) -> bool {
        if self.powered_down {
            return false;
        }
        if self.needs_config {
            return true; // keep the read loop coming until the pad is configured
        }
        match &mut self.dr {
            Some(dr) => dr.is_low().unwrap_or(true),
            None => true,
        }
    }

    fn motion_gpio(&mut self) -> Option<&mut DR> {
        // An unconfigured pad never raises DR, so waiting on the pin would
        // wait for ever: poll on the timer until it is configured again.
        if self.needs_config {
            return None;
        }
        self.dr.as_mut()
    }
}

impl<SPI, CS, DR> PointingDevice<Pinnacle<SPI, CS, DR>>
where
    SPI: SpiBus,
    CS: ChipSelect,
    DR: InputPin + Wait,
{
    // The pad produces a packet every 10 ms; polling faster than that only
    // burns SPI transfers on an unchanged SW_DR.
    const DEFAULT_POLL_INTERVAL_US: u64 = 4_000;
    const DEFAULT_REPORT_HZ: u16 = 100;

    pub fn new(id: u8, spi: SPI, cs: CS, dr: Option<DR>, config: PinnacleConfig) -> Self {
        Self::with_report_hz(id, spi, cs, dr, config, Self::DEFAULT_REPORT_HZ)
    }

    pub fn with_report_hz(id: u8, spi: SPI, cs: CS, dr: Option<DR>, config: PinnacleConfig, report_hz: u16) -> Self {
        let report_interval = Duration::from_hz(report_hz as u64);
        let poll_interval = Duration::from_micros(Self::DEFAULT_POLL_INTERVAL_US).min(report_interval);
        Self {
            id,
            sensor: Pinnacle::new(id, spi, cs, dr, config),
            init_state: InitState::Pending,
            poll_interval,
            report_interval,
            last_poll: Instant::MIN,
            last_report: Instant::MIN,
            accumulated_x: 0,
            accumulated_y: 0,
            accumulated_wheel: 0,
        }
    }
}
