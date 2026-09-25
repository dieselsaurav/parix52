#!/bin/zsh
# Copy exactly one UF2 to the next NICENANO bootloader mount, then report.
#   scripts/flash-uf2.sh build/parix52-rmk-central.uf2
# Waits up to 20 minutes for the volume. Never falls through to a second
# image: one invocation, one file, one board.
UF2=${1:?usage: flash-uf2.sh <file.uf2>}
[ -f "$UF2" ] || { echo "no such file: $UF2"; exit 1; }
for i in {1..2400}; do [ -d /Volumes/NICENANO ] && break; sleep 0.5; done
[ -d /Volumes/NICENANO ] || { echo "$(date +%T) timed out waiting for NICENANO"; exit 1; }
echo "$(date +%T) NICENANO mounted: $(grep -o 'Board-ID: [^ ]*' /Volumes/NICENANO/INFO_UF2.TXT 2>/dev/null)"
if cp "$UF2" "/Volumes/NICENANO/$(basename $UF2)"; then
  echo "$(date +%T) copied $(basename $UF2) ($(stat -f %z $UF2) bytes)"
else
  echo "$(date +%T) COPY FAILED, nothing written"; exit 1
fi
for i in {1..60}; do [ -d /Volumes/NICENANO ] || { echo "$(date +%T) volume gone, board rebooted into $(basename $UF2)"; exit 0; }; sleep 0.5; done
echo "$(date +%T) volume still mounted after 30 s, flash did not take"; exit 1
