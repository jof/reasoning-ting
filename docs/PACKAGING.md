# Packaging reasoning-ting

How reasoning-ting is built and distributed per platform, and the reasoning behind
each choice. The goal: someone who is *not* a command-line user can install it
and get a working menu-bar/tray push-to-talk button.

## What the app needs from the OS

reasoning-ting is a desktop automation tool, and that shapes everything below. At
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

Audio works in *both* a host-toolchain build and a hermetic Nix build. A Nix
binary links Nix's own `libasound`, whose plugin/config search paths point into
the store — but nixpkgs ships its own `pipewire` ALSA plugin
(`${pipewire}/lib/alsa-lib/libasound_module_pcm_pipewire.so`) + config, ABI-
matched to that libasound, and the flake's wrapper points the binary at them
(plus `alsa-plugins` for the `default` fallback chain). The plugin connects to
the running PipeWire daemon over its socket, so `nix run .#gui` does PipeWire
audio on any distro (verified on Ubuntu), not just NixOS. cpal stays the audio
layer everywhere; on Linux "ALSA" is just the client API in front of PipeWire.
The per-user `install.sh` path below is simply the *lighter* dev option (a plain
`cargo build` links the host libasound directly).

What we ship:
- `listener/target/release/reasoning-ting` (tray GUI) + `reasoning-ting-listen` (headless
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

`packaging/macos/` bundles the GUI into `reasoning-ting.app` (LSUIElement menu-bar
app), code-signs + notarizes it, and produces a DMG. CoreAudio + CGEvent mean
none of the Linux ALSA/sandbox issues apply. The focus guard matches the
frontmost **app** (a terminal can't cheaply expose its child PID), via the
always-present `osascript`.

```sh
SIGN_ID="Developer ID Application: …" NOTARY_PROFILE=… packaging/macos/bundle.sh
```
Needs a Mac to build/sign/test — not exercised in the Linux dev environment.

## The role of Nix

Nix is the **reproducible dev + build environment, CI, and a valid runtime**:
- `nix develop` — exact toolchain (cargo, GTK3, ALSA+pipewire, libxdo,
  snixembed) for contributors on any machine; the shell exports the ALSA→
  PipeWire env so an in-shell `cargo run` has working audio.
- `nix build .#gui` / `.#listener` — reproducible binaries whose wrapper bundles
  the pipewire ALSA plugin + config, so `nix run` plays/captures through the
  host PipeWire on any distro. Good for CI artifacts and the macOS build too.
- The per-user `install.sh` (host-toolchain build) is the lighter alternative
  when you don't want a Nix closure; both are first-class.

## Summary

| Platform | Build | Runtime artifact | Sandbox |
|---|---|---|---|
| Linux | host cargo **or** `nix build` | `install.sh` → `~/.local`, or `nix run .#gui` (+ optional AppImage) | none (required — see Flatpak note) |
| macOS | Nix or native | signed `.app` + DMG | macOS entitlements |
| Dev/CI | `nix develop` / `nix build` | — | — |

The remaining "none (required)" sandbox column is about **Flatpak/Snap**, which
are unsuitable for the focus-guard/`/proc` + XTEST/`~/.claude`/USB reasons in
the Linux section — *not* about audio. Audio is solved in every build above.
