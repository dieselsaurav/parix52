#!/bin/zsh
# Flash over the bootloader's serial port instead of the UF2 drive.
#   scripts/flash-serial.sh build/parix52-rmk-central.hex
# The Adafruit bootloader 0.6.0 on the nice!nano drops off USB in the middle
# of a UF2 copy (adafruit/Adafruit_nRF52_Bootloader#212); serial DFU does not
# use the drive. The board must already be in the bootloader.
HEX=${1:?usage: flash-serial.sh <file.hex>}
[ -f "$HEX" ] || { echo "no such file: $HEX"; exit 1; }
PORT=$(ls /dev/cu.usbmodem* 2>/dev/null | head -1)
[ -n "$PORT" ] || { echo "no bootloader serial port found (is the board in the bootloader?)"; exit 1; }
ZIP=${HEX%.hex}-dfu.zip
adafruit-nrfutil dfu genpkg --dev-type 0x0052 --application "$HEX" "$ZIP" >/dev/null || exit 1
echo "$(date +%T) flashing $(basename $HEX) over $PORT"
adafruit-nrfutil dfu serial --package "$ZIP" -p "$PORT" -b 115200 --singlebank
