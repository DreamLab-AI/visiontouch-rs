# Device identification

Fill this in from physical inspection (source class OWNER in the provenance log).

| Item | Answer |
|---|---|
| Exact base-plate / pedestal label text, model and serial, date code | |
| Is "PRO" silkscreened as part of the model name, or a separate sticker? (photo) | |
| PC-end connector: DB25 male straight into the port? Any box in between? | |
| Device-end connector type | |
| Separate amplifier / power box? What is printed on its PSU (V, A)? | |
| Degrees of freedom sensed: XYZ only, or gimbal orientation too? | |
| Any surviving driver media or install directory listing (`.sys`, `.vxd`, `.dll`, `.inf` names)? | |
| Which GHOST / PDD version, if any documentation survives | |
| PCIe parallel card: chipset (`lspci -nn`), base address (`/proc/sys/dev/parport/parport0/base-addr`), modes (`/proc/sys/dev/parport/parport0/modes`) | |

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
