# reasoning-ting 🎙️🤖

> What if you could talk to Claude Code by **squeezing a tiny synth** like a
> walkie-talkie? 🤏✨ Now you can.

This is a deeply silly, earnest project that turns the **Teenage
Engineering TING** (the adorable EP-2350 handheld mic/sampler 🎛️) into a
**hardware push-to-talk button** for Claude Code's voice dictation.

## How it works (the 10-second version) ⚡

```
   🤏 squeeze handle  ──▶  🔊 2525 Hz beep  ──▶  🎧 PC hears it  ──▶  ⌨️  F12 down → 🎙️ recording
   ✋ let go           ──▶  🔊 2475 Hz beep  ──▶  🎧 PC hears it  ──▶  ⌨️  F12 up   → 📝 transcript drops in
   ⚪ white button     ──▶  🔊 3000 Hz beep  ──▶  🎧 PC hears it  ──▶  ⌨️  Enter    → 🚀 sent!
```

Squeeze 🤏 → talk 🗣️ → release ✋ → tap the white button ⚪ → **off your words
go**. No drivers, no patching, no flashing. Just **beeps**. Good old-fashioned,
Apollo-era NASA-flavor **beeps**. 🌝📡

> 🛰️ *Yes, those are real [Quindar tones](https://en.wikipedia.org/wiki/Quindar_tones)
> — the "beep" that bookended NASA mission-control comms. Houston, we have a
> dictation.*

Oh, and that white button? ⚪ It's just **Enter**. So when Claude throws up one of
those *"proceed? [yes]"* prompts and the default is exactly what you wanted, you
can sit back and **tap-tap-tap** your way through them — accept, accept, accept,
one after another, no keyboard required. 👆👆👆 It's basically a big shiny YES
button. ✅

## Why though 🤷

Because the handle on the TING is *begging* to be squeezed, and push-to-talk is
the most satisfying input modality ever invented, and your keyboard's voice key
was feeling lonely. Pick a reason. They're all good. This is a **toy**, lovingly
over-engineered. 🧸

## The two halves 🧩

| 📦 | What | Where |
|----|------|-------|
| 🤚 **The squeeze** | Tiny MicroPython layer that makes the TING beep on squeeze/release/click | [`deploy/`](deploy/) |
| 👂 **The listener** | Cross-platform Rust app that hears the beeps and drives Claude | [`listener/`](listener/) |

The listener ships as a friendly **menu-bar/tray app** 🟢 (green = listening,
🔴 red = recording) *and* a no-nonsense headless CLI for the terminal goblins. 🧙
One Rust core, two faces, runs on Linux + macOS, builds reproducibly with Nix. ❄️

## Get going 🏁

1. 🎛️ Drop our `main.py` + tone WAVs onto the TING (from [`deploy/`](deploy/)),
   then **eject the disk and restart it** — the mod only runs when the device
   owns its own filesystem.
2. ⌨️ Bind `f12` → voice push-to-talk in Claude Code (the app has a one-click
   button for this) and restart Claude.
3. 🎧 Make the TING your system's default mic.
4. 🚀 Launch `reasoning-ting` (tray) or `reasoning-ting-listen` (CLI) — `nix run .#gui`
   works out of the box.
5. 🤏🗣️✋⚪ Squeeze, speak, release, send. Welcome to the future. It beeps.

## Want the gory details? 🔬

All the wonderful, wordy, here's-exactly-why-it-works prose lives in
**[📖 the full guide](docs/GUIDE.md)** — firmware archaeology, the great
power-save sample-clobbering mystery 🕵️, the detector internals, and the complete
deployment runbook. Packaging strategy is in
**[📦 PACKAGING.md](docs/PACKAGING.md)**.

---

*A toy. A whimsy. A small machine that beeps so you don't have to type.* 💛
