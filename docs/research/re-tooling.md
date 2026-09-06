# Reverse-Engineering Tooling for a Win9x/NT Parallel-Port Haptic Driver Stack (Linux/CachyOS analyst)

*Compiled 2026-09-06. Prices/versions are from vendor sites and search results as of this date; anything not directly confirmed is marked "unverified."*

## 1. Binary Ninja

**Product.** Binary Ninja (Vector 35) is an interactive disassembler/decompiler/debugger with a plugin/scripting platform, decompiling to a layered IL stack — LLIL (low-level, per-instruction), MLIL (medium-level, typed/stack-abstracted), and **HLIL** (high-level, C-like pseudocode) ([Binary Ninja](https://binary.ninja), [Wikipedia](https://en.wikipedia.org/wiki/Binary_Ninja)).

**Architectures.** Officially: x86 (32/64-bit), ARMv7/Thumb2, ARMv8, PowerPC, MIPS, RISC-V, 6502; nanoMIPS, TriCore and Hexagon decompilers are gated to the **Ultimate** tier. Community plugins extend coverage further ([Wikipedia](https://en.wikipedia.org/wiki/Binary_Ninja)).

**Linux support.** Native Linux builds exist and were confirmed present across all tiers in the FAQ's edition table ([binary.ninja/faq](https://binary.ninja/faq/)).

**Pricing (as of the 6.0 pricing change, effective ~Aug 19 2026 — [binary.ninja pricing-changes post](https://binary.ninja/2026/07/28/pricing-changes.html)):**

| Tier | Price (new, 6.0) | Notes |
|---|---|---|
| Free | $0 | Cloud-based free tier; basic RE capability |
| Non-Commercial (Personal), named | $199 (down from $299) | Student: 75% off → ~$49 |
| Commercial, named | $1,799 (up from $1,499) | |
| Commercial, per-computer | $2,699 | |
| Headless license | $1,259 (was $936) | No longer bundled separately from Ultimate |
| Ultimate, named | $3,499 (was $2,999) | Includes Firmware Ninja + extra architectures |
| Ultimate, per-computer | $5,249 | |
| Ultimate, floating | $5,499 | |
| Collaboration add-on | $1,299/seat/yr | Unbundled from Enterprise in 6.0 |

All are **perpetual** licenses (one year of updates included; software keeps working after expiry) ([binary.ninja/faq](https://binary.ninja/faq/)). A 35% anniversary coupon (`10YRS35OFF`) expired Aug 1 2026, so treat any lower numbers you find elsewhere as stale.

**Free edition and 32-bit x86.** The FAQ confirms x86 is supported across tiers generally, and Wikipedia states x86 32/64-bit is core architecture support, not Ultimate-gated — so **Binary Ninja Free should decompile 32-bit x86 PE/ELF** (unverified in fine print for Free specifically, but nothing found gates 32-bit x86 to paid tiers; only nanoMIPS/TriCore/Hexagon and the newest architectures are Ultimate-only).

**Headless/API by tier.** The FAQ's edition table shows headless/API access as "Limited" on Free and Non-Commercial (Personal), and fully available on Commercial/Ultimate/Enterprise. A dedicated **Headless license** ($1,259) exists for pure-automation use without a GUI seat ([binary.ninja/faq](https://binary.ninja/faq/), [pricing-changes post](https://binary.ninja/2026/07/28/pricing-changes.html)).

**Plugin ecosystem.** C++, Python, and Rust APIs; the core API and sample plugins are MIT-licensed on GitHub, with a community plugin manager/repository ([binary.ninja/faq](https://binary.ninja/faq/)). "Sidekick" is an AI-assist feature layered on top.

**Verdict for this project:** Binary Ninja Free is a reasonable secondary tool for 32-bit x86 NT `.sys` drivers on a hobby budget, but its collaborative/headless limits and Ultimate-gated architecture set make it less essential than Ghidra unless you specifically want its HLIL and UI ergonomics.

---

## 2. Ghidra

**Source/license.** [github.com/NationalSecurityAgency/ghidra](https://github.com/NationalSecurityAgency/ghidra) — Apache License 2.0, developed by NSA's Research Directorate, cross-platform (Windows/macOS/Linux) ([GitHub repo](https://github.com/NationalSecurityAgency/ghidra)).

**Linux packaging.** Ghidra has graduated from AUR into Arch's **official `extra` repository**: `ghidra 12.1.2-2 (x86_64)` — install with `pacman -S ghidra` ([Arch package page](https://archlinux.org/packages/extra/x86_64/ghidra/)). AUR alternatives still exist for bleeding-edge/dev builds (`ghidra-git`) and extensions (`ghidra-extension-ghidrathon` for a modern CPython-based scripting bridge, `ghidra-mcp`) ([AUR search results](https://aur.archlinux.org/packages/ghidra-git)).

**Java requirement.** Current releases require **JDK 21 64-bit**; building from source (dev branch) wants JDK 25 ([Ghidra install docs, via GitHub repo fetch](https://github.com/NationalSecurityAgency/ghidra)).

**Decompiler & scripting.** Built-in C-like decompiler (P-code based); scripting via Java or Jython (bundled), plus **PyGhidra** — a newer official CPython-based launcher/bridge that replaces the old Jython-only workflow for real Python 3 scripting ([GitHub repo](https://github.com/NationalSecurityAgency/ghidra)).

**Executable format support — the critical part for this project.**

| Format | Native in Ghidra? | Relevance |
|---|---|---|
| PE (32/64-bit) | Yes, native (`PeLoader`) | WinNT `.sys` drivers, Win9x 32-bit user-mode EXEs/DLLs |
| NE (16-bit "New Executable") | Yes, native (`NeLoader.java`) | Win 3.x/early Win9x 16-bit DLLs the stack may still carry |
| MZ/DOS | Yes, native (`MzLoader.java`) | DOS-mode utility/config tools shipped with the robot |
| **LE/LX (Linear Executable)** | **No — not native.** Community extension **[ghidra-lx-loader](https://github.com/yetmorecode/ghidra-lx-loader)** ("Ultimate Ghidra loader for the LX/LE executable file format (OS/2, DOS4GW, VxD)") adds it, including manual base/selector override needed for VxD analysis | **Win9x VxD drivers are LE-format** — this is the one gap you must plug for the VxD side of the stack |
| ELF/Mach-O/COFF/OMF/a.out | Yes, native | not directly relevant here |

Confirmed via Ghidra's own file-formats documentation (mirrored at [nationalsecurityagency-ghidra.mintlify.app/processors/file-formats](https://nationalsecurityagency-ghidra.mintlify.app/processors/file-formats)) and corroborated by [Wikipedia's Linear Executable article](https://en.wikipedia.org/wiki/Linear_Executable) ("under Windows 9x, the VxD is a Linear Executable format file").

**16-bit x86.** Ghidra's x86 processor module includes 16-bit real-mode/16-bit-protected support (used for NE/MZ analysis); this is standard in the SLEIGH x86 spec bundled with Ghidra, though I could not pull an explicit enumerable confirmation beyond the NE/MZ loader existence — treat exact 16-bit protected-mode decompiler fidelity as **unverified/needs hands-on check**.

**Headless analyzer.** Confirmed: "can be run in both user-interactive and automated modes" — the `analyzeHeadless` script is Ghidra's standard batch-analysis entry point, useful for scripted triage of many `.vxd`/`.386`/`.sys`/`.dll` files at once.

**Collaborative server.** Ghidra Server exists for shared-project multi-user analysis (not deeply detailed in the fetched excerpt, but it's a long-standing, documented Ghidra feature) — likely overkill for a solo hobby project.

**Verdict:** Ghidra is the backbone tool here — free, Apache-2.0, packaged directly in CachyOS's Arch-derived repos, and it natively covers PE/NE/MZ. Add `ghidra-lx-loader` (and optionally `ghidra-extension-ghidrathon` for real Python 3 scripting) as day-one extensions for the VxD side.

---

## 3. The Reddit Thread and Its Article

`old.reddit.com`, `www.reddit.com`, and `web.archive.org` were all unreachable from this tool's network path (403/blocked), so the thread itself could not be read directly. However, web search located what is almost certainly the article it links to: **["The Best Software Every Reverse Engineer Should Have for Cracking Software"](https://piprogramming.org/articles/The-Best-Software-every-Reverse-Engineer-should-have-for-cracking-Software-0000000029.html)** (PiProgramming), matching the thread title and posting era (thread is dated ~Jan 2021, `koxz10`).

Tool list from that article, with a **2026 currency flag** on each:

| Category | Tool | 2026 currency note |
|---|---|---|
| Debugger | **x64dbg** | Still current and actively maintained; correctly described as OllyDbg's successor. Still relevant. |
| Disassembler/decompiler | **IDA** (free + Pro) | Still the reference tool; pricing/tiers have changed substantially since 2021 (see §4). |
| Debugger | **Qira** | Largely abandoned/niche by 2026; superseded in practice by Ghidra's debugger, x64dbg, and Frida/rr for time-travel-style workflows. **Dated.** |
| Framework | **Ghidra** | Still central; was brand-new (2019 public release) at thread time, now the de-facto standard alongside IDA. |
| .NET decompiler | **dnSpy** | Original dnSpy is unmaintained since ~2020; community forks (dnSpyEx / AsmResolver-based tools) are the maintained successors. **Dated — use a fork.** Not directly relevant to this Win9x/NT native-driver project anyway. |
| HTTP proxy | **Fiddler** | Still maintained (Progress Telerik); still relevant for any network-facing companion software, less relevant to parallel-port work. |
| Network monitor | **TCPView** | Still ships in Sysinternals; still fine. |
| API tool | **Postman** | Still current; irrelevant to a parallel-port-only stack. |
| Packet analyzer | **Wireshark** | Still the standard; irrelevant unless the haptic software also talks over network/serial-over-IP. |
| Packer ID | **ExeInfoPE**, **Detect It Easy (DiE)**, **PEStudio** | All still maintained/current in 2026; DiE and PEStudio in particular remain best-in-class for identifying 16/32-bit Windows packers and compilers, useful for triaging Win9x driver installers. |
| Hex editor | **HxD** | Still current, still free, still a good default for raw binary/VxD inspection. |
| Resource editor | **Resource Hacker** | Still current; useful for pulling icons/dialogs/version info out of NE/PE installers. |
| Sandboxing | **Sandboxie** | Now **Sandboxie-Plus**, open-sourced since 2021 — the original closed tool referenced in the article is dated, but its OSS successor is very much alive. |
| Process tools | **Process Explorer**, **RegShot**, **Scylla**, **Sysinternals Suite** | All still current (Sysinternals is Microsoft-maintained); Scylla (import-table reconstruction / dumping) pairs with x64dbg. |

**Community-consensus takeaway (as corroborated by other 2026-era sources found in search, e.g. a synthesized "starter kit" recommendation of Ghidra + x64dbg + Detect It Easy, expanding to Frida/CAPE/Wireshark for harder targets)**: the enduring core of any RE toolkit is a disassembler/decompiler (Ghidra or IDA), a live debugger (x64dbg on Windows), and a packer/format identifier (DiE/PEStudio) — this hasn't changed since 2021, but the peripheral tooling (dnSpy, Sandboxie, Qira) has each either aged out or been superseded by an actively maintained fork. None of the 2021 list addresses **retro OS formats (VxD/LE, 16-bit NE)** or **hardware-level parallel-port sniffing**, which are the two things this specific project actually needs beyond the generic list.

---

## 4. Other Static/Dynamic/Hardware Tools for This Specific Problem

### 4.1 Static disassembly/decompilation

- **IDA Free / Home / Pro** — [hex-rays.com/pricing](https://hex-rays.com/pricing), [hex-rays.com/ida-free](https://hex-rays.com/ida-free), [hex-rays.com/ida-home](https://hex-rays.com/ida-home).
  - **IDA Free**: non-commercial only; supports x86/x86-64 processors, 32-bit and 64-bit applications; decompiler is **cloud-only** (no local Hex-Rays decompiler binary); no IDAPython/C++ SDK, no Lumina/Teams, debugger limited to local x86/x64. Good enough for static disassembly of a 32-bit NT `.sys`, weaker for deep interactive decompilation work.
  - **IDA Home**: still non-commercial-only licensing, but adds **cloud-based decompilers for 5 architecture families** (PC/x86, ARM, PPC, MIPS, RISC-V) — one processor family per license. Priced as a hobbyist tier below IDA Pro (exact current price **unverified** — not returned in fetched content; check hex-rays.com/pricing directly).
  - **IDA Pro**: full commercial product, 60+ processor families, local decompiler, full SDK — priced well beyond hobby budget (**unverified exact figure**, historically several thousand USD/year on subscription).
  - Neither Free nor Home appears to natively understand **LE/VxD or 16-bit NE** any better than Ghidra does without help — IDA's loaders are proprietary and undocumented from outside, so budget for the same kind of "does it even open the file" uncertainty.

- **radare2 / rizin + Cutter** — open source, free, run natively on Linux (first-class citizen, unlike Ghidra/IDA which are Linux-ported Java/native GUI apps). rizin (the actively-maintained radare2 fork) plus **Cutter** (Qt GUI on rizin) support x86, ARM, MIPS, PowerPC, RISC-V, SPARC, WASM and more ([Rizin/Cutter search summary](https://terminaltrove.com/rizin/), [Ghidra vs Radare2 vs Rizin comparison, 2026](https://www.pistack.xyz/posts/2026-05-03-ghidra-vs-radare2-vs-rizin-self-hosted-binary-analysis-reverse-engineering-guide-2026/)). Explicit confirmation of 16-bit real-mode x86 disassembly fidelity in rizin/Cutter is **unverified** — r2/rizin has long-standing x86-16 support in its disassembler backend (asm.x86, historically via udis86/capstone-derived engines) but this wasn't independently re-confirmed in a fetched source for 2026. Good complementary/cross-check tool alongside Ghidra; scriptable (r2pipe, Python).

- **objdump** (binutils) — always available on Linux, trivially `pacman -S binutils`; useful only for quick linear disassembly / sanity checks (e.g., confirming a `.sys` is a valid PE and dumping its sections), not real analysis of VxD/LE.

- **Ghidra** — see §2, the primary tool.

### 4.2 Windows 9x / driver-format-specific tooling

- **Dependency Walker** (`depends.exe`) — [dependencywalker.com](https://www.dependencywalker.com/). **Unmaintained since ~2006** (last build 2.2.10011, Oct 2015 from WDK10); doesn't understand modern Windows loader features, but for a Win9x/NT4-era driver stack that's largely irrelevant — the tool's PE import/export walking still works fine on period binaries. Its actively-maintained open-source replacement is **[Dependencies](https://github.com/lucasg/Dependencies)** (C# rewrite, handles API-sets/WinSxS you don't need here anyway). Either works for **mapping which DLLs/VxDs a `.sys`/`.exe`/`.vxd` imports from**, which is a fast first pass to distinguish "talks straight to the port" drivers from "calls into a helper module" drivers.

- **PE tools** generically (CFF Explorer, PEview, PE-bear) — useful for manually walking PE headers/sections/imports on Windows; on Linux, `pev`/`readpe` (from the `pev` toolkit, packaged in some distros/AUR) gives equivalent PE introspection without a VM.

- **VxD/LE analysis resources** — the format itself: [Wikipedia: Linear Executable](https://en.wikipedia.org/wiki/Linear_Executable), [Wikipedia: VxD](https://en.wikipedia.org/wiki/VxD). Practically, the `ghidra-lx-loader` extension (§2) is the only actively maintained open tool found that loads LE/VxD directly into a modern decompiler; there is also a related standalone project referenced as ["A Ghidra loader for the Linear eXecutable format"](https://oshogbo.com/blog/70/) documenting the format's quirks (object tables, fixups, ring-0 entry points) that any from-scratch parser would need to replicate.

- **Identifying direct port I/O vs. a helper VxD/.sys** — this is a disassembly-pattern-matching exercise, not a single tool:
  1. Load the binary in Ghidra (or IDA), let auto-analysis run.
  2. Search the decompiled/disassembled listing for **`IN`/`OUT`/`INS`/`OUTS`** x86 opcodes referencing immediate or DX-loaded I/O addresses in the LPT ranges: **0x378–0x37F (LPT1)**, **0x278–0x27F (LPT2)**, **0x3BC–0x3BF (LPT3)** — in Ghidra you can use the decompiler's search or a script iterating instructions for mnemonic `IN`/`OUT`; in objdump/r2 you can `grep -E '\bin |\bout '` over a disassembly dump, then manually check the DX-load immediately preceding each match for one of those constants (0x378/0x278/0x3BC or a variable added to a PCI **BAR** read via `VxDCall`/`DeviceIoControl`).
  3. If **no** direct `IN`/`OUT` to those ports appears in the user-mode `.exe`/`.dll`, the software is calling through a **VxD (ring-0)** or **kernel-mode `.sys` driver** — confirmed by checking its import table (via Dependency Walker/Dependencies, or Ghidra's Symbol Tree → Imports) for `VWIN32.VXD`, `VCOMM.VXD`, a custom `.vxd`/`.386`, or an NT `CreateFile("\\\\.\\DriverName")` + `DeviceIoControl` pattern — in which case the ring-0 component (the actual VxD/.sys) is where the real port I/O and any protocol/timing logic lives, and *that* file is what needs the LE/VxD or PE-driver loader treatment.
  4. For a PCI-based interface card rather than the legacy on-board LPT, the same `IN`/`OUT` search applies but against a **BAR-relative address** read from PCI config space at driver init (`IN`/`OUT` to 0xCF8/0xCFC, or calls to `VxDCall(_PCI_...)`) rather than a fixed 0x378-family constant — grep for those PCI config port accesses as a secondary pattern.

### 4.3 Dynamic/observational tooling — emulators and VMs

This is the **cleanest path** per the task brief: run the original Win98/NT software against something that either passes real port I/O through to actual hardware (so the real robot moves, while Linux captures the wire) or logs all IN/OUT to the LPT range with timestamps.

| Tool | Passthrough to real `/dev/parport0`? | Port I/O logging? | Notes |
|---|---|---|---|
| **86Box** | Documented support is for a **named-pipe** parallel device and emulated printer devices (e.g., an Epson ESC/P printer); explicit raw hardware passthrough for the parallel port looks **less developed than its serial passthrough**, and users have explicitly requested "LPT passthrough to a real device" as a still-open wish ([86Box docs — Ports](https://86box.readthedocs.io/en/latest/settings/ports.html), [86Box GitHub discussion #4801](https://github.com/86Box/86Box/discussions/4801)). Treat direct hardware LPT passthrough in 86Box as **unverified/likely unsupported** as of 2026; the named-pipe device could be bridged to a small Linux-side helper that pokes `/dev/parport0`, but that's DIY, not built-in. No dedicated port-IO-to-file logger was found documented. |
| **PCem** | Community forum threads discuss "serial and/or parallel port redirection" as a long-requested feature; nothing found confirms it shipped ([PCem forum thread](https://pcem-emulator.co.uk/phpBB3/viewtopic.php?t=48)). **Unverified/likely no.** PCem is also less actively developed than its fork 86Box. |
| **DOSBox-X** | **Yes, confirmed and documented.** `parallel1=reallpt realport:/dev/parport0` passes the emulated LPT1 straight through to a real Linux parallel port; ECP variants exist (`reallpt realbase:378 ecpbase:778`) with some reported bidirectional-ECP limitations on Linux. Requires the invoking user to be in the `lp` group (or otherwise have R/W on `/dev/parportN`) ([DOSBox-X wiki: Setting up printing](https://dosbox-x.com/wiki/Guide:Setting-up-printing-in-DOSBox%E2%80%90X), [GitHub issue #3547](https://github.com/joncampbell123/dosbox-x/issues/3547)). This is DOS-only (no Win9x GUI-mode driver stack), so useful mainly if part of the haptic software is a DOS-mode tool, or as a controlled-environment sanity check of the raw protocol. |
| **QEMU** | **Yes, confirmed.** `-parallel /dev/parport0` (or, more robustly via `-chardev parport,...`) passes the guest's emulated LPT straight to a real Linux parallel port. Needs `modprobe parport_pc ppdev`, and `rmmod lp` (the kernel `lp` driver must not be holding the device) ([QEMU parallel port support — mailing list](https://qemu-devel.nongnu.narkive.com/KkmhaTMR/parallel-port-support), [Proxmox forum notes on parport passthrough](https://forum.proxmox.com/threads/kvm-host-parallel-port.2015/)). QEMU can run full Win98/WinNT4 guests, so **this is the strongest "(a) passthrough so original software drives the real robot while Linux captures the wire" candidate** — pair it with an external logic analyzer/sigrok tap on the DB25 cable between the parallel port and the robot (§4.4) for full protocol capture, since QEMU itself doesn't appear to offer a built-in "log every IN/OUT with timestamp" chardev backend (only pass through or standard file/pipe redirection of the printer-data stream). |
| **VirtualBox** | **Yes, confirmed, Linux-host only.** `VBoxManage modifyvm "VM" --lptmode1 /dev/parport0` plus `--lpt1 0x378 7` passes a real host parallel port through via the kernel's `ppdev` interface; requires `ppdev`/`parport_pc` loaded, `lp` unloaded, and `/dev/parport0` permissions opened (`chmod 666` or udev rule) ([VirtualBox forums — LPT passthrough](https://forums.virtualbox.org/viewtopic.php?t=54187), search summary). Note: **passthrough on *Windows* hosts was removed in VirtualBox 7.0**, but Linux-host passthrough is unaffected — directly relevant since the analyst is on CachyOS. A solid alternative to QEMU if you want a full GUI VM manager. |
| **VMware (Workstation/Player for Linux)** | Historically supported parallel-port passthrough on Linux hosts in older Workstation versions; current-generation VMware Linux support for parallel passthrough is **unverified** for 2026 (VMware's own product line has been in flux post-Broadcom acquisition, and recent Workstation Pro free-tier changes make long-term availability uncertain). Not recommended as a first choice given QEMU/VirtualBox cover the same need with clearer documentation. |
| **Bochs** | Has an internal source-level debugger with **instruction tracing** (`trace on`) and a **control port at 0x8A00** for enabling/disabling tracing and memory-range monitoring from the guest or via magic breakpoints ([Bochs docs — debugger-advanced](https://bochs.sourceforge.io/doc/docbook/development/debugger-advanced.html), [Bochs internal debugger](https://bochs.sourceforge.io/doc/docbook/user/internal-debugger.html)). This gives **whole-CPU instruction tracing**, which will show every `IN`/`OUT` executed if you trace and grep the output — a workable, if verbose, substitute for a dedicated port-IO logger, and doesn't require real hardware. No built-in real-hardware LPT passthrough was found for Bochs. |

**Direct-answer to the brief's key question:** For **(a) real passthrough to `/dev/parport0`** — **QEMU** and **VirtualBox** both confirmed and Linux-native; DOSBox-X confirmed for DOS-mode workloads. For **(b) logging all IN/OUT with timestamps** — none of the mainstream PC emulators expose this as a turnkey feature for the LPT range specifically; **Bochs's instruction-trace mode** is the closest built-in approximation (traces everything, filter for IN/OUT post-hoc), while 86Box/PCem's LPT emulation code could in principle be patched to add a raw logging sink but nothing off-the-shelf was found. There's also a small existing Linux kernel-module project, **[VPPSniffer](https://github.com/strezh/VPPSniffer)**, described as "a simple Linux 3.x kernel module for debugging and sniffing parallel port (LPT) applications without a real physical port" — worth investigating directly as a possible ready-made host-side sniffer/virtual-port logger, though its currency/compatibility with modern kernels is **unverified**.

### 4.4 Hardware — sniffing the real DB25 cable in situ

Given the ambiguity above, a **logic analyzer physically tapped onto the parallel cable between the real Win98/NT PC and the real robot** is the most trustworthy ground truth, independent of any emulator's fidelity.

- **Signal count needed**: standard SPP/EPP capture wants **17 signals** — 8 data lines (D0–D7), 4 control lines (Strobe, Auto Feed, Initialize, Select In — often inverted), 5 status lines (Ack, Busy, Paper Out, Select, Error). A basic parallel-only capture (ignoring status feedback) can get by with fewer channels, but full bidirectional/EPP capture needs all 17 plus ground reference.
- **Saleae Logic 8** — only 8 digital channels, insufficient alone for full SPP+status capture; would need to be run twice (once on data+control, once on status) or paired with a second analyzer.
- **Saleae Logic 16** — 16 digital channels, one channel short of the full 17-signal set (still workable if you drop one rarely-used control/status line, e.g. Select or Paper Out, or capture in two passes).
- **Cheap FX2-based 8/16-channel analyzers** (Cypress FX2LP-based clones, widely sold as "USB Logic Analyzer 24MHz 8CH" or "Saleae clones") — natively supported by **sigrok**'s `fx2lafw` driver family; a 16-channel FX2 clone plus sigrok is the standard hobby-budget answer, and two of them (or one 24-32ch analyzer) covers all 17 signals with margin. Parallel-port clock rates are low (µs-scale strobe pulses), so even a cheap 24 MHz FX2 analyzer has enormous timing margin.
- **sigrok / PulseView**: confirmed to ship a **"Parallel" protocol decoder**, described as a "generic parallel synchronous bus" decoder ([sigrok Protocol decoders wiki](https://sigrok.org/wiki/Protocol_decoders)) — this decodes an arbitrary N-bit parallel word against a clock/strobe line, which is exactly the shape of Centronics/SPP data-plus-strobe traffic (no dedicated "Centronics" or "IEEE1284" decoder was found, but the generic parallel decoder covers the same job, and decoder stacking lets you add custom interpretation on top). PulseView is the sigrok project's Qt-based capture/analysis GUI ([sigrok.org](https://sigrok.org/wiki/Getting_started_with_a_logic_analyzer), [PulseView manual](https://sigrok.org/doc/pulseview/0.4.1/manual.html)); both are packaged in Arch/CachyOS repos (`sigrok-cli`, `pulseview`, `libsigrokdecode`, `libsigrok`).
- **DB25 breakout/tap boards** — passive breakout boards (DB25 male-to-female or male-to-terminal-block) are cheap and widely available from generic electronics vendors; needed to get probe access to all 25 pins without cutting the original cable. A cheap "DB25 breakout board" or "parallel port tester" board plus jumper wires to the logic analyzer's probes is the standard hobbyist rig.

**Recommendation**: a 16-channel FX2-based analyzer (sigrok-supported) is close enough to the 17-signal ideal for practical purposes — drop one control line you're confident is unused (Select In is rarely wired on custom peripherals) or run status separately once behavior is understood from data+control alone.

---

## 5. Recommended Toolchains

### Hobby-budget, Linux-only toolchain (~$0, all free/open-source)

| Layer | Tool | Cost |
|---|---|---|
| Static disasm/decompile | **Ghidra** (`pacman -S ghidra`) + `ghidra-lx-loader` extension for VxD/LE | Free |
| Static disasm/decompile (secondary) | **rizin + Cutter** for cross-checking, and **Binary Ninja Free** for its HLIL view on the 32-bit PE side | Free |
| Import/PE triage | **Dependencies** (open-source Dependency Walker successor) run in a lightweight Win98/WinNT VM, or `pev`/`readpe` on Linux for a quick pass | Free |
| Dynamic passthrough | **QEMU** with `-parallel /dev/parport0` (or VirtualBox `--lptmode1`) running Win98SE/NT4 as guest, real robot on the real port | Free |
| Ground-truth capture | Cheap FX2-based 16-channel logic analyzer (~$10–30) + **sigrok/PulseView**, generic "Parallel" decoder, DB25 breakout board (~$5–15) | ~$15–45 hardware |
| Fallback instruction trace | **Bochs** trace mode if hardware passthrough proves unreliable | Free |

Total hardware spend: roughly the price of a cheap logic analyzer and a breakout board — everything else is free and already fits comfortably in CachyOS's package ecosystem (Ghidra is in `extra`; sigrok/pulseview are in `extra`/AUR; QEMU/VirtualBox are standard).

### "Gold-plated" alternative

| Layer | Tool | Approx. cost |
|---|---|---|
| Primary decompiler | **Binary Ninja Ultimate** (named) | $3,499 (6.0 pricing) |
| or **IDA Pro** | full commercial license, 60+ architectures, local decompiler | Several thousand USD/yr, unverified exact figure — quote from hex-rays.com/pricing |
| Reference framework | **Ghidra** anyway (it's free and covers formats the paid tools may not touch cleanly, e.g. LE/VxD via `ghidra-lx-loader`) | Free |
| Logic analyzer | **Saleae Logic 16** (or two Logic 8 units) for high-fidelity, well-supported capture with Saleae's own software plus sigrok compatibility | ~$400–500 (Logic 16, price unverified as of 2026 — check saleae.com) |
| Dynamic environment | Dedicated period-correct physical Win98/WinNT4 PC with a real LPT port (avoids any emulator-fidelity questions entirely) alongside QEMU/VirtualBox for convenience | Cost of used period hardware, highly variable |
| Collaboration | Binary Ninja Enterprise / Ghidra Server if working with a team | Enterprise pricing, à la carte per 6.0 changes |

---

## 6. Summary Comparison Table

| Tool | Type | License/Cost | Linux-native | Handles PE (32-bit) | Handles NE (16-bit) | Handles LE/VxD | Headless/scriptable |
|---|---|---|---|---|---|---|---|
| Ghidra | Static SRE suite | Free, Apache-2.0 | Yes | Yes, native | Yes, native | No natively; via `ghidra-lx-loader` extension | Yes (`analyzeHeadless`), Java/Jython/PyGhidra |
| IDA Free | Static disasm | Free, non-commercial | Yes | Yes | Unverified | Unverified | No SDK on Free |
| IDA Home | Static disasm/decompile | Paid (price unverified), non-commercial | Yes | Yes (cloud decompiler) | Unverified | Unverified | Limited |
| Binary Ninja Free | Static disasm/decompile | Free | Yes | Yes (believed; not tier-gated) | Unverified | Unverified | Limited (per FAQ table) |
| Binary Ninja Ultimate | Static disasm/decompile | $3,499 named (6.0) | Yes | Yes | Unverified | Unverified | Yes, plus Headless license option |
| rizin/Cutter | Static disasm | Free, open-source | Yes (native) | Yes | Unverified (likely yes) | No | Yes (r2pipe) |
| QEMU | Emulator, dynamic | Free, open-source | Yes | n/a (host tool) | n/a | n/a | Confirmed `/dev/parport0` passthrough |
| VirtualBox | Emulator, dynamic | Free (base), open-core | Yes | n/a | n/a | n/a | Confirmed `/dev/parport0` passthrough (Linux host) |
| DOSBox-X | Emulator, dynamic | Free, open-source | Yes | n/a | n/a | n/a | Confirmed `reallpt` passthrough |
| 86Box / PCem | Emulator, dynamic | Free, open-source | Yes | n/a | n/a | n/a | LPT passthrough unverified/likely absent |
| Bochs | Emulator, dynamic | Free, open-source | Yes | n/a | n/a | n/a | No passthrough; has instruction-trace debugger |
| sigrok/PulseView | Hardware capture | Free, open-source | Yes (native) | n/a | n/a | n/a | Generic "Parallel" decoder confirmed |

---

## Notes on unverifiable items

- Exact current IDA Home / IDA Pro prices were not returned by the fetched pricing page content — check [hex-rays.com/pricing](https://hex-rays.com/pricing) directly for current figures.
- Binary Ninja Free's 32-bit x86 decompiler access, and rizin/Cutter's 16-bit real-mode disassembly fidelity, are inferred from general architecture-support statements rather than an explicit per-tier/per-mode confirmation — verify hands-on before relying on either for the 16-bit NE portions of this project.
- 86Box/PCem raw hardware LPT passthrough and any of the emulators' built-in "log IN/OUT with timestamp" feature were not confirmed to exist as shipped features; treat as open questions requiring direct testing or source inspection, or plan to rely on the logic-analyzer ground-truth path instead.
- The original Reddit thread (r/programming, `koxz10`) could not be fetched directly due to network restrictions on reddit.com and web.archive.org from this tool; the linked article was identified via search and fetched successfully, and is very likely the correct source given exact title match and era.
