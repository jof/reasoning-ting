#!/usr/bin/env python3
"""Analyze a PTT capture: show the spectrum and how cleanly the 2525/2475 Hz
Quindar tones come through (this also tells us what the PC daemon must detect)."""
import sys, wave, numpy as np
import matplotlib; matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy.signal import spectrogram

WAV = sys.argv[1] if len(sys.argv) > 1 else "ptt.wav"
OUT = sys.argv[2] if len(sys.argv) > 2 else "ptt_spectrum.png"
F_IN, F_OUT = 2525.0, 2475.0

w = wave.open(WAV, "rb"); sr = w.getframerate(); nch = w.getnchannels(); n = w.getnframes()
x = np.frombuffer(w.readframes(n), dtype=np.int16).astype(np.float32).reshape(-1, nch) / 32768.0
x = x[:, int(np.argmax(x.std(0)))]
print(f"{WAV}: sr={sr} dur={len(x)/sr:.1f}s")

# Goertzel-ish: sliding magnitude at the two target tones
def tone_env(sig, f, win=2048, hop=256):
    t = np.arange(win); ref = np.exp(-2j*np.pi*f*t/sr) * np.hanning(win)
    mags = []
    for a in range(0, len(sig)-win, hop):
        mags.append(np.abs(np.dot(sig[a:a+win], ref)) / win)
    return np.array(mags), hop
m_in, hop = tone_env(x, F_IN)
m_out, _  = tone_env(x, F_OUT)
tt = np.arange(len(m_in)) * hop / sr
# broadband ref to judge "cleanliness" (energy outside the two tones)
print(f"peak mag @2525={m_in.max():.4f}  @2475={m_out.max():.4f}  "
      f"noise floor(med)={np.median(np.r_[m_in,m_out]):.5f}")

fig, ax = plt.subplots(2, 1, figsize=(15, 8), sharex=True)
ff, ts, Sxx = spectrogram(x, sr, nperseg=2048, noverlap=1792)
ax[0].pcolormesh(ts, ff, 10*np.log10(Sxx+1e-12), shading="gouraud", cmap="magma")
ax[0].axhline(F_IN, color="cyan", lw=0.5, ls="--"); ax[0].axhline(F_OUT, color="lime", lw=0.5, ls="--")
ax[0].set_ylim(0, 6000); ax[0].set_ylabel("Hz"); ax[0].set_title(f"{WAV} spectrogram (0-6kHz)")
ax[1].plot(tt, m_in, label="2525 (intro)", color="c")
ax[1].plot(tt, m_out, label="2475 (outro)", color="g")
ax[1].set_xlabel("s"); ax[1].set_ylabel("tone magnitude"); ax[1].legend()
plt.tight_layout(); plt.savefig(OUT, dpi=110); plt.close()
print("wrote", OUT)
