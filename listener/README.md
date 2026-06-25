# some-ting-listen

Cross-platform daemon that listens for the TING's Quindar tones (2525 Hz on
squeeze, 2475 Hz on release, 3000 Hz on the white button — emitted by our
`deploy/main.py` firmware) and drives Claude Code's voice dictation: hold the
voice key down while the handle is squeezed (push-to-talk), release on let-go,
and tap Enter on the white button to submit. Focus-guarded so it only fires into
a focused Claude window.

Rust, single self-contained binary — chosen over Python for clean cross-platform
distribution (no interpreter/PortAudio bundle; trivial to code-sign + notarize
on macOS; ships as a systemd unit / launchd LaunchAgent).

## Architecture
- `detect.rs` — **portable** Goertzel-style detector (sliding ~85 ms window,
  windowed-DFT magnitude at 2525/2475/3000, threshold + dominance-ratio +
  refractory, press/release state machine + one-shot submit). Direct port of the
  validated Python params; unit-tested and checked against a real capture
  (`--wav`).
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
some-ting-listen --wav capture.wav                   # offline detector check (any recording)
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
- ✅ live cpal capture starts cleanly (Linux: via host libasound → PipeWire)
- ✅ keystroke → Claude voice trigger (F12 `voice:pushToTalk`, confirmed live)
- ✅ end-to-end through PipeWire: squeeze/release/submit → Intro/Outro/Submit
  events with zero spurious/missed (CLI meter, `--no-focus-guard`)
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

## GUI (menu-bar) — `some-ting`
Cross-platform status-tray app on the same core engine (`some_ting::run`). Build:
```
cargo build --release --features gui --bin some-ting   # -> target/release/some-ting
```
Linux needs `libgtk-3-dev` + `libayatana-appindicator3-dev`; macOS/Windows need
no extra system libs.

Icon states (procedural, see `icon.rs`): grey **idle** · green **listening** ·
red **keyed** (voice held) · grey-bars **paused**. The current state is also
mirrored into the tray **tooltip** (some XEmbed bridges redraw a changed pixmap
slowly) and printed to stderr — so launched from a terminal you see every
`[state]`/`[event]` line live, which is the fastest way to tell detection from
injection problems.

Menu: live status · Pause/Resume · **Input device** (system default + each
capture device) · **Sensitivity** (High/Med/Low threshold) · **Focus guard**
(toggle "Claude windows only") · **Write Claude keybinding (f12)** · Quit.

Run / flags: `--dry-run` (detect, never inject), `--no-focus-guard` (inject
regardless of focus — use this to confirm injection works independent of the
guard).

### Running on Linux (important)
Build with the **host toolchain**, not inside `nix develop` — a Nix-shell binary
links Nix's libasound and can't open the host PipeWire `default` device (ENXIO).
The system-linked binary routes through PipeWire correctly. Install per-user:
```
cargo build --release --features gui
../packaging/linux/install.sh            # ~/.local; add --autostart for login
```
On **i3/sway/dwm** the SNI icon needs a tray host — run `snixembed` (bridges to
i3bar). GNOME/KDE render it natively. See `docs/PACKAGING.md` for the full
rationale (and why Flatpak/Snap don't fit this app).

**macOS app bundle:** `packaging/macos/bundle.sh` builds + bundles (`.app`,
`LSUIElement`, mic usage string) and — with `SIGN_ID`/`NOTARY_PROFILE` set —
code-signs (hardened runtime), notarizes, and produces a DMG.

Roadmap: persisted preferences · first-run "Setup…" wizard (request
Accessibility/Mic, write the Claude keybinding, deploy firmware to TINGDISK) ·
launch-at-login toggle in-menu.
