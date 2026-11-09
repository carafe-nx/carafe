#include <errno.h>
#include <malloc.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/reent.h>
#include "native_heap.h"

#define MiB ((size_t)1048576)
static unsigned char area[256 * 1048576] __attribute__((aligned(2097152)));
char *fake_heap_start = (char *)area, *fake_heap_end = (char *)area + sizeof(area);
static char *heap_break = (char *)area;
static struct _reent reent;
static unsigned int locks;
static unsigned char tls_area[4096] __attribute__((aligned(16)));

void *__aarch64_read_tp(void) { return tls_area; }

static long linux_call(long number, long a, long b, long c)
{
    register long x8 __asm__("x8") = number;
    register long x0 __asm__("x0") = a;
    register long x1 __asm__("x1") = b;
    register long x2 __asm__("x2") = c;
    __asm__ volatile("svc 0" : "+r"(x0) : "r"(x1), "r"(x2), "r"(x8) : "memory");
    return x0;
}

#define check(condition) do { if (!(condition)) { \
    static const char message[] = "failed: " #condition "\n"; \
    linux_call(64, 2, (long)message, sizeof(message) - 1); \
    linux_call(93, 1, 0, 0); __builtin_unreachable(); } } while (0)

struct _reent *__getreent(void) { return &reent; }
void __malloc_lock(struct _reent *r) { (void)r; locks++; }
void __malloc_unlock(struct _reent *r) { (void)r; check(locks); locks--; }

void *_sbrk_r(struct _reent *r, ptrdiff_t increment)
{
    char *previous = heap_break;
    check(locks);
    if (increment > 0 && (size_t)increment > (size_t)(fake_heap_end - heap_break))
    { r->_errno = ENOMEM; return (void *)-1; }
    if (increment < 0) check((size_t)-increment <= (size_t)(heap_break - fake_heap_start));
    heap_break += increment;
    return previous;
}

int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attributes)
{ (void)attributes; memset(mutex, 0, sizeof(*mutex)); return 0; }
int pthread_mutex_destroy(pthread_mutex_t *mutex) { (void)mutex; return 0; }
int pthread_mutex_lock(pthread_mutex_t *mutex) { (void)mutex; return 0; }
int pthread_mutex_unlock(pthread_mutex_t *mutex) { (void)mutex; return 0; }

void __attribute__((noreturn)) _exit(int status)
{
    linux_call(93, status, 0, 0);
    __builtin_unreachable();
}

void __attribute__((noreturn)) abort(void) { _exit(2); }

void __attribute__((noreturn)) __assert_func(const char *file, int line, const char *function, const char *expression)
{
    (void)file; (void)line; (void)function;
    linux_call(64, 2, (long)expression, strlen(expression));
    _exit(3);
}

#define HORIZON_MEMFILE_ALLOC_PAGES(size) memalign(4096, size)
#define HORIZON_MEMFILE_FREE_PAGES(pointer, size) free(pointer)
#include "horizon_memfile.h"

static void run(void)
{
    void *small = malloc(12 * MiB), *barrier = malloc(12 * MiB), *large[128], *tail[32];
    size_t count = 0, tail_count = 0, i;
    struct wine_nx_native_heap_stats before, after;
    static const struct horizon_memfile_ops operations;
    check(small && barrier);
    while (count < 128 && (large[count] = memalign(2 * MiB, 2 * MiB))) count++;
    check(count > 8 && count < 128);
    while (tail_count < 32 && (tail[tail_count] = memalign(65536, 65536))) tail_count++;
    check(tail_count < 32);
    for (i = 0; i < 4; i++) free(large[i]);
    free(small);
    wine_nx_native_heap_stats(&before);
    check(before.free == 8 * MiB && before.gap < 65536 && before.small_free >= 11 * MiB);
    struct mallinfo original = mallinfo();
    static const char state[] = "section=16MiB backing_free=8MiB small_free>=11MiB gap<64KiB\n";
    linux_call(64, 1, (long)state, sizeof(state) - 1);
    struct horizon_memfile *file = horizon_memfile_alloc(16 * MiB, 0, &operations);
    check(file != NULL);
    const char bytes[8] = "carafe";
    char result[8];
    for (i = 0; i < 16 * MiB - sizeof(bytes); i += 32749)
    {
        check(horizon_memfile_pwrite(file, bytes, sizeof(bytes), i) == sizeof(bytes));
        check(horizon_memfile_pread(file, result, sizeof(result), i) == sizeof(result));
        check(!memcmp(result, bytes, sizeof(bytes)));
    }
    horizon_memfile_unref(file);
    wine_nx_native_heap_stats(&after);
    check(after.used == before.used && after.free == before.free);
    check(mallinfo().uordblks == original.uordblks);
    check(!horizon_memfile_alloc(32 * MiB, 0, &operations));
    wine_nx_native_heap_stats(&after);
    check(after.used == before.used && after.free == before.free);
    check(mallinfo().uordblks == original.uordblks);
    for (i = 4; i < count; i++) free(large[i]);
    for (i = 0; i < tail_count; i++) free(tail[i]);
    free(barrier);
    wine_nx_native_heap_stats(&after);
    check(!after.used && !after.reserved && after.complete && !locks);
    static const char passed[] = "native section: one section uses both real allocators, copies and rollback passed\n";
    linux_call(64, 1, (long)passed, sizeof(passed) - 1);
}

void _start(void)
{
    __asm__ volatile("msr tpidr_el0, %0" : : "r"(tls_area) : "memory");
    run();
    _exit(0);
}
