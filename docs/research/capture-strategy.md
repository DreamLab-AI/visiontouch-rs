# Capture strategy: how we learn the protocol without decompiling anything

The device research (`device-phantom-parallel.md`) established that the parallel PHANTOM is a plain IEEE-1284 EPP peripheral and that SensAble's own Linux driver was a **userspace** program driving the standard kernel `ppdev` interface. That gives three observation routes, all in the OBSERVE/BLACKBOX provenance classes. Use them in this order.

## Route 1: trace the vendor's Linux userspace driver (no extra hardware)

If a copy of the Linux Phantom Device Driver (PDD 4.3 / "OpenHaptics Academic Edition for Linux", ~2011) can be obtained lawfully (3D Systems support portal behind free registration, or an archived university copy), then on the CachyOS host:

```sh
sudo modprobe parport parport_pc ppdev
cat /proc/sys/dev/parport/parport0/modes        # must include EPP
sudo ln -s /dev/parport0 /dev/phnepp
sudo ln -s /proc/sys/dev/parport/parport0/modes /dev/phnepp_modes
strace -f -tt -T -e trace=openat,ioctl,read,write,close \
       -e signal=none -o pdd.strace ./<vendor-pdd-binary>
```

`ppdev` has no in-kernel bulk path that bypasses the syscall boundary, so the strace log **is** the complete register-level dialogue: every `PPWDATA`/`PPRDATA`/`PPWCONTROL`/`PPRSTATUS`/`PPNEGOT`/`PPSETMODE` ioctl, and every EPP `read()`/`write()` byte, with microsecond timestamps. Decode with a small script into a timeline of (t, register, direction, value).

Caveats: the binary is 32-bit and ~15 years old (may need `lib32-*` packages and a compatibility shim; the jaejunlee0538 notes cover `libraw1394` for the FireWire path, not needed here). If the binary uses `ioperm`/`outb` instead of ppdev it would not need the `/dev/phnepp` symlink; the install notes say it does need it, so it is ppdev. Legally this is s50BA observation of a program we are entitled to run.

## Route 2: run the original Win98/NT stack in QEMU with port passthrough

QEMU's parallel device (`-parallel /dev/parport0`, or `-chardev parallel,id=lpt,path=/dev/parport0 -device isa-parallel,chardev=lpt`) presents a legacy port at **0x378** to the guest and forwards every access to the host `ppdev`, including the EPP address/data registers. This neatly solves the "vendor driver hardcodes 0x378 but the PCIe card lives at a BAR address" problem: the guest sees 0x378, the host driver knows the real address.

Two capture layers, use both:

```sh
qemu-system-i386 ... -parallel /dev/parport0 \
    -trace 'parallel_ioport_*' -D qemu-parallel.log      # guest-side register log
strace -f -tt -T -e trace=ioctl,read,write -o qemu.strace qemu-system-i386 ...   # host-side
```

Caveats:
- Each guest port access is a VM exit plus a ppdev syscall, roughly 5–20 µs. A 1 kHz servo cycle with a few dozen accesses may still fit, but jitter will be large and the vendor driver may report timeouts. Protocol *content* is still valid; use Route 3 for timing.
- Win98 needs a 32-bit guest with the vendor's Win98 driver (VxD) or NT4/2000 with the `.sys`; NT4/2000 is easier to get running in QEMU.
- VirtualBox on a Linux host also passes through `/dev/parport0` (`--lptmode1`), but has no register-level trace; QEMU is preferred.

## Route 3: logic analyser on the DB25 (ground truth, timing-accurate)

Full SPP/EPP capture needs 17 signals: D0–D7, nStrobe/nWrite, nAutoFeed/nDataStb, nInit, nSelectIn/nAddrStb, nAck/Intr, Busy/nWait, PaperOut, Select, nError. A 16-channel FX2-based analyser (sigrok-supported, ~£15–45) plus a DB25 breakout board covers it if one static line is dropped; a Saleae Logic 16 or a 32-channel unit covers everything. sigrok/PulseView ships a `parallel` protocol decoder; save `.sr` files and export VCD for replay tests.

Use this to: validate EPP handshake timing, confirm what Routes 1/2 show, catch anything that happens at power-on before software runs, and measure the real servo period.

## What to look for, in order

1. **Enumeration / presence detect**: what the driver does first; does it probe a device ID?
2. **Mode negotiation**: does it call `PPNEGOT` to EPP, or assume it?
3. **Firmware upload**: is there a large write burst at start-up? If so, see `docs/legal/landscape.md` §11 before going further.
4. **Amplifier enable** and any safety handshake (the PCI sibling has an explicit `PHN_CTL_AMP` bit).
5. **Servo cycle**: the repeating pattern at ~1 kHz; which addresses are read (encoders, status) and written (DAC/force, control).
6. **Encoder homing / calibration** gesture and what the driver stores in its calibration file.
7. **Shutdown**: how forces are zeroed and the amp disabled.

## Safety during capture

The device applies force to a human hand. During any capture with the vendor software driving the robot: keep hands clear of the workspace, park the stylus in the home position, know where the power switch is, and if the amplifier box has a separate motor-power feed, have that switchable. Do not put a hand in the gimbal until our own stack has a working watchdog.
