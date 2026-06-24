# some-ting --- TING EP-2350 as a push-to-talk trigger for Claude voice.
#
# Runs the stock app unchanged (import teenage), then emits a 2525 Hz Quindar
# "intro" burst when the FIRST/outer handle switch engages (talk-start) and a
# 2475 Hz "outro" when it releases (talk-stop). A PC daemon detects the tones.
#
# This build triggers on a *binary* switch (the outer switch fires at the start
# of travel; the analog ui.handle() only rises near the bottom). It also shows a
# switch diagnostic on the LEDs so we can confirm which sw() index is which.
#
# To restore stock behavior: delete this main.py from TINGDISK.

import teenage          # stock app (chdir's to /fat); registers its callback
import ui, spl, time

IN_SLOT, OUT_SLOT = 2, 3
try:
    f = open("quindar_in.wav", "rb");  spl.load_wav(IN_SLOT, f, "oneshot");  f.close()
    f = open("quindar_out.wav", "rb"); spl.load_wav(OUT_SLOT, f, "oneshot"); f.close()
except Exception:
    pass

_stock_cb = teenage.python_callback

time.sleep_ms(200)
REST = [ui.sw(i) for i in range(5)]    # switch states at boot (handle released)
TRIG = 4                               # switch that drives the tone (hypothesis: outer)
_pressed = False

def cb(message):
    global _pressed
    _stock_cb(message)                 # preserve all stock behavior
    try:
        s = [ui.sw(i) for i in range(5)]
    except Exception:
        return
    # Diagnostic LEDs: fx column = first sw in 0..3 that changed from rest (else off);
    #                  sample column = 3 if sw(4) changed from rest, else 0.
    diff = -1
    for i in range(4):
        if s[i] != REST[i]:
            diff = i; break
    ui.leds(diff, 3 if s[4] != REST[4] else 0)
    # Tone trigger: chosen switch changed from its rest state = engaged.
    on = (s[TRIG] != REST[TRIG])
    if on and not _pressed:
        _pressed = True
        spl.trigger(-1, IN_SLOT, True)     # Quindar intro = talk START
    elif (not on) and _pressed:
        _pressed = False
        spl.trigger(-1, OUT_SLOT, True)    # Quindar outro = talk STOP

ui.callback(cb)
