#!/usr/bin/env python3
"""Generate the two Quindar tone WAVs for the TING.
Classic Quindar: intro 2525 Hz, outro 2475 Hz. Short bursts with raised-cosine
fades (no clicks). Mono 16-bit 48 kHz — tiny files, within the ~1 MB budget."""
import wave, struct, math, os

SR = 48000
DUR = 0.20          # 200 ms burst
FADE = 0.010        # 10 ms fades
AMP = 0.6

def gen(path, freq):
    n = int(SR * DUR); nf = int(SR * FADE)
    frames = bytearray()
    for i in range(n):
        env = 1.0
        if i < nf: env = 0.5 - 0.5 * math.cos(math.pi * i / nf)
        elif i > n - nf: env = 0.5 - 0.5 * math.cos(math.pi * (n - i) / nf)
        s = AMP * env * math.sin(2 * math.pi * freq * i / SR)
        frames += struct.pack("<h", int(s * 32767))
    w = wave.open(path, "wb")
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes(frames); w.close()
    print(f"{path}: {freq} Hz, {DUR*1000:.0f} ms, {os.path.getsize(path)} bytes")

here = os.path.dirname(os.path.abspath(__file__))
gen(os.path.join(here, "quindar_in.wav"), 2525)   # press / start
gen(os.path.join(here, "quindar_out.wav"), 2475)  # release / stop
