#!/bin/zsh
# Read the keyboard's USB serial log (rmk `usb_log`). Ctrl-C to stop.
#   scripts/usblog.sh [seconds] > build/usblog.txt
DEV=$(ls /dev/tty.usbmodem* 2>/dev/null | head -1)
[ -n "$DEV" ] || { echo "no usbmodem device"; exit 1; }
echo "reading $DEV"
stty -f "$DEV" 115200 raw -echo
if [ -n "$1" ]; then timeout "$1" cat "$DEV"; else cat "$DEV"; fi
