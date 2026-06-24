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
(e.g. F13, bound to `/voice`) into the focused terminal.

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

## Tooling
- `host/tingrepl.py` — **safe** libusb (pyusb) bridge to the MicroPython REPL.
  Talks only to CDC bulk endpoints (0x02/0x82); never opens /dev/ttyACM, never
  sends modem-control. `exec '<code>'`, `execfile <path>`, `reset`. Env
  `TIMEOUT=<s>`, `REBOOT=1` (soft-reboot after — AVOID on shared buses).
- `host/portcheck.sh` — which USB controller the TING is on vs. BT/audio.
- `host/99-ting.rules` — udev rule: plugdev access + ModemManager ignore
  (installed to /etc/udev/rules.d/).
- `device-probes/` — MicroPython snippets run via tingrepl.
- `firmware/` — stock UF2, release notes, `uf2_strings.txt` (extracted strings
  incl. the frozen `main.py` source and embedded config.json docs).
- `analysis/` — acoustic click study (rejected approach).

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
2. **Keybinding:** `~/.claude/keybindings.json` binds `f13` -> `voice:pushToTalk`
   (Chat). **Restart Claude Code** so it loads the binding.
3. **Audio:** the TING (front-mic input) must be the system **default input**
   (it is) so both Claude's dictation and the daemon hear it.
4. **Daemon** (separate terminal, keeps running):
   `./venv/bin/python host/ptt_daemon.py`
   - `--dry-run` to watch detections without keystrokes
   - `--no-focus-guard` to inject regardless of focus
   - focus guard only injects when a window with `claude` in its process tree is focused
5. **Use:** focus the Claude Code window, **squeeze** the TING handle (intro tone
   -> F13 down -> dictation records), talk, **release** (outro tone -> F13 up ->
   sends). Needs a Claude.ai account (voice dictation isn't available on API keys).

Prereqs installed: `xdotool`, venv has numpy/scipy/pyusb/mcp; `parec` for capture.
