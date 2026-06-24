#!/usr/bin/env python3
"""some-ting PTT daemon.

Listens to the TING's front-mic input for the Quindar tones emitted by our
firmware main.py (2525 Hz on squeeze = talk START, 2475 Hz on release = STOP)
and taps Claude's voice-dictation key into the focused Claude window.

Pipeline: parec (raw s16le) -> sliding 85 ms window -> windowed-DFT magnitude at
2525/2475 -> edge detection (threshold + refractory) -> state machine -> xdotool.

Run with --dry-run first to watch detections without sending keystrokes.
"""
import argparse, subprocess, sys, time, shutil
import numpy as np

SR = 48000
WIN = 4096                  # ~85 ms -> resolves the 50 Hz gap (proven 290x)
HOP = 1024                  # ~21 ms
F_IN, F_OUT = 2525.0, 2475.0
REFRACTORY = 0.30           # s, ignore re-triggers within this
MIN_RATIO = 4.0             # dominant tone must beat the other by this

def default_source():
    try:
        return subprocess.check_output(["pactl", "get-default-source"], text=True).strip()
    except Exception:
        return None

def focused_pid():
    """X11 id + pid of the focused window (xdotool), or (None, None)."""
    try:
        wid = subprocess.check_output(["xdotool", "getactivewindow"], text=True).strip()
        pid = subprocess.check_output(["xdotool", "getwindowpid", wid], text=True).strip()
        return wid, int(pid)
    except Exception:
        return None, None

def proc_tree_has(pid, name):
    """True if `name` is in pid's command or any descendant (cheap /proc walk)."""
    try:
        # collect children map
        seen = set()
        stack = [pid]
        while stack:
            p = stack.pop()
            if p in seen:
                continue
            seen.add(p)
            try:
                with open(f"/proc/{p}/cmdline", "rb") as fh:
                    if name.encode() in fh.read():
                        return True
            except Exception:
                pass
            # children
            try:
                ch = subprocess.check_output(["pgrep", "-P", str(p)], text=True).split()
                stack.extend(int(c) for c in ch)
            except Exception:
                pass
        return False
    except Exception:
        return False

def claude_focused(guard):
    if not guard:
        return True, None
    wid, pid = focused_pid()
    if pid is None:
        return False, None
    return proc_tree_has(pid, "claude"), wid

def key_down(key, dry):
    if not dry:
        subprocess.run(["xdotool", "keydown", key], check=False)  # XTEST -> focused window

def key_up(key, dry):
    if not dry:
        subprocess.run(["xdotool", "keyup", key], check=False)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", default=default_source())
    ap.add_argument("--key", default="F13", help="key bound to /voice tap mode")
    ap.add_argument("--threshold", type=float, default=0.008)
    ap.add_argument("--max-hold", type=float, default=30.0,
                    help="safety: force key-up after this many seconds held")
    ap.add_argument("--no-focus-guard", action="store_true",
                    help="inject regardless of which window is focused")
    ap.add_argument("--dry-run", action="store_true", help="detect only, no keystrokes")
    args = ap.parse_args()
    if not shutil.which("xdotool") and not args.dry_run:
        print("xdotool not found (needed to send keystrokes). Install it or use --dry-run.",
              file=sys.stderr); sys.exit(1)
    if not args.source:
        print("no audio source; pass --source", file=sys.stderr); sys.exit(1)

    t = np.arange(WIN)
    ref_in = np.exp(-2j*np.pi*F_IN*t/SR) * np.hanning(WIN)
    ref_out = np.exp(-2j*np.pi*F_OUT*t/SR) * np.hanning(WIN)

    parec = subprocess.Popen(
        ["parec", "--format=s16le", f"--rate={SR}", "--channels=1",
         "--latency-msec=20", "-d", args.source],
        stdout=subprocess.PIPE)
    print(f"listening on {args.source}  key={args.key}  "
          f"focus_guard={not args.no_focus_guard}  dry_run={args.dry_run}", flush=True)

    buf = np.zeros(WIN, dtype=np.float32)
    held = False                 # is the voice key currently held down
    held_since = 0.0
    last_evt = 0.0
    bytes_per_hop = HOP * 2
    try:
        while True:
            raw = parec.stdout.read(bytes_per_hop)
            if not raw or len(raw) < bytes_per_hop:
                break
            block = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
            buf = np.roll(buf, -HOP); buf[-HOP:] = block
            m_in = abs(np.dot(buf, ref_in)) / WIN
            m_out = abs(np.dot(buf, ref_out)) / WIN
            hi, lo = (m_in, m_out) if m_in >= m_out else (m_out, m_in)
            now = time.time()
            # safety: never leave the key stuck down
            if held and now - held_since > args.max_hold:
                key_up(args.key, args.dry_run); held = False
                print(f"(safety key-up after {args.max_hold:.0f}s)", flush=True)
            if hi > args.threshold and hi > MIN_RATIO * (lo + 1e-9) and now - last_evt > REFRACTORY:
                last_evt = now
                is_intro = m_in >= m_out
                tag = "INTRO/2525" if is_intro else "OUTRO/2475"
                if is_intro and not held:
                    ok, _ = claude_focused(not args.no_focus_guard)
                    if not ok:
                        print(f"{tag}  (ignored: no Claude window focused)", flush=True)
                        continue
                    key_down(args.key, args.dry_run); held = True; held_since = now
                    print(f"{tag} -> START (keydown {args.key})", flush=True)
                elif (not is_intro) and held:
                    key_up(args.key, args.dry_run); held = False
                    print(f"{tag} -> SEND  (keyup {args.key})", flush=True)
                else:
                    print(f"{tag}  (no-op; held={held})", flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        if held:
            key_up(args.key, args.dry_run)
        parec.terminate()

if __name__ == "__main__":
    main()
