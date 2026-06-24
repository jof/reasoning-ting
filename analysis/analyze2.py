#!/usr/bin/env python3
"""Zoom into the template/click region, low threshold, shape-based view.
Counts every transient after t0 and overlays their normalized shapes so we can
see whether there are TWO distinct click morphologies (ON vs OFF)."""
import sys, wave, numpy as np
import matplotlib; matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy.signal import butter, sosfiltfilt, find_peaks

WAV = "capture.wav"; OUT = "."
T0 = 22.0   # analyze only after speech ends (pure clicks)
w = wave.open(WAV, "rb"); sr = w.getframerate(); nch = w.getnchannels(); n = w.getnframes()
raw = np.frombuffer(w.readframes(n), dtype=np.int16).astype(np.float32).reshape(-1, nch)/32768.0
x = raw[:, int(np.argmax(raw.std(0)))]

# band-pass 300-4000 Hz (where the click energy lives), envelope
sos = butter(4, [300/(sr/2), 4000/(sr/2)], btype="band", output="sos")
bp = sosfiltfilt(sos, x)
win = int(0.004*sr); env = np.sqrt(np.convolve(bp**2, np.ones(win)/win, "same"))

seg0 = int(T0*sr)
sub = env[seg0:]
med = np.median(sub); mad = np.median(np.abs(sub-med))+1e-9
thr = med + 4*1.4826*mad                      # lower (4 MAD) to catch weak clicks
peaks, _ = find_peaks(sub, height=thr, distance=int(0.4*sr))
peaks = peaks + seg0
print(f"threshold={thr:.5f}  clicks in t>{T0}s: {len(peaks)}")
print("  #   time(s)   peakamp   amp_dB")
amps=[]
for i,c in enumerate(peaks):
    amps.append(env[c]);
    print(f" {i:2d}  {c/sr:7.2f}   {env[c]:.4f}   {20*np.log10(env[c]+1e-9):6.1f}")

# overlay normalized shapes, aligned on peak
fig, ax = plt.subplots(1,2, figsize=(15,5))
for i,c in enumerate(peaks):
    a,b = c-int(0.01*sr), c+int(0.025*sr)
    seg = x[a:b];
    ax[0].plot((np.arange(a,b)/sr - c/sr)*1000, seg/ (np.max(np.abs(seg))+1e-9),
               lw=0.8, label=f"#{i} {c/sr:.1f}s")
ax[0].set_title("click shapes, peak-aligned, amplitude-normalized")
ax[0].set_xlabel("ms"); ax[0].legend(fontsize=8)
ax[1].bar(range(len(amps)), 20*np.log10(np.array(amps)+1e-9))
ax[1].set_title("per-click peak level (dB)"); ax[1].set_xlabel("click #"); ax[1].set_ylabel("dB")
plt.tight_layout(); plt.savefig("clicks_compare.png", dpi=120); plt.close()
print("wrote clicks_compare.png")
