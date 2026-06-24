# some-ting --- TING EP-2350 as a push-to-talk trigger for Claude voice.
#
# Runs the stock app unchanged (import teenage), then emits a 2525 Hz Quindar
# "intro" burst when the outer handle switch engages (talk-start) and a 2475 Hz
# "outro" when it releases (talk-stop). A PC daemon Goertzel-detects the tones
# and toggles Claude's voice dictation.
#
# Use the CLEAN effect preset and a moderate volume (the green knob under the
# lid): effects color the tone and the 2 VRMS output overloads a mic input.
#
# To restore stock behavior: delete this main.py from TINGDISK.

import teenage          # stock app (chdir's to /fat); registers its callback
import ui, spl, time

IN_SLOT, OUT_SLOT = 2, 3                 # sacrifice sample slots 2/3 for the tones
try:
    f = open("quindar_in.wav", "rb");  spl.load_wav(IN_SLOT, f, "oneshot");  f.close()
    f = open("quindar_out.wav", "rb"); spl.load_wav(OUT_SLOT, f, "oneshot"); f.close()
except Exception:
    pass

_stock_cb = teenage.python_callback

time.sleep_ms(200)
SW = 4                                    # outer/first handle switch (mic-on gate)
REST = ui.sw(SW)                          # its released state at boot
_pressed = False

def cb(message):
    global _pressed
    _stock_cb(message)                    # preserve all stock behavior
    try:
        on = (ui.sw(SW) != REST)          # outer switch engaged
    except Exception:
        return
    if on and not _pressed:
        _pressed = True
        spl.trigger(-1, IN_SLOT, True)    # Quindar intro = talk START
    elif (not on) and _pressed:
        _pressed = False
        spl.trigger(-1, OUT_SLOT, True)   # Quindar outro = talk STOP

ui.callback(cb)
