# Device identification

Fill this in from physical inspection (source class OWNER in the provenance log).

| Item | Answer |
|---|---|
| Exact base-plate / pedestal label text, model and serial, date code | Owner reports PHANToM 1.0A, part 02278, RoHS; serial/date not supplied (P-0002) |
| Is "PRO" silkscreened as part of the model name, or a separate sticker? (photo) | |
| PC-end connector: DB25 male straight into the port? Any box in between? | Owner confirms direct DB25 cable from PCIe parallel card to unit, no intermediate box (P-0003) |
| Device-end connector type | |
| Separate amplifier / power box? What is printed on its PSU (V, A)? | Owner reports all one unit, no separate interface box; PSU rating unknown (P-0003) |
| Degrees of freedom sensed: XYZ only, or gimbal orientation too? | Six packet counters observed; owner confirms rotary pen encoder. Last-three axis mapping/calibration pending (P-0031) |
| Any surviving driver media or install directory listing (`.sys`, `.vxd`, `.dll`, `.inf` names)? | |
| Which GHOST / PDD version, if any documentation survives | |
| PCIe parallel card: chipset (`lspci -nn`), base address (`/proc/sys/dev/parport/parport0/base-addr`), modes (`/proc/sys/dev/parport/parport0/modes`) | Oxford OXPCIe952, 1415:c110 at 02:00.0; base addresses 53264 / 53248; PCSPP,TRISTATE,EPP,ECP; IRQ 18 (P-0004) |

## Host-side checks to record now

```sh
lspci -nn | grep -i -E 'parallel|serial|1c00|9710|1415'
lsmod | grep -E 'parport|ppdev'
cat /proc/sys/dev/parport/parport0/modes
cat /proc/sys/dev/parport/parport0/base-addr
cat /proc/sys/dev/parport/parport0/irq
ls -l /dev/parport0
dmesg | grep -i -E 'parport|ppdev'
```

The `modes` file must list `EPP` for this device. If the card only reports `PCSPP,TRISTATE`, EPP will need to be forced via `parport_pc` module options or the card is unsuitable.

## Host access check (2026-10-16)

Host is Omarchy 4.0.4 (Arch-based), rather than the CachyOS assumed in earlier research. `/dev/parport0` exists on the host as character device 99:0, mode 0660, owner root, group lp. The original agent process belongs to john and wheel, not lp. After the owner ran `sudo usermod -aG lp john`, `getent group lp` and `id john` confirmed account membership in lp; the running agent process still had stale groups and read/write permission checks returned false. A full desktop logout/login or reboot and reopening the app is pending verification. The restricted agent filesystem hides this device; an unsandboxed read confirmed it. No port was opened or negotiated, and no device commands were issued. EPP is advertised, not yet validated on the wire.

## End-effector investigation (2026-10-16)

Owner reports three main motors and possibly two smaller motors near the end
effector (P-0031). Do not equate encoder count with actuator count. The currently
configured reference reports six input DOFs and three output DOFs; synthetic
wrist-torque requests produce no additional output packets. This configuration
is not a physical identification. Preserve the possible wrist actuators as an
open question. All six counters are retained in the SDK and rotation capture.
