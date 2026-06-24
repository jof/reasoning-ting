#!/usr/bin/env bash
# Guided click-capture for the TING push-to-talk experiment.
# Records the default PipeWire/Pulse source while printing timed prompts.
set -euo pipefail

OUT="${1:-/tmp/claude-1000/-home-jof/daba3961-86ab-46fa-a14c-de0209e8761f/scratchpad/capture.wav}"
DUR=42

echo "Recording to: $OUT"
echo "Capturing the DEFAULT source (should be the TING / Generic USB Audio)."
echo "Stereo, 48 kHz, 16-bit (front mic may be on only one channel). Follow the prompts. Get ready..."
echo

# Start recording in background. -t bounds the duration; ffmpeg exits on its own.
ffmpeg -hide_banner -loglevel error -f pulse -i default -ac 2 -ar 48000 -t "$DUR" -y "$OUT" &
FF=$!

start=$(date +%s.%N)
mark() { # mark <seconds-from-start> <message>
  local target=$1; shift
  while :; do
    now=$(date +%s.%N)
    el=$(awk "BEGIN{print $now-$start}")
    cmp=$(awk "BEGIN{print ($el>=$target)?1:0}")
    [ "$cmp" = "1" ] && break
    sleep 0.05
  done
  printf '[%5.1fs] %s\n' "$target" "$*"
}

mark  0.0 "BASELINE — stay completely silent, don't touch the mic"
mark  3.0 ">>> CLICK THE BUTTON *ON* NOW <<<"
mark  4.5 "SPEAK slowly: 'one ... two ... three ... four ... five'"
mark  9.5 "STOP talking, but KEEP THE BUTTON ON — long silent pause (this is the key test)"
mark 15.0 "SPEAK again: 'the quick brown fox jumps over the lazy dog'"
mark 20.5 ">>> CLICK THE BUTTON *OFF* NOW <<<"
mark 22.0 "Silent..."
mark 25.0 "TEMPLATE PHASE: isolated clicks, NO talking. Leave ~1.5s between each."
mark 26.0 ">>> CLICK ON  (1) <<<"
mark 28.0 ">>> CLICK OFF (1) <<<"
mark 30.0 ">>> CLICK ON  (2) <<<"
mark 32.0 ">>> CLICK OFF (2) <<<"
mark 34.0 ">>> CLICK ON  (3) <<<"
mark 36.0 ">>> CLICK OFF (3) <<<"
mark 38.0 "Done clicking — stay silent until recording stops"
mark 41.5 "Wrapping up..."

wait "$FF"
echo
echo "Saved: $OUT"
ls -l "$OUT"
