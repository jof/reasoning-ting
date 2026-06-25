# some-ting --- TING EP-2350 as a push-to-talk trigger for Claude voice.
#
# Runs the stock app unchanged (import teenage), then:
#  - outer handle switch engages -> 2525 Hz "intro" (talk START)
#  - handle releases             -> 2475 Hz "outro" (talk STOP)
#  - white button                -> 3000 Hz "submit" (PC daemon taps Enter)
# A PC daemon Goertzel-detects the tones and drives Claude's voice dictation
# (push-to-talk) + submit, so you can dictate multiple chunks then send.
#
# Use the CLEAN effect preset and a moderate volume (the green knob under the
# lid): effects color the tone and the 2 VRMS output overloads a mic input.
#
# To restore stock behavior: delete this main.py from TINGDISK.

import teenage          # stock app (chdir's to /fat); registers its callback
import ui, spl, time

IN_SLOT, OUT_SLOT, SUBMIT_SLOT = 2, 3, 1  # sample slots repurposed for our tones

def load_tones():
    # (Re)load our tones into their slots. The stock app reloads the sample pack
    # into ALL slots whenever the USB drive is mounted/ejected (examine_drive),
    # which clobbers our tones — so we also re-assert on drive events (see cb).
    try:
        f = open("quindar_in.wav", "rb");  spl.load_wav(IN_SLOT, f, "oneshot");  f.close()
        f = open("quindar_out.wav", "rb"); spl.load_wav(OUT_SLOT, f, "oneshot"); f.close()
        f = open("submit.wav", "rb");      spl.load_wav(SUBMIT_SLOT, f, "oneshot"); f.close()
    except Exception:
        pass

load_tones()

_stock_cb = teenage.python_callback

# Outer handle switch: engaged == ui.sw(SW) == 1, released == 0 (measured).
# Use the ABSOLUTE state (not a boot-time baseline) so it can't invert if the
# handle happens to be squeezed while the unit powers on.
SW = 4
_pressed = False

def cb(message):
    global _pressed
    t = message >> 16
    v = message & 0xFFFF
    # White button (sample-play, mess_val 0) -> submit tone (PC daemon hits Enter).
    if t == 1 and v == 0:
        spl.trigger(-1, SUBMIT_SLOT, True)
        return                            # don't also play the stock sample
    if t == 2 and v == 0:
        return                            # swallow white-button release
    _stock_cb(message)                    # preserve all other stock behavior
    if t == 4:                            # USB drive mount/eject -> stock reloaded
        load_tones()                      # the sample pack into all slots; re-assert ours
        return
    try:
        on = (ui.sw(SW) == 1)             # engaged == 1 (released == 0), absolute
    except Exception:
        return
    if on and not _pressed:
        _pressed = True
        spl.trigger(-1, IN_SLOT, True)    # Quindar intro = talk START
    elif (not on) and _pressed:
        _pressed = False
        spl.trigger(-1, OUT_SLOT, True)   # Quindar outro = talk STOP

ui.callback(cb)
