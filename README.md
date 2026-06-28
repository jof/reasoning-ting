# reasoning-ting

Hacking the **Teenage Engineering TING (EP-2350)** into a hardware push-to-talk
trigger for Claude Code's voice dictation. Squeeze the handle → a tone is emitted
→ a PC daemon detects it → keydown/keyup of the voice key into the focused
Claude window. Release to drop the transcript in; tap the white button to submit.

Ships as a **menu-bar/tray app** (`reasoning-ting`) with a status icon, input-device
and sensitivity pickers, a focus-guard toggle, and a one-click keybinding writer
— plus a headless CLI (`reasoning-ting-listen`) for power users. Cross-platform (Linux
+ macOS) off one Rust core; reproducible builds via a Nix flake.

## The device
- **EP-2350 "TING"** — RP2350-based handheld mic/sampler/effects unit.
- Firmware is **MicroPython 1.25** (custom TE build), app version `EP-2350 1.0.3`
  (UF2 file is rev `1.0.8`).
- Custom native modules: `ui` (handle/buttons/LEDs/accel/ADC), `spl` (sample
  load/trigger), `fx` (effects chain incl. a `RING` oscillator).
- `main.py` is **frozen** into the firmware; it reads `/fat/config.json` + WAVs.
- Audio goes out the **3.5mm analog jack** into the PC's mic input
  (USB-C is control/data only: a CDC-ACM REPL + mass storage "TINGDISK").
- Connects over USB as `2367:0620`. BOOTSEL ("TING BOOT") shows as
  `Board-ID: RP2350`, drag-drop UF2 target.

## Plan (chosen approach)
Emit **Quindar-style tones** from firmware on handle press/release:
- press (`ui.handle()` rises past a threshold) → 2525 Hz burst ("start")
- release (falls below) → 2475 Hz burst ("stop")
- white button → 3000 Hz burst ("submit" → Enter)
Distinct narrowband tones = unambiguous press, release, AND submit, immune to
long speech pauses. PC daemon Goertzel-detects them and synthesizes a keypress
(F12, bound to `/voice`; Enter to submit) into the focused Claude window.

## Key findings
- `ui.handle()` → 0.0–1.0 float (handle position). The handle is **analog**
  (ADC ch1, rest ~2034/4095); the "two switches" read as ADC, not button events.
- A constant **type-3 tick** event streams from firmware (the apparent "flood").
- `spl.trigger(-1, slot, True/False)` plays a loaded sample; `spl.load_wav(slot,
  fh, playmode)` loads one. Playmodes: oneshot / hold / startstop.
- **Power-save clobbers loaded samples.** After 5 min idle (battery only — USB
  power never sleeps) the unit enters power save, which reinitializes the `spl`
  engine back to the ROM stock pack (slot 2=gunshot, 3=monkey-boy, 1=alarm). Our
  Python state survives the wake but **no event fires**, so a boot-time
  `load_tones()` is silently lost and squeeze/release/submit play the stock ROM
  samples until a power-cycle. Fix: `main.py` re-asserts the WAV into its slot
  immediately before *every* trigger (`play()`), which is the only wake-proof
  option since there's nothing to hook. Symptom that pinned it down: tones work
  right after power-on, then revert to gunshot/tarzan once it's sat idle.

## Repo layout
- `deploy/` — **TING-side**: `main.py` (runs on the device) + `quindar_gen.py`
  (generates the tone WAVs). These files go on TINGDISK.
- `listener/` — **the product**: Rust core + the `reasoning-ting` tray GUI and
  `reasoning-ting-listen` CLI (same `reasoning_ting::run` engine). See `listener/README.md`.
- `flake.nix` — Nix dev shell (`nix develop`) + per-platform builds
  (`nix build .#gui` / `.#listener`); the Linux binaries bundle the PipeWire
  ALSA route so `nix run .#gui` just works.
- `packaging/` — `linux/` per-user install (icon, `.desktop`, `install.sh`),
  `macos/` `.app` bundle/sign/notarize, plus systemd-user + launchd units.
- `docs/` — `PACKAGING.md` (per-platform distribution strategy + rationale).

## Approach taken (resolved)
**No patching/flashing.** The firmware runs **`fat/main.py` from TINGDISK if
present** (else the frozen stock `teenage.py`), so we drop a `main.py` on the
drive that does `import teenage` (stock app) + a handle→Quindar layer. Proven
end-to-end (clean 2525/2475 tones, ~290x detection margin).

## Running it (live)
1. **Device:** `main.py` + `quindar_in.wav` + `quindar_out.wav` on TINGDISK
   (deploy from `deploy/`). Use the **clean** preset + moderate volume.
   **⚠️ Then EJECT TINGDISK on the host and restart the TING.** USB mass storage
   is exclusive: while the host has TINGDISK mounted, the device can't run
   `fat/main.py` and boots the stock frozen app (stock sounds, no tones). The
   modded firmware only runs when the *device* owns its filesystem. The daemon
   uses the analog audio, not the disk, so leave TINGDISK ejected.
2. **Keybinding:** `~/.claude/keybindings.json` binds `f12` -> `voice:pushToTalk`
   (Chat). **Restart Claude Code** so it loads the binding.
3. **Audio:** the TING (front-mic input) must be the system **default input**
   (it is) so both Claude's dictation and the daemon hear it.
4. **App** — pick one:
   - **Tray GUI** (recommended): `reasoning-ting` (or `nix run .#gui`). Status icon
     (green = standing by, red = voice key held / recording), menu for input device /
     sensitivity / focus-guard / "Write Claude keybinding". Run from a terminal
     to watch the event log (`reasoning-ting │ squeeze   voice key down (f12)` …);
     device/sensitivity/focus-guard choices persist across restarts. On
     i3/sway/dwm run `snixembed` so the icon shows.
   - **Headless CLI:** `reasoning-ting-listen` (keeps running in a terminal).
     `--dry-run` watches detections without keystrokes; `--no-focus-guard`
     injects regardless of focus. The focus guard otherwise only injects when a
     window with `claude` in its process tree is focused.
5. **Use:** focus Claude, **squeeze** the handle (2525 Hz → F12 down → dictation
   records), talk, **release** (2475 Hz → F12 up → transcript drops into the
   input). Repeat to dictate multiple chunks; press the **white button**
   (3000 Hz → Enter) to submit when ready. Needs a Claude.ai account (voice
   isn't available on API keys). Max-hold safety defaults to 600 s.

## App (listener/)
Rust, cross-platform, off one core engine (`reasoning_ting::run`): cpal audio →
Goertzel-style tone detector → enigo key injection, with a native x11rb focus
guard on Linux. Two front-ends — the `reasoning-ting` tray GUI (`--features gui`) and
the `reasoning-ting-listen` CLI — share it. Detector + icon unit-tested (`cargo test`)
and validated against a real capture.

Build (host toolchain): `cargo build --release --features gui` (Linux needs
`libgtk-3-dev` + `libayatana-appindicator3-dev` + `libxdo-dev`), then
`packaging/linux/install.sh` for a per-user install. Or build reproducibly with
Nix: `nix build .#gui`. On Linux "ALSA" is just the client API in front of
**PipeWire** — both build paths route through it (see `docs/PACKAGING.md`).
Validate the Claude keybinding without the TING: `reasoning-ting-listen --test-key`.
The original Python prototype proved the pipeline and has been retired.
