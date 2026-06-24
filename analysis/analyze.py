#!/usr/bin/env python3
"""Analyze the TING capture: find broadband transients (clicks) vs speech.

Renders an overview (waveform / HF-energy / spectrogram) plus a zoom per
detected transient, and prints a feature table. Mechanical clicks show up as
very short, fast-rising, broadband (incl. >5 kHz) spikes; speech is slower and
concentrated below ~4 kHz.
"""
import sys, wave, numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy.signal import butter, sosfiltfilt, find_peaks, spectrogram

WAV = sys.argv[1] if len(sys.argv) > 1 else "capture.wav"
OUT = sys.argv[2] if len(sys.argv) > 2 else "/tmp/claude-1000/-home-jof/daba3961-86ab-46fa-a14c-de0209e8761f/scratchpad"

# --- load ---
w = wave.open(WAV, "rb")
sr = w.getframerate(); nch = w.getnchannels(); n = w.getnframes()
raw = np.frombuffer(w.readframes(n), dtype=np.int16).astype(np.float32)
raw = raw.reshape(-1, nch) if nch > 1 else raw.reshape(-1, 1)
raw /= 32768.0
# pick the loudest channel as the mic channel
ch = int(np.argmax(raw.std(axis=0)))
x = raw[:, ch]
t = np.arange(len(x)) / sr
print(f"file={WAV} sr={sr} ch={nch} dur={len(x)/sr:.1f}s  using channel {ch} "
      f"(per-channel std={np.round(raw.std(axis=0),4).tolist()})")

# --- HF transient envelope: high-pass >5 kHz, rectify, smooth ---
sos = butter(4, 5000/(sr/2), btype="high", output="sos")
hf = sosfiltfilt(sos, x)
win = int(0.003 * sr)                       # 3 ms smoothing
env = np.sqrt(np.convolve(hf**2, np.ones(win)/win, mode="same"))

# adaptive threshold: median + k*MAD
med = np.median(env); mad = np.median(np.abs(env - med)) + 1e-9
thr = med + 8 * 1.4826 * mad
peaks, props = find_peaks(env, height=thr, distance=int(0.15 * sr))  # 150ms refractory
print(f"HF-env threshold={thr:.5f}  candidate transients={len(peaks)}")

# --- features per transient ---
def feat(i):
    c = peaks[i]
    a, b = max(0, c - int(0.03*sr)), min(len(x), c + int(0.03*sr))
    seg = x[a:b]
    # spectral centroid
    sp = np.abs(np.fft.rfft(seg * np.hanning(len(seg))))
    fr = np.fft.rfftfreq(len(seg), 1/sr)
    cen = float((fr*sp).sum() / (sp.sum()+1e-9))
    hf_ratio = float(sp[fr>5000].sum() / (sp.sum()+1e-9))
    # rise time: 10%->90% of env peak within +/-15ms
    aa = max(0, c-int(0.015*sr))
    loc = env[aa:c+1]; pk = env[c]
    rise = (np.argmax(loc >= 0.9*pk) - np.argmax(loc >= 0.1*pk)) / sr * 1000 if pk>0 else 0
    return dict(t=c/sr, amp=float(env[c]), centroid_hz=cen, hf_ratio=hf_ratio, rise_ms=rise)

feats = [feat(i) for i in range(len(peaks))]
print("\n  #   time(s)   HFamp    centroid   HF>5k%   rise(ms)")
for i, f in enumerate(feats):
    print(f" {i:2d}  {f['t']:7.2f}  {f['amp']:.4f}  {f['centroid_hz']:8.0f}  "
          f"{100*f['hf_ratio']:5.1f}   {f['rise_ms']:5.1f}")

# --- overview figure ---
fig, ax = plt.subplots(3, 1, figsize=(16, 9), sharex=True)
ax[0].plot(t, x, lw=0.4); ax[0].set_ylabel("waveform")
ax[0].set_title(f"{WAV}  (channel {ch})")
ax[1].plot(t, env, lw=0.6, color="darkorange")
ax[1].axhline(thr, color="red", ls="--", lw=0.8, label=f"thr={thr:.4f}")
ax[1].plot(peaks/sr, env[peaks], "kv", ms=6)
for i, f in enumerate(feats):
    ax[1].annotate(str(i), (f["t"], env[peaks[i]]), fontsize=8)
ax[1].set_ylabel(">5kHz env"); ax[1].legend(loc="upper right", fontsize=8)
ff, tt, Sxx = spectrogram(x, sr, nperseg=1024, noverlap=768)
ax[2].pcolormesh(tt, ff, 10*np.log10(Sxx+1e-12), shading="gouraud", cmap="magma")
ax[2].set_ylabel("Hz"); ax[2].set_xlabel("time (s)"); ax[2].set_ylim(0, 16000)
plt.tight_layout(); plt.savefig(f"{OUT}/overview.png", dpi=110); plt.close()
print(f"\nwrote {OUT}/overview.png")

# --- per-transient zooms (grid) ---
if len(peaks):
    cols = 4; rows = int(np.ceil(len(peaks)/cols))
    fig, axz = plt.subplots(rows, cols, figsize=(16, 3*rows))
    axz = np.atleast_2d(axz)
    for i in range(rows*cols):
        a = axz.flat[i]
        if i < len(peaks):
            c = peaks[i]; a0, b0 = c-int(0.02*sr), c+int(0.02*sr)
            a0, b0 = max(0, a0), min(len(x), b0)
            a.plot((np.arange(a0, b0)/sr - c/sr)*1000, x[a0:b0], lw=0.6)
            a.set_title(f"#{i} t={c/sr:.2f}s cen={feats[i]['centroid_hz']:.0f}Hz",
                        fontsize=8)
            a.set_xlabel("ms")
        else:
            a.axis("off")
    plt.tight_layout(); plt.savefig(f"{OUT}/clicks_zoom.png", dpi=110); plt.close()
    print(f"wrote {OUT}/clicks_zoom.png")
