#!/bin/zsh
# Log the Parix 52's Bluetooth HID registry entry for $1 seconds (default 60).
# The entry id changes when the Mac re-creates the device, i.e. after the
# keyboard dropped its link and reconnected.
END=$(( $(date +%s) + ${1:-60} )); last=""
while [ $(date +%s) -lt $END ]; do
  id=$(ioreg -r -c IOHIDDevice -l 2>/dev/null | awk '/\+-o/{hdr=$0} /"Product" = "Parix 52"/{print hdr}' | grep -o 'id 0x[0-9a-f]*' | sort | tr '\n' ' ')
  cur="bt-hid=${id:-ABSENT}"
  [ "$cur" != "$last" ] && echo "$(date +%H:%M:%S)  $cur" && last="$cur"
  sleep 1
done
echo "$(date +%T)  monitor ended"
