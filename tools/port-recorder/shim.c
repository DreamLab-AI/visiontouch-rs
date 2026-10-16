#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <sys/ioctl.h>
#include <sys/syscall.h>
#include <unistd.h>

extern void *port_capture_begin(int fd, int operation, uint64_t arg, uintptr_t pointer);
extern void port_capture_end(void *pending, int64_t result, int error);

static ssize_t (*next_read)(int, void *, size_t);
static ssize_t (*next_write)(int, const void *, size_t);
static int (*next_ioctl)(int, unsigned long, ...);
static pthread_once_t resolve_once = PTHREAD_ONCE_INIT;
static _Thread_local unsigned int depth;

static void resolve(void) {
    next_read = dlsym(RTLD_NEXT, "read");
    next_write = dlsym(RTLD_NEXT, "write");
    next_ioctl = dlsym(RTLD_NEXT, "ioctl");
    if (!next_read || !next_write || !next_ioctl) {
        static const char message[] = "port-recorder: cannot resolve libc entry points\n";
        syscall(SYS_write, 2, message, sizeof(message) - 1);
        _exit(125);
    }
}

ssize_t read(int fd, void *buffer, size_t count) {
    if (depth) return syscall(SYS_read, fd, buffer, count);
    depth++;
    int saved = errno;
    pthread_once(&resolve_once, resolve);
    void *pending = port_capture_begin(fd, 0, count, (uintptr_t)buffer);
    errno = saved;
    ssize_t result = next_read(fd, buffer, count);
    saved = errno;
    port_capture_end(pending, result, saved);
    errno = saved;
    depth--;
    return result;
}

ssize_t write(int fd, const void *buffer, size_t count) {
    if (depth) return syscall(SYS_write, fd, buffer, count);
    depth++;
    int saved = errno;
    pthread_once(&resolve_once, resolve);
    void *pending = port_capture_begin(fd, 1, count, (uintptr_t)buffer);
    errno = saved;
    ssize_t result = next_write(fd, buffer, count);
    saved = errno;
    port_capture_end(pending, result, saved);
    errno = saved;
    depth--;
    return result;
}

// The x86-64 entry shim forwards the third argument register unchanged. This
// avoids va_arg on no-argument requests (including unknown requests) and avoids
// interpreting scalar arguments as pointers. Rust only reads known ppdev buffers.
int port_capture_ioctl(int fd, unsigned long request, uintptr_t argument) {
    if (depth) return syscall(SYS_ioctl, fd, request, argument);
    depth++;
    int saved = errno;
    pthread_once(&resolve_once, resolve);
    void *pending = port_capture_begin(fd, 2, request, argument);
    errno = saved;
    int result = next_ioctl(fd, request, argument);
    saved = errno;
    port_capture_end(pending, result, saved);
    errno = saved;
    depth--;
    return result;
}
