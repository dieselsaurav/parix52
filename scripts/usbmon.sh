#!/bin/zsh
# Log the Parix 52's USB session id every half second for $1 seconds (default 60).
# A new session id = the board re-enumerated = it rebooted or was replugged.
END=$(( $(date +%s) + ${1:-60} ))
last=""
while [ $(date +%s) -lt $END ]; do
  sid=$(ioreg -p IOUSB -l -w0 2>/dev/null | grep -A30 '"USB Product Name" = "Parix 52"' | grep -o '"sessionID" = [0-9]*' | head -1 | awk '{print $3}')
  boot=$([ -d /Volumes/NICENANO ] && echo BOOTLOADER || echo -)
  cur="kbd=${sid:-ABSENT} boot=$boot"
  [ "$cur" != "$last" ] && echo "$(date +%H:%M:%S.%N | cut -c1-12)  $cur" && last="$cur"
  sleep 0.5
done
echo "$(date +%T)  monitor ended"
