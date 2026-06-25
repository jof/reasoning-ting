# some-ting

Hacking the **Teenage Engineering TING (EP-2350)** into a hardware push-to-talk
trigger for Claude Code's voice dictation. Squeeze the handle → a tone is emitted
→ a PC daemon detects it → keydown/keyup of the voice key into the focused
Claude window.

## The device
- **EP-2350 "TING"** — RP2350-based handheld mic/sampler/effects unit.
- Firmware is **MicroPython 1.25** (custom TE build), app version `EP-2350 1.0.3`
  (UF2 file is rev `1.0.8`).
- Custom native modules: `ui` (handle/buttons/LEDs/accel/ADC), `spl` (sample
  load/trigger), `fx` (effects chain incl. a `RING` oscillator).
- `main.py` is **frozen** into the firmware; it reads `/fat/config.json` + WAVs.
- Audio goes out the **3.5mm analog jack** into the PC's front mic input
  (USB-C is control/data only: a CDC-ACM REPL + mass storage "TINGDISK").
- Connects over USB as `2367:0620`. BOOTSEL ("TING BOOT") shows as
  `Board-ID: RP2350`, drag-drop UF2 target.

## Plan (chosen approach)
Emit **Quindar-style tones** from firmware on handle press/release:
- press (`ui.handle()` rises past a threshold) → 2525 Hz burst ("start")
- release (falls below) → 2475 Hz burst ("stop")
Two distinct narrowband tones = unambiguous press AND release, immune to long
speech pauses. PC daemon Goertzel-detects them and synthesizes a keypress
(e.g. F12, bound to `/voice`) into the focused terminal.

Detection of the click acoustically was rejected: the rocker is acoustically
asymmetric (one edge ~-8 dB, the other near noise floor — see `analysis/`).

## Key findings
- Firmware is **unsigned** → RP2350 secure boot is off → modified firmware runs.
- BOOTSEL is mask-ROM → **unbrickable**; recover by re-dropping the stock UF2.
- `ui.handle()` → 0.0–1.0 float (handle position). The handle is **analog**
  (ADC ch1, rest ~2034/4095); the "two switches" read as ADC, not button events.
- A constant **type-3 tick** event streams from firmware (the apparent "flood").
- `spl.trigger(-1, slot, True/False)` plays a loaded sample; `spl.load_wav(slot,
  fh, playmode)` loads one. Playmodes: oneshot / hold / startstop.

## ⚠️ Hardware hazard (important)
The TING destabilizes whatever USB **controller** it's plugged into — opening
its CDC port (DTR/RTS the firmware never ACKs) **or** a MicroPython soft-reboot
can reset the whole controller, dropping every device on it. We crashed:
- Bus 005 controller `0000:59:00.0` (→ killed Bluetooth + USB audio)
- Bus 003 controller `0000:57:00.0` (→ killed keyboard + mouse)

**Recovery** (rebind the affected controller; needs sudo):
```
echo -n <PCI_ADDR> | sudo tee /sys/bus/pci/drivers/xhci_hcd/unbind
echo -n <PCI_ADDR> | sudo tee /sys/bus/pci/drivers/xhci_hcd/bind
```
All ports tried so far land on Bus 005 or Bus 003 (both shared with critical
devices). A truly isolated controller, or offline flashing, avoids the risk.

## Repo layout
- `deploy/` — **TING-side**: `main.py` (runs on the device) + `quindar_gen.py`
  (generates the tone WAVs). These files go on TINGDISK.
- `listener/` — **the product**: Rust daemon that detects the tones and drives
  Claude voice. See `listener/README.md`.
- `packaging/` — systemd user service (Linux) + launchd LaunchAgent (macOS).
- `firmware/` — stock TE UF2, release notes, `uf2_strings.txt` (RE reference /
  recovery image).
- `docs/` — how it works + the reverse-engineering writeup.
- `dev/` — dev/RE/tuning utilities (REPL bridge `tingrepl.py`, USB `portcheck.sh`,
  `99-ting.rules`, capture analyzer `characterize.py`, `uf2_to_bin.py`).

## Approach taken (resolved)
A from-scratch UF2 is infeasible (TE's `ui`/`spl`/`fx` are closed). But Ghidra RE
of the boot path showed the firmware runs **`fat/main.py` from TINGDISK if present**
(else the frozen `teenage.py`) — see `docs/reverse-engineering.md`. So **no
patching/flashing**: we drop a `main.py` on the drive that does `import teenage`
(stock app) + a handle→Quindar layer. Proven end-to-end (clean 2525/2475 tones,
~290x detection margin).

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
4. **Daemon** (separate terminal, keeps running):
   `listener/target/release/some-ting-listen`
   - `--dry-run` to watch detections without keystrokes
   - `--no-focus-guard` to inject regardless of focus
   - focus guard only injects when a window with `claude` in its process tree is focused
5. **Use:** focus Claude, **squeeze** the handle (2525 Hz → F12 down → dictation
   records), talk, **release** (2475 Hz → F12 up → transcript drops into the
   input). Repeat to dictate multiple chunks; press the **white button**
   (3000 Hz → Enter) to submit when ready. Needs a Claude.ai account (voice
   isn't available on API keys). Max-hold safety defaults to 600 s.

Prereqs installed: `xdotool`, venv has numpy/scipy/pyusb/mcp; `parec` for capture.

## Daemon (listener/)
Rust, cross-platform, single self-contained binary (cpal audio, enigo injection,
native x11rb focus on Linux). Detector unit-tested + validated against a real
capture. Build: `cargo build --release` (Linux needs `libxdo-dev`). Validate the
Claude keybinding without the TING: `some-ting-listen --test-key`. The original
Python prototype proved the pipeline and has been retired in favor of this.
