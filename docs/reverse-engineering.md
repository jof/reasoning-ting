# Reverse-engineering scope — TING EP-2350 firmware

Goal: understand just enough of TE's closed firmware to inject a **handle →
Quindar tone** behavior, without source. We do NOT need to understand the whole
DSP — only the boot flow, the relevant native-module entry points, and the tone
output path.

## Target binary
- File: `firmware/ep-2350_firmware_1_0_8.bin` (extracted from the UF2).
- Size: 935,936 bytes (0xe4800).
- **Load base: `0x10000000`** (XIP flash). Maps `0x10000000`–`0x100e4800`.
- Arch: **ARM Cortex-M33, little-endian** (RP2350 Arm-S, family `0xe48bff59`).
- Vector table at base: **initial SP `0x20082000`**, **reset handler `0x1000015c`**
  (Thumb). SRAM at `0x20000000` (~520 KB). Peripherals at `0x40000000`+.
- Runtime: **MicroPython 1.25** (open source — major RE accelerator).
- Image is **unsigned**; picobin `IMAGE_DEF` block at `0x10000138`.

## Ghidra load
1. Import `ep-2350_firmware_1_0_8.bin` as **Raw Binary**.
2. Language: **ARM:LE:32:Cortex** (Cortex-M / ARMv8-M).
3. Set image base **`0x10000000`**. Disassemble from reset vector `0x1000015c`.
4. Add memory blocks: SRAM `0x20000000` (len 0x82000, rw), and (optional) an
   RP2350 SVD via SVD-Loader for peripheral labels (ADC/PWM/PIO/DMA/I2S used by
   audio + handle).
5. Let auto-analysis run; the vector table gives many exception handlers.

## RE questions, prioritized (value / effort)
1. **Boot flow — does it run a filesystem `main.py`/`boot.py`?** (settles Path A
   vs B). HIGH value, LOW effort. Anchor: xref the `"main.py"` / `"boot.py"`
   strings and the pyexec calls; look for `pyexec_file_if_exists` vs
   `pyexec_frozen_module`. If a VFS file is tried first → Path A (drop our
   main.py on TINGDISK, no patch).
2. **Frozen-module table layout** (settles Path B patchability). MED/HIGH value.
   Find the `mp_frozen_str_*` (names / sizes / content) structures around the
   embedded `main.py` source so we know if we can edit & resize it.
3. **`spl.trigger` / `spl.load_wav` + the tone output path.** HIGH value. How a
   sample reaches the DAC; whether we can synthesize a tone (or must load a WAV).
   Also inspect the `RING` effect (oscillator, freq 0–20 kHz) as a tone source.
4. **`ui` event model + handle ADC.** LOW effort (mostly known): confirm the
   `ui.callback` message encoding, which ADC channel is the handle, thresholds.
5. **fx DSP internals.** LOW priority — not needed for the mod.

## Method (MicroPython-aware)
- **String anchors** (we have them in `firmware/uf2_strings.txt`): `"main.py"`,
  `"boot.py"`, `"raw REPL; CTRL-B to exit"`, `"HANDLE:"`, `"config.json"`,
  `"playmode"`, `"oneshot"`/`"startstop"`/`"hold"`, the embedded help text,
  `"Uncaught exception in IRQ callback handler"`. Xref each to land in the
  relevant C function.
- **Module method tables:** find the qstr pool, then the `mp_obj_module_t`
  globals dicts for `ui`/`spl`/`fx`. Their entries map Python method qstrs
  (`handle`, `trigger`, `load_wav`, `callback`, `sw`, `acc`) →
  `mp_obj_fun_builtin_*` → the C function address. This is the fast way to reach
  `spl.trigger` etc. without reading everything.
- **Accelerator — reference build:** compile **MicroPython 1.25 for RP2350
  (Arm)** locally and build a Ghidra **FunctionID** DB (or use BSim) to auto-ID
  the stock runtime functions. That collapses the unknown surface to *just TE's
  custom native code* (ui/spl/fx + board glue). Worth doing early.

## ghidra-mcp workflow
With the GhidraMCP bridge connected (LaurieWired/GhidraMCP or similar), the loop is:
1. `list_strings` / search → find an anchor string.
2. `get_xrefs` to the string → candidate function.
3. `decompile_function` → read C-ish output.
4. `rename_function` / `set_comment` as we identify things (build up a map).
5. Walk method tables / xrefs to reach `spl.trigger`, the tone path, the boot
   sequence. Iterate.

## Setup checklist (what's needed before I can drive it)
- [ ] Ghidra installed; project created with the `.bin` loaded per above.
- [ ] GhidraMCP extension installed + its HTTP server running in Ghidra.
- [ ] MCP bridge added to Claude Code (`claude mcp add ...`) so the ghidra tools
      appear in this session.
- [ ] (Optional, high-leverage) reference MicroPython 1.25 RP2350 build for
      FunctionID matching.

Once those tools are live in the session, start at RE question #1 (boot flow).
