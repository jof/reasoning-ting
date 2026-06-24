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

TH_HI = 0.55          # squeeze past this -> "pressed"  (tune after testing)
TH_LO = 0.30          # fall below this  -> "released"
_pressed = False

def cb(message):
    global _pressed
    _stock_cb(message)                  # preserve every stock behavior
    try:
        h = ui.handle()
    except Exception:
        return
    if (not _pressed) and h > TH_HI:
        _pressed = True
        ui.leds(3, 3)                   # visual cue: keyed (may flicker vs stock)
        spl.trigger(-1, IN_SLOT, True)  # Quindar intro = talk START
    elif _pressed and h < TH_LO:
        _pressed = False
        ui.leds(-1, 0)
        spl.trigger(-1, OUT_SLOT, True) # Quindar outro = talk STOP

ui.callback(cb)
