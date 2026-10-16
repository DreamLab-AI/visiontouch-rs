/* Linux x86-64 SysV ABI: forward rdi/rsi/rdx without reading C varargs. */
.text
.globl ioctl
.type ioctl, @function
ioctl:
    jmp port_capture_ioctl
.size ioctl, .-ioctl
.section .note.GNU-stack,"",@progbits
