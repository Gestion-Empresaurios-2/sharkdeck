# Reverse-engineering workspace — SharkDeck

This folder holds the reverse-engineering notes, toolchain scripts and future
artefacts for the Attack Shark K86 (ROYUAN family) native firmware and its
built-in screen OS.

## Toolchain (installed locally, no system changes)

`sharkdeck/re/setup-rea-toolchain.sh` installs everything into
`/tmp/opencode/rea-toolchain`:

| Tool | Where | Why |
|---|---|---|
| Node 22.20 | `rea-toolchain/node22` | REA requires Node ≥ 22.19 (repo box has 20.12) |
| REA (`rea-agents`) | `rea-toolchain/npm/bin/rea` | the MCP/CLI reverse-engineering agent toolkit |
| unblob 26.6 | `~/.local/bin/unblob` | firmware region extraction |
| binwalk 2.1 | `~/.local/bin/binwalk` | firmware signature scanning |
| Ghidra 11.3.2 | `rea-toolchain/ghidra` | ARM Cortex-M native analysis |

REA picks Ghidra up through `GHIDRA_INSTALL_DIR`:

```bash
export PATH=/tmp/opencode/rea-toolchain/npm/bin:/tmp/opencode/rea-toolchain/node22/bin:$HOME/.local/bin:$PATH
export GHIDRA_INSTALL_DIR=/tmp/opencode/rea-toolchain/ghidra
rea doctor
```

Useful REA commands for this target:

```bash
rea inspect-firmware-regions --input firmware.bin   # binwalk/unblob pass
rea extract-firmware      --input firmware.bin      # verified extraction + provenance
rea decompile             ...                      # native functions via Ghidra
```

## Documents

| File | Contents |
|---|---|
| `PROTOCOL-AUDIT.md` | every public opcode, byte-accurate, with sources |
| `SHARKFIN-FLASH-AUDIT.md` | sharkfin's protocol module: flash/backup/restore capability, firmware image hunt |
| `PLAN.md` | the three-phase plan: backup → analyse → own OS |

## Ground rules

* **Read-only first.** Every experiment on live hardware is a GET-family probe
  (`sharkdeck-cli opdump` sweeps `0x80..=0x9F` only). Destructive opcodes
  (`0xAC` flash erase, `0x2C` display erase, `0x7F`/`0x30` boot entries, `0x02`
  factory reset) are never wired into the app or run without the owner's
  explicit consent.
* **The keyboard is the user's own hardware** — everything here stays inside
  the device the app already talks to.
* Every claim in these documents must be traceable to a file:line, a URL or a
  captured byte dump. Anything inferred is marked `INFERRED`.
