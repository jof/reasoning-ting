#!/usr/bin/env python3
"""Extract a raw flash image from an RP2350 UF2 (concatenate payloads by addr).
Usage: uf2_to_bin.py in.uf2 out.bin   ->   prints load base + vector table."""
import sys, struct

raw = open(sys.argv[1], "rb").read()
blocks = {}
for i in range(len(raw) // 512):
    b = raw[i * 512:(i + 1) * 512]
    m0, m1, flags, addr, psize, blkno, numblk, famid = struct.unpack("<8I", b[:32])
    assert m0 == 0x0A324655 and m1 == 0x9E5D5157, "bad UF2 magic"
    blocks[addr] = b[32:32 + psize]

addrs = sorted(blocks); base = addrs[0]
img = bytearray()
for a in addrs:
    while base + len(img) < a:
        img.append(0xFF)
    img += blocks[a]

open(sys.argv[2], "wb").write(img)
sp, reset = struct.unpack("<2I", img[:8])
print(f"wrote {sys.argv[2]}  size=0x{len(img):x}  base=0x{base:08x}  "
      f"family=0x{famid:08x}")
print(f"vector table: initial SP=0x{sp:08x}  reset=0x{reset:08x}")
