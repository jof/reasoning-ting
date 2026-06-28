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

# slot -> our WAV file. Single source of truth for load_tones() and play().
TONES = {
    IN_SLOT:     "quindar_in.wav",
    OUT_SLOT:    "quindar_out.wav",
    SUBMIT_SLOT: "submit.wav",
}

def load_one(slot):
    # Load our WAV into a slot, clobbering whatever's there. Swallow errors so a
    # transient read failure can never take down the callback / the stock app.
    try:
        f = open(TONES[slot], "rb"); spl.load_wav(slot, f, "oneshot"); f.close()
    except Exception:
        pass

def load_tones():
    for slot in TONES:
        load_one(slot)

def play(slot):
    # Re-assert our WAV into the slot, then trigger it. We reload every time
    # because the slot may have been clobbered by the stock ROM sample pack with
    # NO Python event to notify us:
    #   - USB mount/eject runs examine_drive (handled separately in cb), but
    #   - power save (5 min idle on battery) reinitializes the spl engine back to
    #     the ROM pack (slot 2=gunshot, 3=monkey-boy, 1=alarm) and fires no event
    #     on wake — so a boot-time load is silently lost. Reloading at trigger
    #     time is the only wake-proof option. (load_wav in the callback is already
    #     proven safe — the t==4 path below does it.) A flash read is cheap and
    #     doesn't wear the chip; the few-ms cost is immaterial for push-to-talk.
    load_one(slot)
    spl.trigger(-1, slot, True)

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
        play(SUBMIT_SLOT)
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
        play(IN_SLOT)                     # Quindar intro = talk START
    elif (not on) and _pressed:
        _pressed = False
        play(OUT_SLOT)                    # Quindar outro = talk STOP

ui.callback(cb)
