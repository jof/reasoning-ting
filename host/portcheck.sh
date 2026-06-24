#!/usr/bin/env bash
# Where is the TING, and does its USB controller share with Bluetooth?
ting=$(lsusb | grep -i '2367:0620')
[ -z "$ting" ] && { echo "TING NOT FOUND on USB (replug?)"; exit 0; }
echo "TING: $ting"
tbus=$(echo "$ting" | sed -E 's/Bus 0*([0-9]+) Device.*/\1/')
tname=$(echo "$ting" | grep -oiE 'EP-2350|RP2350 Boot')
echo "  -> bus $tbus, mode: ${tname:-?}"

ctrl_of_bus(){ # echo PCI controller addr for usb bus number
  local b=$1; local p; p=$(readlink -f /sys/bus/usb/devices/usb$b 2>/dev/null)
  # parent dir is the PCI controller
  echo "$p" | grep -oE '0000:[0-9a-f]{2}:[0-9a-f]{2}\.[0-9]' | tail -1
}
tctrl=$(ctrl_of_bus $tbus)
echo "  -> controller: $tctrl"

# Bluetooth + audio controllers
btline=$(lsusb | grep -i '8087:0032'); bbus=$(echo "$btline"|sed -E 's/Bus 0*([0-9]+) Device.*/\1/')
auline=$(lsusb | grep -i '26ce:0a06'); abus=$(echo "$auline"|sed -E 's/Bus 0*([0-9]+) Device.*/\1/')
bctrl=$(ctrl_of_bus ${bbus:-0}); actrl=$(ctrl_of_bus ${abus:-0})
echo "Bluetooth(8087:0032) on bus ${bbus:-?} controller $bctrl"
echo "USB audio(26ce:0a06) on bus ${abus:-?} controller $actrl"

echo "---"
if [ -n "$tctrl" ] && { [ "$tctrl" = "$bctrl" ] || [ "$tctrl" = "$actrl" ]; }; then
  echo "VERDICT: ❌ BAD — TING shares a controller with Bluetooth/audio. Try another port."
elif [ -n "$tctrl" ]; then
  echo "VERDICT: ✅ GOOD — TING is on its OWN controller ($tctrl), isolated from BT/audio."
  echo "Other devices on TING's controller:"
  for b in $(ls -d /sys/bus/usb/devices/usb* 2>/dev/null); do
    bn=$(basename $b|sed 's/usb//'); [ "$(ctrl_of_bus $bn)" = "$tctrl" ] && lsusb | grep -E "^Bus 0*$bn " | grep -v 'root hub'
  done | sed 's/^/   /'
fi
