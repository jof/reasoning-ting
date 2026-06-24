#!/usr/bin/env python3
"""Safe MicroPython REPL bridge for the TING over libusb (pyusb).
Talks ONLY to CDC-data bulk endpoints (0x02 OUT / 0x82 IN). Never opens
/dev/ttyACM, never sends SET_CONTROL_LINE_STATE. Bulk reads use short timeouts.

Usage:
  tingrepl.py exec '<python code>'      # raw-REPL exec, prints stdout/stderr
  tingrepl.py execfile <path>           # exec a local .py file on the device
"""
import sys, time, usb.core, usb.util

VID, PID = 0x2367, 0x0620
EP_OUT, EP_IN, DATA_IF = 0x02, 0x82, 1

def connect():
    d = usb.core.find(idVendor=VID, idProduct=PID)
    if d is None:
        print("TING not found", file=sys.stderr); sys.exit(2)
    for i in (0, 1):
        try:
            if d.is_kernel_driver_active(i):
                d.detach_kernel_driver(i)
        except Exception:
            pass
    usb.util.claim_interface(d, DATA_IF)
    return d

def _w(d, b): d.write(EP_OUT, b, timeout=1000)

def _read_until(d, pred, deadline):
    buf = bytearray()
    while time.time() < deadline:
        try:
            buf += bytes(d.read(EP_IN, 64, timeout=120))
        except usb.core.USBError:
            pass
        if pred(buf):
            return buf
    return buf

def raw_exec(d, code, timeout=8.0):
    end = time.time() + timeout
    _w(d, b"\r\x03\x03")                       # interrupt anything running
    time.sleep(0.05)
    try:                                       # flush
        while True: d.read(EP_IN, 64, timeout=80)
    except usb.core.USBError: pass
    _w(d, b"\r\x01")                            # enter raw REPL
    _read_until(d, lambda b: b"to exit\r\n>" in b, end)
    _w(d, code.encode() + b"\x04")             # send code + Ctrl-D
    buf = _read_until(d, lambda b: b.count(b"\x04") >= 2, end)
    _w(d, b"\r\x02")                            # back to friendly REPL
    body = buf.split(b"OK", 1)[-1]
    parts = body.split(b"\x04")
    out = parts[0].decode("utf-8", "replace") if parts else ""
    err = parts[1].decode("utf-8", "replace") if len(parts) > 1 else ""
    return out, err

def soft_reboot(d, wait=3.0):
    _w(d, b"\r\x03\x03"); time.sleep(0.2)
    try:
        while True: d.read(EP_IN, 64, timeout=100)
    except usb.core.USBError: pass
    _w(d, b"\x04")                              # Ctrl-D -> reruns main.py
    buf = _read_until(d, lambda b: b"MICROPYTHON" in b or b">>>" in b, time.time() + wait)
    return buf.decode("utf-8", "replace")

def main():
    if len(sys.argv) < 2:
        print(__doc__); sys.exit(1)
    cmd = sys.argv[1]
    d = connect()
    try:
        if cmd == "reset":
            sys.stdout.write(soft_reboot(d))
            return
        code = open(sys.argv[2]).read() if cmd == "execfile" else sys.argv[2]
        out, err = raw_exec(d, code, timeout=float(__import__("os").environ.get("TIMEOUT", "6")))
        sys.stdout.write(out)
        if err.strip():
            sys.stderr.write("\n[stderr]\n" + err)
        if __import__("os").environ.get("REBOOT") == "1":
            soft_reboot(d)                      # leave device clean (no stale callback)
    finally:
        usb.util.release_interface(d, DATA_IF)
        usb.util.dispose_resources(d)

if __name__ == "__main__":
    main()
