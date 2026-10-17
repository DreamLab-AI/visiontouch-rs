# Host capability query

This specifies host API use only; the peripheral protocol remains unknown.
Source: P-0010. Initial supported ABI: Linux x86-64.

Open the selected character device read/write, query PPGETMODES into a four-byte
unsigned integer, then close. Do not claim, negotiate, change mode or direction,
read/write data, or read control/status registers. The Linux generic ioctl
encoding is direction 2, size 4, type 0x70, command 0x97 (0x80047097).

Decode capability bits from public linux/parport.h: PCSPP=1, TRISTATE=2,
EPP=4, ECP=8, COMPAT=16, DMA=32, SAFEININT=64. Preserve the raw mask and unknown
bits. These are host capabilities; a disconnected cable yields the same result.
Never label this as successful unit identification or a working EPP transaction.
Fail on an absent/non-character node, permission error, or unsuccessful ioctl.
No automatic sudo or fallback writes. Use scripts/with-port-access to refresh groups.
