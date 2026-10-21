#include <pthread.h>
#include <stdint.h>
#include <stdio.h>

#include <switch.h>

#define NATIVE_KINDS 64

void __real_wine_nx_threads_report_stalled(void);
int __real_pthread_create(pthread_t* thread, const pthread_attr_t* attr, void* (*start)(void*), void* arg);
void carafe_diag_dump(void) __attribute__((weak));
void carafe_alert_dump(void) __attribute__((weak));
void wine_nx_runtime_trace(const char* msg) __attribute__((weak));

static uintptr_t g_native_starts[NATIVE_KINDS];
static unsigned int g_native_counts[NATIVE_KINDS];

void __wrap_wine_nx_threads_report_stalled(void) {
    __real_wine_nx_threads_report_stalled();
    if (carafe_diag_dump) {
        carafe_diag_dump();
    }
    if (carafe_alert_dump) {
        carafe_alert_dump();
    }
}

static uintptr_t runtimeBase(void) {
    static uintptr_t base;
    MemoryInfo info;
    u32 page = 0;
    if (!base && R_SUCCEEDED(svcQueryMemory(&info, &page, (u64)(uintptr_t)&runtimeBase))) {
        base = info.addr;
    }
    return base;
}

static void noteNativeThread(uintptr_t start, uintptr_t caller) {
    unsigned int slot = NATIVE_KINDS;
    for (unsigned int i = 0; i < NATIVE_KINDS; i++) {
        uintptr_t seen = __atomic_load_n(&g_native_starts[i], __ATOMIC_ACQUIRE);
        if (seen == start) {
            slot = i;
            break;
        }
        if (seen == 0) {
            uintptr_t expected = 0;
            if (__atomic_compare_exchange_n(&g_native_starts[i], &expected, start, false, __ATOMIC_ACQ_REL,
                    __ATOMIC_ACQUIRE) || expected == start) {
                slot = i;
                break;
            }
        }
    }
    if (slot == NATIVE_KINDS || wine_nx_runtime_trace == NULL) {
        return;
    }
    unsigned int count = __atomic_add_fetch(&g_native_counts[slot], 1, __ATOMIC_RELAXED);
    if (count & (count - 1)) {
        return;
    }
    uintptr_t base = runtimeBase();
    char line[160];
    snprintf(
        line,
        sizeof(line),
        "[NATIVE] threads started at runtime+0x%lx, from runtime+0x%lx: %u",
        (unsigned long)(start - base),
        (unsigned long)(caller - base),
        count
    );
    wine_nx_runtime_trace(line);
}

int __wrap_pthread_create(pthread_t* thread, const pthread_attr_t* attr, void* (*start)(void*), void* arg) {
    noteNativeThread((uintptr_t)start, (uintptr_t)__builtin_return_address(0));
    return __real_pthread_create(thread, attr, start, arg);
}
