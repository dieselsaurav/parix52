MEMORY
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* These values correspond to the nRF52840 WITH Adafruit nRF52 bootloader */
  /* App region ends at 0xEC000: RMK storage occupies 0xEC000-0xF4000
     (config/keyboard.toml [storage], 8 sectors) and the Adafruit bootloader
     starts at 0xF4000. Keep LENGTH = 0xEC000 - 0x1000 so a build that
     outgrows the region fails to link instead of overlapping its storage. */
  FLASH : ORIGIN = 0x00001000, LENGTH = 940K
  RAM : ORIGIN = 0x20000008, LENGTH = 255K

  /* These values correspond to the nRF52840 */
  /* FLASH : ORIGIN = 0x00000000, LENGTH = 1024K */
  /* RAM : ORIGIN = 0x20000000, LENGTH = 256K */
}