//! Initialize flash boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::hardware::ChipSeries;
use rmk_config::resolved::{Hardware, Layout};

/// `layout` is the central's keymap; a peripheral stores no keymap and passes `None`.
pub(crate) fn expand_flash_init(hardware: &Hardware, layout: Option<&Layout>) -> TokenStream2 {
    if hardware.storage.is_none() {
        // This config actually does nothing if storage is disabled
        return quote! {
            // let storage_config = ::rmk::config::StorageConfig::default();
            // let flash = ::rmk::DummyFlash::new();
        };
    }
    let storage = hardware.storage.as_ref().unwrap();
    let num_sectors = storage.num_sectors;
    let start_addr = storage.start_addr;
    if let Some(layout) = layout {
        check_storage_capacity(hardware, layout, num_sectors);
    }
    let clear_storage = storage.clear_storage;
    let clear_layout = storage.clear_layout;
    let mut flash_init = quote! {
        let storage_config = ::rmk::config::StorageConfig {
            num_sectors: #num_sectors,
            start_addr: #start_addr,
            clear_storage: #clear_storage,
            clear_layout: #clear_layout
        };
    };
    flash_init.extend(
    match hardware.chip.series {
            ChipSeries::Stm32 => {
                quote! {
                    let flash = ::rmk::storage::async_flash_wrapper(::embassy_stm32::flash::Flash::new_blocking(p.FLASH));
                }
            }
            ChipSeries::Nrf52 => {
                quote! {
                    let flash = ::nrf_mpsl::Flash::take(mpsl, p.NVMC);
                }
            }
            ChipSeries::Rp2040 => {
                quote! {
                    const FLASH_SIZE: usize = 2 * 1024 * 1024;
                    let flash = ::embassy_rp::flash::Flash::<_, ::embassy_rp::flash::Async, FLASH_SIZE>::new(
                        p.FLASH,
                        p.DMA_CH1,
                        Irqs,
                    );
                }
            }
            ChipSeries::Esp32 => quote! {
                let flash = ::rmk::storage::async_flash_wrapper(::esp_storage::FlashStorage::new(p.FLASH));
            },
        }
    );

    flash_init
}


/// Refuse a storage too small for what RMK writes into it.
///
/// `Storage::initialize_storage_with_config` stores every key of every layer
/// as its own item -- rows x cols x layers of them, each about 16 bytes on
/// a 4-byte-word flash -- plus the layout, behaviour and macro blobs and the
/// BLE bonds. `sequential-storage`'s map keeps one page free as a migration
/// buffer, so all of that live data must fit in `num_sectors - 1` pages.
/// When it does not, the first boot stores the keymap only partially and
/// every later write fails with `FullStorage`: Vial edits appear to take and
/// are gone on the next power-up, with nothing to see but a defmt error.
/// That is a config mistake, so it is caught here, at build time.
fn check_storage_capacity(hardware: &Hardware, layout: &Layout, num_sectors: u8) {
    // Only chips whose erase page this macro knows. Others keep the old
    // runtime-only behaviour.
    let page: usize = match hardware.chip.series {
        ChipSeries::Nrf52 | ChipSeries::Rp2040 => 4096,
        _ => return,
    };
    let (rows, cols, layers) = (layout.rows as usize, layout.cols as usize, layout.layers as usize);
    let per_key = 16; // 8-byte item header + ~4-byte key + ~4-byte value, word aligned
    let keymap = rows * cols * layers * per_key;
    let fixed = 4096; // layout + behaviour + macros + 4 bonds + peers, generously
    let need = keymap + fixed;
    let have = (num_sectors as usize).saturating_sub(1) * page;
    if need > have {
        let sectors = need.div_ceil(page) + 1;
        panic!(
            "[storage] num_sectors = {num_sectors} gives {have} bytes of usable storage but the \
             keymap alone ({rows} x {cols} x {layers} keys x {per_key} bytes) plus config needs \
             about {need}. Set num_sectors = {sectors} or more, and move start_addr down to match."
        );
    }
}
