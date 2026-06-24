# Ghidra + GhidraMCP setup

Installed (by Claude):
- **Ghidra 11.3.2** → `~/ghidra/ghidra_11.3.2_PUBLIC/` (launch: `ghidraRun`).
- **GhidraMCP 1.4** extension staged at
  `~/ghidra/ghidra_11.3.2_PUBLIC/Extensions/Ghidra/GhidraMCP-1-4.zip`
  (targets Ghidra 11.3.2 — version matched on purpose).
- Bridge: `tools/bridge_mcp_ghidra.py` (talks to the plugin's HTTP server on
  `127.0.0.1:8080`, default).
- MCP server `ghidra` registered (project-local) →
  `venv/bin/python tools/bridge_mcp_ghidra.py`.
- venv has `mcp` + `requests`.

## Your steps (the GUI bits)
1. **Launch Ghidra:** `~/ghidra/ghidra_11.3.2_PUBLIC/ghidraRun`
2. **Install the extension:** File → Install Extensions → check **GhidraMCP** →
   OK → restart Ghidra when prompted.
3. **New project**, then **Import File**: `firmware/ep-2350_firmware_1_0_8.bin`
   - Format: **Raw Binary**
   - Language: **ARM:LE:32:Cortex** (the `…` picker → filter "Cortex")
   - Options → **Base Address: `0x10000000`**
4. Open in **CodeBrowser**. When asked to analyze, say yes (defaults fine).
   The vector table at base gives SP `0x20082000` / reset `0x1000015c`.
5. **Enable the MCP plugin:** File → Configure → Miscellaneous → check
   **GhidraMCPPlugin** (it starts the HTTP server on :8080). Keep CodeBrowser
   open with the program loaded.

## Then
- The `ghidra` MCP tools may need a **Claude Code restart** to appear in-session.
- Once they're live and Ghidra has the program open, Claude drives the analysis
  starting at RE question #1 (boot flow) in `reverse-engineering.md`.

## Sanity check the plugin server (optional)
`curl -s http://127.0.0.1:8080/methods | head`  → should list functions once the
program is loaded and the plugin is enabled.
