# some-ting --- TING EP-2350 as a push-to-talk trigger for Claude voice.
#
# Runs the stock app unchanged (import teenage: mic passthrough, samples, fx),
# then wraps the input callback so squeezing the handle emits a 2525 Hz Quindar
# "intro" burst (talk-start) and releasing emits a 2475 Hz "outro" burst
# (talk-stop). A PC daemon detects the tones and toggles voice dictation.
#
# To restore stock behavior: delete this main.py from TINGDISK.

import teenage          # stock application (also chdir's to /fat); registers its callback
import ui, spl

# teenage.py left cwd at /fat, so these load straight off TINGDISK.
# We sacrifice sample slots 2 and 3 to hold the two Quindar tones.
IN_SLOT, OUT_SLOT = 2, 3
try:
    f = open("quindar_in.wav", "rb");  spl.load_wav(IN_SLOT, f, "oneshot");  f.close()
    f = open("quindar_out.wav", "rb"); spl.load_wav(OUT_SLOT, f, "oneshot"); f.close()
except Exception:
    pass

_stock_cb = teenage.python_callback

# Trigger at the FIRST/outer switch (start of travel), not the bottom stop.
# Lowered from 0.55; we also show a live handle meter on the LEDs to calibrate.
TH_HI = 0.10          # squeeze past this -> "pressed"
TH_LO = 0.05          # fall below this  -> "released"
_pressed = False

def cb(message):
    global _pressed
    _stock_cb(message)                  # preserve every stock behavior
    try:
        h = ui.handle()
    except Exception:
        return
    # live handle meter: LED column level 0..3 tracks how far the handle is.
    lvl = int(h * 3.999)
    if lvl > 3:
        lvl = 3
    ui.leds(lvl, lvl)
    if (not _pressed) and h > TH_HI:
        _pressed = True
        spl.trigger(-1, IN_SLOT, True)  # Quindar intro = talk START
    elif _pressed and h < TH_LO:
        _pressed = False
        spl.trigger(-1, OUT_SLOT, True) # Quindar outro = talk STOP

ui.callback(cb)
