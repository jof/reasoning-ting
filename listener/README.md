# some-ting-listen

Cross-platform daemon that listens for the TING's Quindar tones (2525 Hz on
squeeze, 2475 Hz on release — emitted by our `deploy/main.py` firmware) and
drives Claude Code's voice dictation: hold the voice key down while the handle
is squeezed (push-to-talk), release on let-go. Focus-guarded so it only fires
into a focused Claude window.

Rust, single self-contained binary — chosen over Python for clean cross-platform
distribution (no interpreter/PortAudio bundle; trivial to code-sign + notarize
on macOS; ships as a systemd unit / launchd LaunchAgent).

## Architecture
- `detect.rs` — **portable** Goertzel-style detector (sliding ~85 ms window,
  windowed-DFT magnitude at 2525/2475, threshold + dominance-ratio + refractory,
  press/release state machine). Direct port of the validated Python params;
  unit-tested and checked against a real capture (`--wav`).
- `audio.rs` — capture via **cpal** (CoreAudio / WASAPI / ALSA-PipeWire); mono
  f32 samples over a channel. Sample-rate-agnostic.
- `inject.rs` — key synthesis via **enigo** (X11 / macOS CGEvent / Windows
  SendInput); push-to-talk `down()`/`up()`.
- `focus.rs` — `FocusGuard`: Linux native via **x11rb** (`_NET_ACTIVE_WINDOW` →
  `_NET_WM_PID` → process-subtree contains `claude`); macOS via frontmost-app
  allowlist (`osascript`); `--no-focus-guard` to disable.

## Build
```
cargo build --release      # -> target/release/some-ting-listen
```
Linux build/runtime needs **libxdo** (enigo's X11 backend): `apt install libxdo-dev`.
macOS/Windows need no extra system libs.

## Run
```
some-ting-listen --list-devices
some-ting-listen --wav ../analysis/ptt.wav          # offline detector check
some-ting-listen --dry-run --no-focus-guard          # live detect, no keystrokes
some-ting-listen                                     # live: key=f12, focus-guarded
```
Flags: `--device <substr>`, `--key <f12|space|…>` (must match
`~/.claude/keybindings.json`), `--threshold`, `--max-hold <s>` (safety release),
`--focus-proc <name>` (Linux subtree match, default `claude`), `--dry-run`,
`--no-focus-guard`.

## Validation status
- ✅ detector unit tests (`cargo test`)
- ✅ detector vs real capture (`--wav ../analysis/ptt.wav` → 6 alternating events,
  matches the Python analysis)
- ✅ live cpal capture starts cleanly
- ⏳ keystroke → Claude voice trigger (validate with the user; the F12 keysym
  must reach Claude and `voice:pushToTalk` must be bound + voice enabled)
- ⏳ macOS path (focus allowlist, Accessibility permission, audio-in hardware)

## macOS notes (for the deployment target)
- **Accessibility permission** required for key injection (System Settings →
  Privacy & Security → Accessibility); **Microphone** permission for capture.
- **Audio input is a hardware gap** — Macs lack analog line-in; the TING's 3.5 mm
  output needs a USB-C audio interface/adapter.
- Focus guard relaxes to "frontmost app ∈ allowlist" (terminal / Claude app).
- Distribute signed + notarized; install as a `launchd` LaunchAgent.

## Platform status
| | audio (cpal) | inject (enigo) | focus |
|---|---|---|---|
| Linux/X11 | ✅ | ✅ (libxdo) | ✅ x11rb native |
| macOS | ✅ | ✅ CGEvent | ✅ osascript allowlist |
| Windows | ✅ | ✅ SendInput | ⏳ stub (allows) |
| Linux/Wayland | ✅ | ⚠️ enigo libei/uinput TBD | ⏳ |
