# Windows-binary analysis tooling, Linux-native

*Reference for the case where an interoperability question survives all three observation routes and a `DECOMPILATION-LOG.md` entry has been made. Everything here runs on CachyOS; no Windows desktop required. See `re-tooling.md` for the wider survey and pricing.*

## What the vendor binaries are likely to be

| Artefact | Format | Notes |
|---|---|---|
| NT4 / 2000 kernel driver (`*.sys`) | PE32, subsystem NATIVE | Imports from `ntoskrnl.exe` and `hal.dll`. Port I/O appears as `READ_PORT_UCHAR` / `WRITE_PORT_UCHAR` / `_ULONG` imports, or inline `in`/`out` instructions. Look for `IoCreateDevice`, `IoConnectInterrupt`, `KeSetTimer`, or `KeStallExecutionProcessor` to find the servo path. |
| Win95/98/ME driver (`*.vxd`) | **LE** (Linear Executable) | Not PE. Ghidra needs a loader extension. Calls kernel services via `int 20h` VxDCall thunks (VPICD for IRQ, VTD for timing); port I/O is inline `in`/`out`. |
| User-mode DLLs (GHOST, PDD) | PE32 | Talk to the driver via `DeviceIoControl`; the IOCTL codes and buffer layouts are the user/kernel contract and are useful spec material. |
| 16-bit components, if any | NE | Ghidra loads NE natively. |
| `.inf` and config files | text | Device IDs, port/IRQ defaults, resource ranges. Read, do not copy. |

Identify with `file`, `pev`/`readpe`, or `python -c 'import pefile'`.

## Static analysis

**Ghidra** (Apache-2.0, `pacman -S ghidra`, JDK 21). First choice. Loads PE and NE natively. For VxD/LE install [ghidra-lx-loader](https://github.com/yetmorecode/ghidra-lx-loader). Use `analyzeHeadless` for scripted passes and PyGhidra for Python 3 scripting. Useful searches once loaded: instruction search for `IN`/`OUT`, import references to `READ_PORT_*`/`WRITE_PORT_*`, cross-references to `DeviceIoControl` in the DLLs, and constant search for `0x378`, `0x37B`, `0x37C` (SPP/EPP register offsets) and the IEEE-1284 EPP address/data offsets `+3`/`+4`.

**Binary Ninja** (native Linux; Free tier decompiles 32-bit x86 PE; Personal ~$199). Better HLIL ergonomics than Ghidra for tracing a single function; no LE loader in core, so use it on the PE artefacts only.

**IDA Free** (native Linux build; free, x86/x64 only, cloud decompiler with limits). Third option; fine for PE, historically the strongest LE/VxD loader if a licensed version is available.

**rizin + Cutter** (`pacman -S rizin cutter`) and **radare2**: quick CLI triage, `rz-bin -i` for imports, `rz-find` for constants, `aaa; pdf @ sym.*` for functions. Decompiler via rz-ghidra plugin.

**Command-line triage**: `mingw-w64-binutils` (`i686-w64-mingw32-objdump -d`, `-p` for imports), `llvm-objdump`, `pev` (`readpe`, `pesec`), `strings`, `flare-floss` for obfuscated strings, `binwalk` to spot an embedded firmware blob, `lief` (Python) for programmatic parsing of PE/NE, `Detect It Easy` (Linux build) for packer/compiler identification.

## Dynamic analysis of the Windows stack from Linux

The vendor driver has to run inside a guest. Three Linux-native ways to watch it:

1. **QEMU trace events**: `-trace 'parallel_ioport_*'` logs every guest access to the emulated port with timestamps (see `capture-strategy.md`, Route 2). This is observation, not decompilation.
2. **QEMU gdbstub**: `qemu-system-i386 -s -S ...` then `gdb -ex 'set architecture i386' -ex 'target remote :1234'`. Breakpoints and watchpoints anywhere in the guest, including ring 0 in the NT4 driver or a Win98 VxD, driven from the Linux host with no in-guest debugger. `hbreak` on the port I/O instruction addresses found statically gives register state at each access. Pair with `pwndbg` or `gef` for comfort.
3. **In-guest kernel debugger**, when needed: WinDbg on NT4/2000 over a serial link exposed by QEMU (`-serial tcp::4444,server,nowait`), with a second guest running WinDbg, or `-serial pty` to a Wine-hosted WinDbg. Heavier; prefer the gdbstub.

**Wine** (`pacman -S wine`) runs the user-mode vendor utilities ("PHANTOM Configuration", "PHANTOM Test") for black-box study, and `WINEDEBUG=+relay` logs every Win32 API call they make, including `DeviceIoControl` codes and buffer sizes. Wine cannot load the kernel driver, so this is user/kernel-contract discovery only. Note that Wine's `ppdev` support for direct port access is minimal; do not expect the vendor utility to drive the robot under Wine.

**Bochs** (`pacman -S bochs`) has an instruction-level internal debugger and port I/O logging; slower than QEMU, but deterministic, and useful when a timing-sensitive VxD misbehaves under QEMU.

## Firmware check

Before any decompilation, run `binwalk` and `strings` over the driver and DLLs and look for a large high-entropy section or an obvious code blob. If the driver uploads firmware at start-up, that changes the legal handling (`docs/legal/landscape.md` §11) and the capture from Route 1 or 2 will show it as a long write burst anyway.

## Housekeeping under the clean-room policy

- Keep vendor binaries, Ghidra projects, and decompiler exports in `/vendor-original/` and `/disasm/` (git-ignored). Record SHA-256 of each binary examined in `DECOMPILATION-LOG.md`.
- Every decompilation session answers one named question. Write the answer into `docs/spec/` in our own words with a DECOMPILE provenance row; do not paste pseudocode.
- Facts from DECOMPILE rows do not go in the paper.

## Install line

```sh
sudo pacman -S --needed ghidra jdk21-openjdk rizin cutter radare2 mingw-w64-binutils \
    python-pefile python-lief binwalk wine bochs qemu-system-x86 gdb pwndbg
# ghidra-lx-loader: build from source per its README and drop the .zip into Ghidra's Extensions
# Binary Ninja Free / IDA Free: download the Linux installers from the vendors
```
