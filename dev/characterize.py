#!/usr/bin/env python3
"""Characterize a TING capture and recommend detector settings.

Uses the SAME windowed-DFT magnitude as the Rust detector (85 ms hann window at
2525/2475 Hz), so the recommended --threshold applies directly. Also reports the
tone's true dominant frequency per burst (in case an effect shifted it) and
writes a spectrogram + magnitude-over-time PNG.

Usage: characterize.py <capture.wav> [out.png]
"""
import sys, wave, numpy as np
import matplotlib; matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy.signal import spectrogram

WAV = sys.argv[1] if len(sys.argv) > 1 else "ting_cap.wav"
OUT = sys.argv[2] if len(sys.argv) > 2 else WAV.rsplit(".", 1)[0] + "_char.png"
F_IN, F_OUT = 2525.0, 2475.0

w = wave.open(WAV, "rb"); sr = w.getframerate(); nch = w.getnchannels(); n = w.getnframes()
x = np.frombuffer(w.readframes(n), dtype=np.int16).astype(np.float32).reshape(-1, nch) / 32768.0
x = x[:, int(np.argmax(x.std(0)))]
print(f"{WAV}: {sr} Hz, {len(x)/sr:.1f}s")

win = int(0.085 * sr); hop = int(0.021 * sr)
t = np.arange(win); hann = 0.5 - 0.5 * np.cos(2 * np.pi * t / (win - 1))
ref_in = hann * np.exp(-2j * np.pi * F_IN * t / sr)
ref_out = hann * np.exp(-2j * np.pi * F_OUT * t / sr)
fr = np.fft.rfftfreq(win, 1 / sr)
band = (fr >= 1000) & (fr <= 6000)

starts = range(0, len(x) - win, hop)
m_in = np.empty(len(starts)); m_out = np.empty(len(starts)); dom = np.empty(len(starts))
tt = np.empty(len(starts))
for i, a in enumerate(starts):
    seg = x[a:a + win]
    m_in[i] = abs(seg @ ref_in) / win
    m_out[i] = abs(seg @ ref_out) / win
    sp = np.abs(np.fft.rfft(seg * hann))
    sp[~band] = 0
    dom[i] = fr[np.argmax(sp)]
    tt[i] = a / sr
mmax = np.maximum(m_in, m_out)

# burst detection: frames well above the median (noise/voice) floor
noise = float(np.median(mmax))
peak = float(mmax.max())
thr_rel = max(noise * 4, peak * 0.25)
active = mmax > thr_rel
# group contiguous active frames into bursts
bursts = []
i = 0
while i < len(active):
    if active[i]:
        j = i
        while j < len(active) and active[j]:
            j += 1
        k = i + int(np.argmax(mmax[i:j]))
        kind = "INTRO" if m_in[k] >= m_out[k] else "OUTRO"
        bursts.append((tt[k], kind, m_in[k], m_out[k], dom[k]))
        i = j
    else:
        i += 1

print(f"\nnoise floor (median |mag|): {noise:.5f}    peak: {peak:.5f}")
print(f"bursts found: {len(bursts)}")
print(f"{'t(s)':>7} {'kind':>6} {'m2525':>8} {'m2475':>8} {'domFreq':>8} {'ratio':>6}")
intro_f, outro_f = [], []
for (ts, kind, mi, mo, d) in bursts:
    ratio = max(mi, mo) / (min(mi, mo) + 1e-9)
    print(f"{ts:7.2f} {kind:>6} {mi:8.4f} {mo:8.4f} {d:8.0f} {ratio:6.1f}")
    (intro_f if kind == "INTRO" else outro_f).append(d)

# recommendations
burst_peaks = [max(mi, mo) for (_, _, mi, mo, _) in bursts]
if burst_peaks:
    min_burst = min(burst_peaks)
    rec_thr = float(np.sqrt(noise * min_burst))  # geometric mean: well above noise, below weakest burst
    print(f"\n--- recommended ---")
    print(f"weakest burst: {min_burst:.4f}   noise: {noise:.5f}")
    print(f"--threshold {rec_thr:.4f}   (margin: {min_burst/rec_thr:.1f}x over burst, {rec_thr/noise:.1f}x over noise)")
    if intro_f:
        print(f"intro dominant freq: {np.median(intro_f):.0f} Hz (target {F_IN:.0f})")
    if outro_f:
        print(f"outro dominant freq: {np.median(outro_f):.0f} Hz (target {F_OUT:.0f})")
    fi = np.median(intro_f) if intro_f else F_IN
    fo = np.median(outro_f) if outro_f else F_OUT
    shifted = abs(fi - F_IN) > 30 or abs(fo - F_OUT) > 30
    cmd = f"some-ting-listen --threshold {rec_thr:.4f}"
    if shifted:
        cmd += f" --f-intro {fi:.0f} --f-outro {fo:.0f}"
        print("** tones are SHIFTED from 2525/2475 — add the --f-intro/--f-outro flags **")
    print(f"\nsuggested: {cmd}")
else:
    print("\nNO BURSTS detected above the floor — tone too weak or not present.")
    print("Check: clean preset, TING volume up a bit, and that you squeezed during the recording.")

# plot
fig, ax = plt.subplots(2, 1, figsize=(15, 8), sharex=True)
ff, ts2, Sxx = spectrogram(x, sr, nperseg=2048, noverlap=1792)
ax[0].pcolormesh(ts2, ff, 10 * np.log10(Sxx + 1e-12), shading="gouraud", cmap="magma")
ax[0].axhline(F_IN, color="cyan", lw=0.5, ls="--"); ax[0].axhline(F_OUT, color="lime", lw=0.5, ls="--")
ax[0].set_ylim(0, 6000); ax[0].set_ylabel("Hz"); ax[0].set_title(f"{WAV}")
ax[1].plot(tt, m_in, label="m2525", color="c"); ax[1].plot(tt, m_out, label="m2475", color="g")
ax[1].axhline(noise, color="grey", ls=":", label="noise")
if burst_peaks:
    ax[1].axhline(rec_thr, color="red", ls="--", label=f"rec thr {rec_thr:.4f}")
ax[1].set_xlabel("s"); ax[1].set_ylabel("detector |mag|"); ax[1].legend(fontsize=8)
plt.tight_layout(); plt.savefig(OUT, dpi=110); plt.close()
print(f"\nwrote {OUT}")
