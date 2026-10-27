/*
 * c_library_contract.c -- what nife's C library promises for the pieces fio 3.40 needed
 * (milestone 833 (the same storage benchmark on nife and Linux, by porting real fio)).
 *
 * Written for the purpose, so it fetches nothing. helpers/build-c-library-contract.sh links it
 * with the C library; system_tests/src/user/c_program_tests.rs runs it on all three ISAs and
 * reads its transcript. Each check prints one line, `ok <name>` or `FAIL <name>: <why>`, and the
 * last line is `contract: <n> failed`. The test fails on any FAIL line and on a missing last line.
 *
 * The first include is stdlib.h on purpose: before 2026-10-12 (UTC) that header did not include
 * features.h, and a file that began with it failed to compile.
 *
 * Name: provisional, lane lane/fio-c-library-pieces, 2026-10-12 (UTC), for the file and the
 * program. Milestone 869 (the C library's headers cannot drift, and its contract is tested in nife)
 * is where contract programs like this one are meant to live and run in CI.
 */
#include <stdlib.h>

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <netdb.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/param.h>
#include <sys/resource.h>
#include <sys/select.h>
#include <sys/shm.h>
#include <syslog.h>
#include <time.h>
#include <unistd.h>

static int failed;

static void check(int good, const char *name, const char *why) {
    if (good) {
        printf("ok %s\n", name);
    } else {
        printf("FAIL %s: %s\n", name, why);
        failed++;
    }
}

static long long now_us(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (long long)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}

/* basename and dirname change their argument, so each case gets its own copy. */
static int base_is(const char *path, const char *want) {
    char buf[64];
    strcpy(buf, path);
    return strcmp(basename(buf), want) == 0;
}

static int dir_is(const char *path, const char *want) {
    char buf[64];
    strcpy(buf, path);
    return strcmp(dirname(buf), want) == 0;
}

static void check_libgen(void) {
    check(base_is("/usr/lib/", "lib") && base_is("usr", "usr") && base_is("/", "/")
              && base_is("", "."),
          "basename", "a final component was wrong");
    check(dir_is("/usr/lib", "/usr") && dir_is("usr", ".") && dir_is("/", "/")
              && dir_is("/usr/", "/"),
          "dirname", "a parent was wrong");
}

static void check_rusage(void) {
    struct rusage ru;
    memset(&ru, 0xa5, sizeof ru);
    int self = getrusage(RUSAGE_SELF, &ru);
    /* nife accounts no CPU time to a process (c_library/README.md BUGS), so it is zero, not
     * elapsed time passed off as CPU time. */
    check(self == 0 && ru.ru_utime.tv_sec == 0 && ru.ru_utime.tv_usec == 0
              && ru.ru_stime.tv_sec == 0 && ru.ru_maxrss == 0 && ru.ru_nvcsw == 0,
          "getrusage(RUSAGE_SELF)", "did not answer zeros");
    check(getrusage(RUSAGE_THREAD, &ru) == 0 && getrusage(RUSAGE_CHILDREN, &ru) == 0,
          "getrusage(RUSAGE_THREAD, RUSAGE_CHILDREN)", "refused");
    errno = 0;
    check(getrusage(42, &ru) == -1 && errno == EINVAL, "getrusage(42)", "was not EINVAL");
    errno = 0;
    check(getpriority(PRIO_PROCESS, 0) == 0 && errno == 0 && nice(0) == 0,
          "getpriority and nice", "nice value was not 0");
}

/* fio's helper thread waits in select on a pipe its main thread writes. */
static int pipe_fds[2];

static void *write_later(void *arg) {
    (void)arg;
    struct timespec t = {0, 20 * 1000 * 1000};
    nanosleep(&t, NULL);
    write(pipe_fds[1], "x", 1);
    return NULL;
}

static void check_select(void) {
    if (pipe(pipe_fds) != 0) {
        check(0, "select", "pipe failed");
        return;
    }
    fd_set r, w;

    /* Nothing to read: a 500 us timeout must wait it out and report nothing. Rounded to poll's
     * milliseconds it would not wait at all. */
    FD_ZERO(&r);
    FD_SET(pipe_fds[0], &r);
    struct timeval tv = {0, 500};
    long long t0 = now_us();
    int n = select(pipe_fds[0] + 1, &r, NULL, NULL, &tv);
    long long waited = now_us() - t0;
    check(n == 0 && !FD_ISSET(pipe_fds[0], &r) && waited >= 500, "select times out",
          "returned early or reported a descriptor");

    /* The write end is writable at once. */
    FD_ZERO(&w);
    FD_SET(pipe_fds[1], &w);
    struct timeval zero = {0, 0};
    n = select(pipe_fds[1] + 1, NULL, &w, NULL, &zero);
    check(n == 1 && FD_ISSET(pipe_fds[1], &w), "select sees a writable pipe", "it did not");

    /* Another thread's write wakes a select with no timeout. */
    pthread_t t;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setstacksize(&attr, 64 * 1024);
    if (pthread_create(&t, &attr, write_later, NULL) != 0) {
        check(0, "select is woken by another thread", "pthread_create failed");
    } else {
        FD_ZERO(&r);
        FD_SET(pipe_fds[0], &r);
        n = select(pipe_fds[0] + 1, &r, NULL, NULL, NULL);
        pthread_join(t, NULL);
        check(n == 1 && FD_ISSET(pipe_fds[0], &r), "select is woken by another thread",
              "it was not");
    }

    /* A descriptor that is not open is EBADF, and too many is EINVAL. */
    FD_ZERO(&r);
    FD_SET(200, &r);
    errno = 0;
    check(select(201, &r, NULL, NULL, &zero) == -1 && errno == EBADF, "select on a closed fd",
          "was not EBADF");
    errno = 0;
    check(select(FD_SETSIZE + 1, NULL, NULL, NULL, &zero) == -1 && errno == EINVAL,
          "select past FD_SETSIZE", "was not EINVAL");
    close(pipe_fds[0]);
    close(pipe_fds[1]);
}

static void check_sync_write(void) {
    const char *path = "contract-osync";
    int fd = open(path, O_CREAT | O_TRUNC | O_WRONLY | O_SYNC, 0600);
    int wrote = fd >= 0 ? (int)write(fd, "durable", 7) : -1;
    if (fd >= 0) close(fd);
    char back[8] = {0};
    fd = open(path, O_RDONLY);
    int got = fd >= 0 ? (int)read(fd, back, 7) : -1;
    if (fd >= 0) close(fd);
    unlink(path);
    check(O_SYNC != 0 && O_DSYNC != 0 && wrote == 7 && got == 7 && strcmp(back, "durable") == 0,
          "O_SYNC write", "the write did not read back");
    check(POSIX_MADV_DONTNEED == 4, "POSIX_MADV_DONTNEED", "is not Linux's 4");
}

static void check_refusals(void) {
    check(dlopen("engine.so", RTLD_LAZY) == NULL && dlerror() != NULL && dlerror() == NULL,
          "dlopen refuses, dlerror says why once", "it did not");
    errno = 0;
    check(shmget(IPC_PRIVATE, 4096, IPC_CREAT | 0600) == -1 && errno == ENOSYS,
          "shmget is ENOSYS", "it was not");
    struct addrinfo *res = (struct addrinfo *)1;
    check(getaddrinfo("localhost", "8765", NULL, &res) == EAI_SYSTEM && res == NULL
              && errno == ENOSYS && gai_strerror(EAI_SYSTEM) != NULL,
          "getaddrinfo is EAI_SYSTEM", "it was not");
    /* LOG_PERROR copies to standard error, which the test does not read; it must not trap. */
    openlog("contract", LOG_PERROR, LOG_USER);
    syslog(LOG_INFO, "syslog reaches standard error (%d)", 42);
    closelog();
    check(1, "syslog with LOG_PERROR", "");
}

int main(void) {
    check_libgen();
    check_rusage();
    check_select();
    check_sync_write();
    check_refusals();
    printf("contract: %d failed\n", failed);
    return failed != 0;
}
