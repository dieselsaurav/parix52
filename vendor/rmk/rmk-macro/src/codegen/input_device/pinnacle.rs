use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::resolved::hardware::{ChipModel, ChipSeries, PinnacleConfig};

use super::Initializer;

/// Expand Cirque Pinnacle trackpad configuration (hardware SPIM, nRF52 only).
/// Returns (device initializers, processor initializers), same shape as PMW33xx.
pub(crate) fn expand_pinnacle_device(
    config: Vec<PinnacleConfig>,
    chip: &ChipModel,
) -> (Vec<Initializer>, Vec<Initializer>) {
    if config.is_empty() {
        return (Vec::new(), Vec::new());
    }
    if chip.series != ChipSeries::Nrf52 {
        panic!("pinnacle is only supported on nRF52 chips");
    }

    let mut device_initializers = vec![];
    let mut processor_initializers = vec![];

    for (idx, pad) in config.iter().enumerate() {
        let id = pad.id.unwrap_or(0);
        let name = if pad.name.is_empty() {
            format!("pinnacle_{}_id{}", idx, id)
        } else {
            format!("{}_id{}", pad.name, id)
        };
        let device_ident = format_ident!("{}_device", name);
        let processor_ident = format_ident!("{}_processor", name);
        let processor_config_ident = format_ident!("{}_config", processor_ident);

        let spi = &pad.spi;
        let instance_ident = format_ident!("{}", spi.instance);
        let sck_ident = format_ident!("{}", spi.sck);
        let mosi_ident = format_ident!("{}", spi.mosi);
        let miso_ident = format_ident!("{}", spi.miso);
        let cs_ident = format_ident!("{}", spi.cs.as_ref().expect("pinnacle requires `cs` in spi config"));

        // DR is active-high; PointingDevice waits for low, so it is wrapped.
        let dr_init = if let Some(dr) = &pad.dr {
            let dr_ident = format_ident!("{}", dr);
            quote! {
                Some(ActiveHigh(::embassy_nrf::gpio::Input::new(p.#dr_ident, ::embassy_nrf::gpio::Pull::Down)))
            }
        } else {
            quote! { None::<ActiveHigh<::embassy_nrf::gpio::Input<'static>>> }
        };

        let invert_x = pad.invert_x;
        let invert_y = pad.invert_y;
        let curved_overlay = pad.curved_overlay;
        let sleep = pad.sleep;
        let scroll_ring = pad.scroll_ring;
        let ring_width_percent = pad.ring_width_percent;
        let ring_degrees_per_tick = pad.ring_degrees_per_tick;
        let ring_invert = pad.ring_invert;
        let cursor_divisor = pad.cursor_divisor;
        let report_hz = pad.report_hz;
        let proc_invert_x = pad.proc_invert_x;
        let proc_invert_y = pad.proc_invert_y;
        let proc_swap_xy = pad.proc_swap_xy;
        let auto_layer = match pad.auto_layer {
            Some(l) => quote! { Some(#l) },
            None => quote! { None },
        };
        let auto_layer_timeout_ms = pad.auto_layer_timeout_ms as u64;

        let device_init = quote! {
            let mut #device_ident = {
                use ::embassy_nrf::spim::{Frequency, Spim, Config, MODE_1};
                use ::rmk::input_device::pinnacle::{ActiveHigh, NrfChipSelect, Pinnacle, PinnacleConfig};
                use ::rmk::input_device::pointing::PointingDevice;

                let cs = NrfChipSelect::new(p.#cs_ident);
                let dr = #dr_init;

                let mut spi_config = Config::default();
                spi_config.frequency = Frequency::M2;
                spi_config.mode = MODE_1;
                let spi_bus = Spim::new(p.#instance_ident, Irqs, p.#sck_ident, p.#miso_ident, p.#mosi_ident, spi_config);

                let config = PinnacleConfig {
                    invert_x: #invert_x,
                    invert_y: #invert_y,
                    curved_overlay: #curved_overlay,
                    sleep: #sleep,
                    scroll_ring: #scroll_ring,
                    ring_width_percent: #ring_width_percent,
                    ring_degrees_per_tick: #ring_degrees_per_tick,
                    ring_invert: #ring_invert,
                    cursor_divisor: #cursor_divisor,
                };

                PointingDevice::<Pinnacle<_, _, _>>::with_report_hz(#id, spi_bus, cs, dr, config, #report_hz)
            };
        };
        device_initializers.push(Initializer {
            initializer: device_init,
            var_name: device_ident,
        });

        let processor_init = quote! {
            let #processor_config_ident = ::rmk::input_device::pointing::PointingProcessorConfig {
                device_id: #id,
                invert_x: #proc_invert_x,
                invert_y: #proc_invert_y,
                swap_xy: #proc_swap_xy,
                auto_layer: #auto_layer,
                auto_layer_timeout: ::embassy_time::Duration::from_millis(#auto_layer_timeout_ms),
                ..Default::default()
            };
            let mut #processor_ident = ::rmk::input_device::pointing::PointingProcessor::new(&keymap, #processor_config_ident);
        };
        processor_initializers.push(Initializer {
            initializer: processor_init,
            var_name: processor_ident,
        });
    }

    (device_initializers, processor_initializers)
}

/// `bind_interrupts!` lines for each pad's SPIM instance.
pub(crate) fn expand_pinnacle_interrupts(config: &[PinnacleConfig]) -> TokenStream2 {
    let lines = config.iter().map(|pad| {
        let instance_ident = format_ident!("{}", pad.spi.instance);
        quote! {
            #instance_ident => ::embassy_nrf::spim::InterruptHandler<::embassy_nrf::peripherals::#instance_ident>;
        }
    });
    quote! { #(#lines)* }
}
