#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/ppdev.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

static void *thread_write(void *path) {
    int fd = open(path, O_WRONLY | O_APPEND);
    assert(fd >= 0);
    assert(write(fd, "xy", 2) == 2);
    close(fd);
    return NULL;
}

int main(int argc, char **argv) {
    assert(argc == 2);
    int fd = open(argv[1], O_RDWR);
    assert(fd >= 0);
    const unsigned char bytes[] = {0, 0xff, 0x22, 0x5c, 0x0a};
    errno = EBUSY;
    assert(write(fd, bytes, sizeof(bytes)) == sizeof(bytes));
    assert(errno == EBUSY);
    assert(lseek(fd, 0, SEEK_SET) == 0);
    unsigned char output[16];
    assert(read(fd, output, sizeof(output)) == sizeof(bytes));
    assert(memcmp(bytes, output, sizeof(bytes)) == 0);
    assert(read(fd, output, sizeof(output)) == 0);
    int mode = 0x2040;
    assert(ioctl(fd, PPSETMODE, &mode) == -1 && errno == ENOTTY);
    struct ppdev_frob_struct control = {0x0f, 0x04};
    assert(ioctl(fd, PPFCONTROL, &control) == -1 && errno == ENOTTY);
    assert(ioctl(fd, PPCLAIM) == -1 && errno == ENOTTY);
    assert(ioctl(fd, FIOCLEX) == 0); /* unknown, no third argument */
    assert(ioctl(fd, PPSETMODE, (void *)(uintptr_t)1) == -1 && errno == ENOTTY);
    void *volatile invalid = (void *)(uintptr_t)1;
    assert(write(fd, invalid, 4) == -1 && errno == EFAULT);
    unsigned char *large = malloc(65537);
    assert(large);
    memset(large, 0xa5, 65537);
    assert(write(fd, large, 65537) == 65537);
    free(large);
    close(fd);
    fd = open(argv[1], O_RDONLY);
    assert(fd >= 0);
    assert(write(fd, bytes, sizeof(bytes)) == -1 && errno == EBADF);
    close(fd);
    pthread_t threads[2];
    for (int i = 0; i < 2; ++i) assert(pthread_create(&threads[i], NULL, thread_write, argv[1]) == 0);
    for (int i = 0; i < 2; ++i) assert(pthread_join(threads[i], NULL) == 0);
    return 0;
}
