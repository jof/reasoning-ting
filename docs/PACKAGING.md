# Packaging some-ting

How some-ting is built and distributed per platform, and the reasoning behind
each choice. The goal: someone who is *not* a command-line user can install it
and get a working menu-bar/tray push-to-talk button.

## What the app needs from the OS

some-ting is a desktop automation tool, and that shapes everything below. At
runtime it must:

1. **Capture audio** from the sound server (PipeWire/Pulse on Linux, CoreAudio
   on macOS) to hear the TING's Quindar tones.
2. **Inject keystrokes** into whatever app is focused (XTEST on X11, CGEvent on
   macOS) — F12 for Claude's push-to-talk, Enter to submit.
3. **Read the focused window's process tree** (`/proc` on Linux) so the *focus
   guard* only fires into a real Claude Code session.
4. **Write `~/.claude/keybindings.json`** (the voice keybinding) and read the
   **TINGDISK** USB volume (to deploy firmware).
5. **Show a status-tray icon** via StatusNotifierItem (Linux) / NSStatusItem
   (macOS).

Items 2–4 are exactly the things desktop **sandboxes are designed to forbid**.
That is the single most important packaging constraint.

## Linux — unsandboxed (release binary + per-user install)

**Decision: ship a normal dynamically-linked binary, installed per-user; do
NOT use Flatpak or Snap.**

Why not Flatpak/Snap:
- They run the app in a **PID namespace**, so the host PIDs behind
  `_NET_WM_PID` are invisible in the sandbox's `/proc`. The focus guard (which
  walks the focused window's process subtree looking for `claude`) would always
  fail → it would never inject. Window title/class can't substitute: a terminal
  running Claude reports `WM_CLASS=kitty` and a title that's the *conversation
  topic*, not "claude". The `/proc` walk is the only reliable signal.
- XTEST keystroke injection into arbitrary host windows, writing the real
  `~/.claude`, and touching the TINGDISK USB volume are all blocked or
  awkward-to-portal under the sandbox.

Why not a hermetic **Nix-closure** binary: it links Nix's own `libasound`,
whose ALSA plugin search path points into the Nix store and can't load the
host's `pipewire-alsa` bridge — opening the PipeWire `default` device fails with
ENXIO. (See `[[linux-audio-packaging-decision]]` / the README audio notes.) The
binary must link the **host** `libasound` so it routes through the running
PipeWire daemon. cpal stays the audio layer on every platform; on Linux "ALSA"
is just the client API in front of PipeWire.

What we ship:
- `listener/target/release/some-ting` (tray GUI) + `some-ting-listen` (headless
  CLI), built with the host toolchain against system GTK3 + libasound.
- `packaging/linux/install.sh` → installs to `~/.local/bin`, drops a
  `.desktop` launcher + scalable icon under `~/.local/share`, and (with
  `--autostart`) an XDG autostart entry. `uninstall.sh` reverses it. No root.
- Tray visibility: GNOME/KDE render SNI natively. **i3/sway/dwm need an SNI
  host** — we recommend `snixembed` (bridges SNI → i3bar's XEmbed tray).

Build + install:
```sh
cd listener && cargo build --release --features gui
../packaging/linux/install.sh            # add --autostart to launch at login
```

Future option: an **AppImage** (also unsandboxed — bundles GTK so it runs on
older distros, while keeping full host access). The per-user script is the
zero-dependency baseline; AppImage is a nice-to-have, not a sandbox.

## macOS — signed .app bundle (the non-CLI target)

`packaging/macos/` bundles the GUI into `some-ting.app` (LSUIElement menu-bar
app), code-signs + notarizes it, and produces a DMG. CoreAudio + CGEvent mean
none of the Linux ALSA/sandbox issues apply. The focus guard matches the
frontmost **app** (a terminal can't cheaply expose its child PID), via the
always-present `osascript`.

```sh
SIGN_ID="Developer ID Application: …" NOTARY_PROFILE=… packaging/macos/bundle.sh
```
Needs a Mac to build/sign/test — not exercised in the Linux dev environment.

## The role of Nix

Nix is the **reproducible dev + build environment and CI**, not the Linux
runtime format:
- `nix develop` — exact toolchain (cargo, GTK3, ALSA, libxdo, snixembed) for
  contributors on any machine.
- `nix build .#gui` / `.#listener` — builds the binaries reproducibly. On Linux
  these are fine for *building/CI* but are **not** a desktop-audio runtime (the
  hermetic-libasound/PipeWire clash above); ship the host-linked binary instead.
- Best fit for the **macOS** build and as the CI substrate that produces every
  platform's artifacts.

## Summary

| Platform | Build | Runtime artifact | Sandbox |
|---|---|---|---|
| Linux | host cargo (or Nix for CI) | `install.sh` → `~/.local` (+ optional AppImage) | none (required) |
| macOS | Nix or native | signed `.app` + DMG | macOS entitlements |
| Dev/CI | `nix develop` / `nix build` | — | — |
