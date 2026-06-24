#!/usr/bin/env python3
"""Compare noise floor across regions to test whether mic-ON (handle held)
is distinguishable from mic-OFF (handle released) WITHOUT any tone.
If the silent-hold region differs clearly from the off regions, we can detect
handle state from the noise floor alone."""
import wave, numpy as np
from scipy.signal import welch

w = wave.open("capture.wav","rb"); sr=w.getframerate(); nch=w.getnchannels(); n=w.getnframes()
x = np.frombuffer(w.readframes(n),dtype=np.int16).astype(np.float32).reshape(-1,nch)/32768.0
x = x[:, int(np.argmax(x.std(0)))]

# regions (s): label, (start,end) -- per the capture prompt timeline
regions = [
    ("baseline  (likely mic OFF, pre-click)", 0.4, 2.6),
    ("speaking 1-5 (mic ON, voice)",          5.0, 9.0),
    ("SILENT HOLD (mic ON, no speech)",       10.5, 14.5),
    ("speaking fox (mic ON, voice)",          15.5, 19.5),
    ("after click-off (likely mic OFF)",      22.5, 24.8),
    ("end tail (likely mic OFF)",             38.5, 41.0),
]
def band_rms(seg, lo, hi):
    f, P = welch(seg, sr, nperseg=4096)
    m = (f>=lo)&(f<hi)
    return np.sqrt(np.trapezoid(P[m], f[m])+1e-20)

print(f"sr={sr} dur={len(x)/sr:.1f}s")
print(f"{'region':42s} {'full dB':>8} {'20-200Hz':>9} {'200-2k':>8} {'2k-8k':>8} {'8k-20k':>8}")
for lab, a, b in regions:
    seg = x[int(a*sr):int(b*sr)]
    full = 20*np.log10(np.sqrt(np.mean(seg**2))+1e-12)
    bands = [20*np.log10(band_rms(seg,lo,hi)+1e-12) for lo,hi in
             [(20,200),(200,2000),(2000,8000),(8000,20000)]]
    print(f"{lab:42s} {full:8.1f} " + " ".join(f"{b:8.1f}" for b in bands))
