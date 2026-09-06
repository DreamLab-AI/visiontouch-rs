# Rust parallel-port driver stack on modern Linux (CachyOS) for a 1 kHz haptic loop

*Research compiled 2026-09-06 by a web-research agent. Items not verified against a primary source are marked **[UNVERIFIED]**.*

## 1. The Linux parallel-port subsystem

Linux exposes the parallel port through a layered subsystem. **`parport`** is the generic core (port sharing, IEEE-1284 signalling state); **`parport_pc`** is the x86 low-level driver for the SPP/EPP/ECP register set; on top sit client drivers: **`lp`** (printer) and **`ppdev`**, which exposes each port as `/dev/parport0` etc. for userspace `ioctl()` access. Module parameters (`parport_pc io=0x378 irq=7,auto`), IRQ-sharing caveats and the `/proc/sys/dev/parport` tree are documented in [Documentation/admin-guide/parport.rst](https://github.com/torvalds/linux/blob/master/Documentation/admin-guide/parport.rst) ([rendered](https://docs.kernel.org/admin-guide/parport.html)). The in-kernel low-level API is at [driver-api/parport-lowlevel](https://www.kernel.org/doc/html/latest/driver-api/parport-lowlevel.html).

**The ppdev ioctl API** ([include/uapi/linux/ppdev.h](https://github.com/torvalds/linux/blob/master/include/uapi/linux/ppdev.h), confirmed directly against kernel master):

- `PPCLAIM` / `PPRELEASE` / `PPEXCL` / `PPYIELD` — port arbitration. `PPCLAIM` before any I/O; `PPEXCL` before `PPCLAIM` for exclusive access.
- `PPRDATA` / `PPWDATA` — 8-bit data register.
- `PPRCONTROL` / `PPWCONTROL` / `PPFCONTROL` (mask+value read-modify-write) — strobe / autofeed / init / select-in lines.
- `PPRSTATUS` — busy / ack / paper-out / select / error lines.
- `PPDATADIR` — switch the data bus to input (bidirectional reads).
- `PPNEGOT` — on-the-wire IEEE-1284 mode negotiation (compat, nibble, byte, EPP, ECP, ECPRLE).
- `PPSETMODE` / `PPGETMODE` / `PPSETPHASE` / `PPGETPHASE` — mode used by subsequent `read()`/`write()`.
- `PPGETMODES` — hardware capability bitmask.
- `PPSETTIME` / `PPGETTIME` — peripheral timeout.
- `PPWCTLONIRQ` — pre-arm control-line values written the instant an interrupt fires; `PPCLRIRQ` reads/clears the interrupt count.
- `PPGETFLAGS` / `PPSETFLAGS` — `PP_FASTWRITE`, `PP_FASTREAD`, `PP_W91284PIC`.

**CachyOS build configuration** — from the actual [linux-cachyos config](https://github.com/CachyOS/linux-cachyos/blob/master/linux-cachyos/config):

```
CONFIG_PARPORT=m
CONFIG_PARPORT_PC=m
CONFIG_PARPORT_SERIAL=m
CONFIG_PARPORT_PC_FIFO=y
CONFIG_PARPORT_PC_SUPERIO=y
CONFIG_PARPORT_1284=y
CONFIG_PRINTER=m
CONFIG_PPDEV=m
```

Everything needed is built as a module and not disabled. `modprobe parport parport_pc ppdev` (plus `parport_serial` for MosChip/WCH combo cards), then a udev rule or group membership on `/dev/parport0`. The `-rt-bore` variant presumably inherits the same driver config **[UNVERIFIED — not diffed]**.

## 2. Userspace alternatives to ppdev

**Raw port I/O via `ioperm()`/`iopl()`** ([ioperm(2)](https://man7.org/linux/man-pages/man2/ioperm.2.html), [iopl(2)](https://man7.org/linux/man-pages/man2/iopl.2.html)) lets a `CAP_SYS_RAWIO` process issue `in`/`out` directly. x86-only, not inherited across `fork()`, and incompatible with `parport_pc` owning the port. Legacy ISA I/O cycles were ~1 µs; on modern LPC/PCIe-bridged paths the cost is dominated by bridge translation and can be higher **[approximate; no authoritative modern microbenchmark found]**. A `ppdev` ioctl adds a syscall (hundreds of ns with mitigations). For a 1 kHz loop touching a dozen registers per cycle either is fine in isolation; the real risk is **tail latency from scheduling jitter**, which is why CPU isolation and SCHED_FIFO matter far more than shaving ioctl overhead.

**`libieee1284`** ([twaugh/libieee1284](https://github.com/twaugh/libieee1284)) abstracts ppdev/raw I/O across OSes. Convenient but an indirection layer we don't need; LinuxCNC-style projects bind ppdev or raw I/O directly.

**Throughput**: bit-banged SPP ~40–60 kbit/s; hardware EPP 500 KB/s–2 MB/s with the strobe/wait handshake done in silicon; ECP with DMA ~2–2.5 MB/s ([IEEE 1284 — Wikipedia](https://en.wikipedia.org/wiki/IEEE_1284)). If the original protocol depends on software-paced setup/hold delays, EPP's hardware handshake may violate them; if it is genuinely EPP, the hardware handshake is a large win.

## 3. Hardware reality in 2026

**No consumer motherboards ship an LPT header**; some industrial boards do ([LinuxCNC PCI/PCIe card thread](https://forum.linuxcnc.org/18-computer/42826-parallell-port-pci-pcie-add-in-card-info-thread)).

**PCIe parallel cards** — confirmed in [drivers/parport/parport_serial.c](https://github.com/torvalds/linux/blob/master/drivers/parport/parport_serial.c): MosChip/ASIX **MCS9901** and WCH **CH382/CH382L** are matched by PCI ID in-tree. Caveats:
- MCS9901 reports are mixed on some kernels ([LinuxCNC](https://www.forum.linuxcnc.org/38-general-linuxcnc-questions/33322-cant-get-pci-e-parallel-card-to-work-moschip-ms9901-cv-cc)); CH382L clones with the same PCI ID exist; the single-port WCH variant needed [a kernel patch](https://www.spinics.net/lists/kernel/msg2524513.html).
- **Sunix** cards use a vendor driver bypassing `parport_pc` ([LinuxCNC](https://forum.linuxcnc.org/38-general-linuxcnc-questions/35025-sunix-pci-parallel-port-cards)). **Oxford** OX16PCI952-class chips are natively supported and generally the most reliable.
- **PCIe cards do not sit at 0x378/0x278/0x3BC.** The BAR is assigned at boot. Anything that hardcodes 0x378 (including the original Win98/NT driver in a passthrough VM) will fail. Discover via `lspci -v`, sysfs `resource`, or `/proc/sys/dev/parport/parport0/base-addr`.
- EPP/ECP support on these cards is chip/revision dependent. CH382 datasheet claims EPP/ECP ([datasheet](https://bitsavers.org/components/wch/_dataSheets/CH382DS0.PDF)) but at least one user could not get EPP working ([LinuxCNC](https://www.forum.linuxcnc.org/38-general-linuxcnc-questions/31715-pci-e-parallel-card-problem)).

**USB-to-parallel adapters are printer-class only.** They implement the USB Printer Class byte stream via `usblp`; there is no transaction meaning "assert pin 2 now". Kernel developers rejected a ppdev-over-USB interface as impractical ([LKML](https://lkml.iu.edu/hypermail/linux/kernel/0511.3/0605.html)).

**Protocol-bridge alternative**: implement the DB25-side timing on a microcontroller and expose a framed protocol over USB.
- **FTDI FT232H** bit-bang / MPSSE ([AN_373](https://ftdichip.com/Support/Documents/AppNotes/AN_373%20Bit-Bang%20Modes%20for%20the%20FT-X%20Series.pdf), [datasheet](https://ftdichip.com/wp-content/uploads/2023/09/DS_FT232H.pdf)). Mature Linux support; but USB round-trip latency means strobe/ack logic must live on the adapter side.
- **RP2040 / Teensy**: PIO state machines give cycle-accurate DB25 timing independent of Linux scheduler jitter; host sees "set force X / read encoder Y" over USB. More firmware work, but moves the hard-real-time boundary off Linux. Arguably the better-engineered 2026 answer; still bounded by USB polling on the host↔MCU leg.

## 4. Rust ecosystem and real-time userspace

crates.io API queried directly: **zero crates named or matching `parport`, `ppdev`, or `ieee1284`**. The HAL is greenfield. For ioctl bindings:
- **`nix`** ([docs](https://docs.rs/nix/latest/nix/)) — `ioctl_read!` / `ioctl_write_ptr!` / `ioctl_write_int!` macros map directly onto `ppdev.h`'s `_IOR`/`_IOW`/`_IO`.
- **`rustix`** ([crates.io](https://crates.io/crates/rustix)) — `no_std`-friendly, lower-level raw ioctl path; see [Rust and ioctl (meka.rs, 2025)](https://meka.rs/blog/2025/03/18/rust-and-ioctl/).

**Kernel module in Rust: no.** Rust-for-Linux is mature (merged 6.1, no longer "experimental" as of late 2025 — [Phoronix](https://www.phoronix.com/news/Rust-To-Stay-Linux-Kernel)), but `parport` has no Rust abstraction layer and a device-specific client driver has no need to be in kernel space: `PPWCTLONIRQ`/`PPCLRIRQ` already give userspace the interrupt-driven ack path. **Stay in userspace.**

**Real-time scheduling in Rust userspace:**
- `sched_setscheduler(SCHED_FIFO)` via `libc` or the `thread-priority` crate (v3.1.1). Also `periodic-thread` and `preempt-rt` crates: small, worth reading rather than depending on for a safety-critical loop.
- `mlockall(MCL_CURRENT | MCL_FUTURE)`; pre-allocate everything; no allocation on the hot path.
- `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME)` to an absolute deadline ([man](https://man7.org/linux/man-pages/man2/clock_nanosleep.2.html)); coarse sleep to ~200 µs before deadline then spin on `clock_gettime`. `timerfd` adds no precision over this.
- CPU isolation: `isolcpus=`, `nohz_full=`, `rcu_nocbs=` ([kernel docs](https://docs.kernel.org/admin-guide/cpu-isolation.html)); cgroup v2 cpuset isolated partitions are the modern runtime alternative.
- **PREEMPT_RT** merged in mainline 6.12 ([Phoronix](https://www.phoronix.com/news/Linux-6.12-Does-Real-Time)). Default `linux-cachyos` has `CONFIG_PREEMPT_RT` unset (uses `PREEMPT_DYNAMIC`); **`linux-cachyos-rt-bore`** is the RT variant ([CachyOS wiki](https://wiki.cachyos.org/features/kernel/), [issue #193](https://github.com/CachyOS/linux-cachyos/issues/193)). For a 1 kHz loop with force-safety deadlines: `-rt-bore` + `isolcpus`/`nohz_full` + `SCHED_FIFO`. The default kernel is fine for prototyping.

## 5. Recommended Rust architecture

- **`hal`** — a `PortIo` trait (`read_data`, `write_data`, `read_status`, `write_control`, `set_data_direction`, `negotiate_mode`) with backends: `ppdev` (safest, portable), `ioperm` (x86-only, lowest per-access latency, exclusive ownership), and a **mock / recorded-trace backend** replaying captured I/O for hardware-free tests.
- **`protocol`** — pure, `no_std`-friendly bit-level protocol (framing, checksums, command/response state) over an abstract byte/bit stream. Validated against captured traces; fuzzable.
- **`device`** — state machine: calibration, encoder reads, force output, and safety: **watchdog** (missed deadline ⇒ zero force within one cycle), **force clamping** (hard limits independent of the controller), **fail-safe on loss of loop** (daemon death must leave the hardware at zero force / braked, which realistically needs a hardware- or bridge-side watchdog since software cannot protect against its own hang).
- **`daemon`** — SCHED_FIFO, `mlockall`'d RT thread at 1 kHz on an isolated CPU; Unix socket for control-plane commands; `memfd` shared-memory ring for the high-rate data path.
- **bindings** — thin IPC clients for the application language.

## 6. Testing strategy

Capture with sigrok/PulseView into `.sr` ([format](https://sigrok.org/wiki/File_format:Sigrok/v2)) or export VCD ([sigrok VCD](https://sigrok.org/wiki/File_format:Vcd)). Feed the decoded pin timeline into the `protocol` crate's mock backend as timestamped register reads/writes and assert the decoded commands/responses. Regression-test glitches, out-of-spec timing and error frames without the robot present.
