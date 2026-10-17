# The device: SensAble PHANTOM, parallel (EPP) interface generation

*Research compiled 2026-09-06 by a web-research agent, edited. Items marked **[UNVERIFIED]** need a primary-source check. Owner's description: "PHANTOM pro v1 parallel desktop", fully out of support; PCIe parallel card fitted, cable connected, Linux enumerates the port.*

## 1. Which model

SensAble's naming shifted over ten years of production:

- **PHANToM 1.0 (1993–96)**: Massie & Salisbury's MIT device, 3-DOF force, stylus/thimble end-effector ([Massie & Salisbury 1994, PDF](https://alliance.seas.upenn.edu/~medesign/wiki/uploads/Courses/Massie94-DSC-Phantom.pdf)).
- **PHANTOM Premium 1.0 / 1.0A / 1.5 / 1.5HF / 3.0**: the commercial line; "Premium" was added once the cheaper Desktop appeared. The device guide states the unit "connects to the PC via the parallel port (EPP) interface" ([Premium 1.0/1.5/HF Device Guide, 3D Systems S3](https://s3.amazonaws.com/dl.3dsystems.com/binaries/Sensable/UserGuide/Premium1.0_1.5_HF_Device_guide.pdf); [Dentsable Premium 1.0 page](https://dentsable.com/phantom-premium-1-0.htm)).
- **PHANTOM Desktop (~1998–2000 on)**: compact, lower-cost; "a simple parallel port interface", IEEE-1284 EPP cable with a ferrite bead ([Desktop Device Guide, 3D Systems S3](https://s3.amazonaws.com/dl.3dsystems.com/binaries/Sensable/UserGuide/Desktop_Device_Guide.pdf); [Dentsable Desktop page](https://dentsable.com/haptic-phantom-desktop.htm)).
- **"Pro" / "1.0 Pro"**: not found as an official SensAble model name **[UNVERIFIED]**. Possibly a GHOST SDK edition sticker or an informal lab label. Confirm from the base plate.

Desktop vs Premium 1.0: the Desktop is smaller and lighter with a shorter reach; the Premium bolts to a desk mount and is bulkier. Both use the same DB25 EPP cable, so the label is the discriminator, not the cable.

## 2. The interface: straight EPP, no interface card

**Most important finding.** The Premium 1.0/1.5 and Desktop connect **directly to the PC parallel port in IEEE-1284 EPP mode**. There is no ISA/PCI interface card for this generation; the DB25 cable runs from the PC to the device base (device guides above).

Corroborated by a community Linux how-to for a **Premium 1.5 over parallel**: confirm the port is in EPP mode via `/proc/sys/dev/parport/parport0/modes`, then symlink `/dev/phnepp → /dev/parport0` and `/dev/phnepp_modes → /proc/sys/dev/parport/parport0/modes` for SensAble's **userspace** Phantom Device Driver ([jaejunlee0538/sensable_phantom_in_linux README](https://github.com/jaejunlee0538/sensable_phantom_in_linux/blob/master/README.md)).

**Consequence: SensAble's own Linux driver for this device was a userspace binary driving the standard kernel `ppdev` interface.** No custom kernel module existed for the parallel path. This opens a purely software observation route (see `docs/research/capture-strategy.md`).

An external **amplifier box** exists in the family (DB25 in, motor-power cable out, does the motor current loops) but it is a power/amplifier enclosure, not a bus interface ([Stanford BDML PHANToMs page](https://bdml.stanford.edu/twiki/bin/view/Haptics/PHANToMs)) **[partially UNVERIFIED for this exact model]**. Larger/later devices moved to a **PCI card (PLX 9050 bridge)**, then FireWire (Omni, 2004), then USB/Ethernet (Touch). Quanser's QUARC docs list the three legacy driver families: "Parallel Port: PDD v5.1.7+", "FireWire: PDD v5.1.7+", "Ethernet/USB: Touch Device Driver 2016+" ([Quanser](https://docs.quanser.com/quarc/documentation/phantom_block.html)).

## 3. The Windows stack: GHOST SDK + Phantom Device Driver

- **GHOST** (General Haptics Open Software Toolkit): C++ scene-graph SDK over a low-level driver. GHOST 4 (Aug 2002) added Red Hat Linux and Windows XP; "available for Windows NT, 2000, XP, and Red Hat Linux" ([HPCWire](https://www.hpcwire.com/2002/08/16/sensable-technologies-announces-ghost-version-4/): 403'd on fetch; text from search snippet **[reconfirm]**). So the Win95/98/NT4 support the owner remembers is GHOST 2.x/3.x, mid-to-late 1990s. Wayback Machine for sensable.com 1998–2001 is the place to pin this down.
- GHOST 4 added **`gstDeviceIO`**: raw encoder counts, direct force commands, motor temperatures. A public description of the lowest documented software layer.
- Utilities: **"PHANTOM Configuration"** (port/IRQ/device type, calibration store) and **"PHANTOM Test"**; a 3D Systems KB says to re-run both to regenerate deleted config/calibration files ([3D Systems support](https://support.3dsystems.com/s/article/Haptic-Device-Drivers)) **[page rendered as JS shell; from snippet]**.
- File names `phantom.sys`, `phantomio.vxd`, `ghost*.dll` are plausible by architecture but **[UNVERIFIED]**. A surviving install directory or CD would settle it.
- Later the driver was rebranded **PDD** (Phantom Device Driver); PDD 5.x targeted XP+ and would never have supported 98/NT4. The Win98/NT4 installer is abandonware in the fullest sense: not sold, not listed, not archived by the current owner.

## 4. Linux and the GPL question

- **`drivers/misc/phantom.c`** in mainline Linux (Jiří Slabý, 2007, GPL-2.0-or-later) is a **PCI-only** driver matching `PCI_VENDOR_ID_PLX, PCI_DEVICE_ID_PLX_9050`. It exposes `PHN_GET_REG`/`PHN_SET_REG`/`PHN_GETREGS`/`PHN_SETREGS` ioctls over three BARs (config, 8 input regs, 8 output regs) and a `PHN_CONTROL` register with `PHN_CTL_IRQ` and `PHN_CTL_AMP` (amplifier enable) bits ([source](https://github.com/torvalds/linux/blob/master/drivers/misc/phantom.c); [LWN](https://lwn.net/Articles/232954/); [LKML thread](http://lkml.iu.edu/hypermail/linux/kernel/0703.1/2282.html)). It needs a userspace library (Slabý's, at a probably-dead fi.muni.cz URL) or OpenHaptics on top. **It has no parallel/EPP path.** It is still PUBLIC-class evidence of the register model of a sibling interface: 8 in / 8 out registers, an amp-enable bit, and a 1–2 kHz loop.
- For the **parallel/EPP** device, SensAble's official Linux support (PDD 4.3 / OpenHaptics Academic Edition, ~2011) was closed-source userspace over `ppdev`. **SensAble / 3D Systems never released the parallel protocol or driver as open source.**

## 5. Low-level characteristics (public record)

- Canonical design paper: Massie & Salisbury 1994 (PDF above). Serial linkage, DC brushed motors with cable/capstan transmissions, incremental optical encoders per joint. Exact encoder counts and DAC bit depth need a direct read of the PDF **[not extracted this pass]**.
- Servo loop: ~**1 kHz** standard; the kernel-driver thread discusses 2 kHz.
- Encoders are relative, so a **home/reset gesture** at power-up is required; the driver stores a calibration file on the host.
- **Amplifier enable** is a distinct software-gated step (mirrors `PHN_CTL_AMP` on the PCI sibling). Any bring-up sequence must find and respect it; this is also the natural safety interlock for our stack.

## 6. Patents

- **US 5,587,937** "Force reflecting haptic interface", MIT, Massie & Salisbury, priority 1993-10-01, granted 1996-12-24, **expired 2013-12-24** ([Google Patents](https://patents.google.com/patent/US5587937A/en)).
- Later family members (e.g. **US 6,879,315**, **US 7,411,576**, **US 7,714,836**) have their own dates; do not assume the whole family is expired. Check each individually if it ever matters. For a private, non-commercial experimental project under UK Patents Act 1977 s60(5) it does not.
- "US 6,417,638" from the brief: **[UNCONFIRMED]**, do not cite.

## 7. Ownership chain

SensAble Technologies (Woburn, MA) → **Geomagic** (2012) → **3D Systems** (2013). 3D Systems sells Touch/Touch X and OpenHaptics today and hosts surviving SensAble documentation on its S3 bucket. Legacy PDD drivers for Win7/10-era and some Linux are gated behind free registration on the support portal; nothing for Win98/NT4. Nothing has ever been open-sourced by the owner.

## 8. Existing open work

| Project | What it is | Useful to us? |
|---|---|---|
| `drivers/misc/phantom.c` | GPL PCI/PLX9050 register passthrough | PUBLIC evidence of register model, amp-enable, loop rate. Not the same interface. |
| jaejunlee0538/sensable_phantom_in_linux | Install notes for the proprietary Linux PDD incl. parallel Premium 1.5 | Confirms userspace-over-ppdev design; a strace target if the binary can be obtained. |
| CHAI3D | Open haptics framework; PHANTOM backend links proprietary OpenHaptics | No protocol knowledge. |
| H3D API | Open haptics API; forum thread "OpenHaptics and Haptic Drivers are not open" ([h3d.org](https://h3d.org/forums/topic/openhaptics-and-haptic-drivers-are-not-open/)) **[fetch failed]** | Confirms the gap. |

**No repository, thesis, or post was found that has reverse-engineered the parallel PHANTOM's EPP wire protocol.** This is genuinely undocumented territory, which is both the cost and the reason a paper would be interesting.
