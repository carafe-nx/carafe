#include <pthread.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <switch.h>

#include "carafe_bench.h"

#define HANDOFF_ROUNDS 2000
#define BUSY_ROUNDS 8
#define ATOMIC_ROUNDS 200000
#define UNCACHED_BYTES (256 * 1024)
#define LINE_BYTES 64
#define BRANCH_STEPS 200000
#define WORKER_PRIORITY 58
#define GAME_PRIORITY 59
#define HOME_CORE 2

Result __real_svcSetThreadCoreMask(Handle handle, s32 core, u32 mask);

typedef struct {
    unsigned int core;
    s32 priority;
    bool sleep;
    volatile u32* word;
    u32 rounds;
    volatile bool ready;
} Partner;

typedef struct {
    unsigned int core;
    volatile bool* stop;
    volatile bool ready;
} Spinner;

typedef struct {
    unsigned int core;
    volatile u64* counter;
    u32 rounds;
    volatile bool* go;
    volatile bool ready;
} Adder;

static volatile u32 g_word __attribute__((aligned(LINE_BYTES)));
static volatile u64 g_counter __attribute__((aligned(LINE_BYTES)));

static double cycles(u64 ticks, unsigned int mhz, u64 count) {
    return count == 0 ? 0 : (double)armTicksToNs(ticks) * mhz / 1000.0 / (double)count;
}

static bool pinTo(unsigned int core) {
    if (R_FAILED(__real_svcSetThreadCoreMask(CUR_THREAD_HANDLE, (s32)core, 1u << core))) {
        return false;
    }
    for (int tries = 0; tries < 8 && svcGetCurrentProcessorNumber() != core; tries++) {
        svcSleepThread(0);
    }
    return svcGetCurrentProcessorNumber() == core;
}

static void waitFor(volatile u32* word, u32 value, bool sleep) {
    while (__atomic_load_n(word, __ATOMIC_ACQUIRE) != value) {
        if (sleep) {
            svcWaitForAddress((void*)word, ArbitrationType_WaitIfEqual, (s64)(value - 1), -1);
        }
    }
}

static void post(volatile u32* word, u32 value, bool sleep) {
    __atomic_store_n(word, value, __ATOMIC_RELEASE);
    if (sleep) {
        svcSignalToAddress((void*)word, SignalType_Signal, 0, 1);
    }
}

static void* partnerMain(void* arg) {
    Partner* partner = arg;
    pinTo(partner->core);
    svcSetThreadPriority(CUR_THREAD_HANDLE, partner->priority);
    partner->ready = true;
    for (u32 i = 0; i < partner->rounds; i++) {
        waitFor(partner->word, 2 * i + 1, partner->sleep);
        post(partner->word, 2 * i + 2, partner->sleep);
    }
    return NULL;
}

static void* spinnerMain(void* arg) {
    Spinner* spinner = arg;
    pinTo(spinner->core);
    svcSetThreadPriority(CUR_THREAD_HANDLE, GAME_PRIORITY);
    spinner->ready = true;
    while (!*spinner->stop) {
    }
    return NULL;
}

static double handoff(unsigned int core, bool sleep, s32 priority, u32 rounds, unsigned int mhz, bool busy) {
    volatile bool stop = false;
    Spinner spinner = {core, &stop, false};
    pthread_t spinning;
    if (busy && pthread_create(&spinning, NULL, spinnerMain, &spinner) != 0) {
        return 0;
    }
    g_word = 0;
    Partner partner = {core, priority, sleep, &g_word, rounds, false};
    pthread_t thread;
    double result = 0;
    if (pthread_create(&thread, NULL, partnerMain, &partner) == 0) {
        while (!partner.ready || (busy && !spinner.ready)) {
            svcSleepThread(1000000);
        }
        svcSleepThread(1000000);
        u64 start = armGetSystemTick();
        for (u32 i = 0; i < rounds; i++) {
            post(&g_word, 2 * i + 1, sleep);
            waitFor(&g_word, 2 * i + 2, sleep);
        }
        result = cycles(armGetSystemTick() - start, mhz, 2ull * rounds);
        pthread_join(thread, NULL);
    }
    if (busy) {
        stop = true;
        pthread_join(spinning, NULL);
    }
    return result;
}

static void* adderMain(void* arg) {
    Adder* adder = arg;
    pinTo(adder->core);
    svcSetThreadPriority(CUR_THREAD_HANDLE, WORKER_PRIORITY);
    adder->ready = true;
    while (!*adder->go) {
    }
    for (u32 i = 0; i < adder->rounds; i++) {
        __atomic_fetch_add(adder->counter, 1, __ATOMIC_ACQ_REL);
    }
    return NULL;
}

static double contended(int other, unsigned int mhz) {
    volatile bool go = false;
    g_counter = 0;
    Adder adder = {(unsigned int)(other < 0 ? HOME_CORE : other), &g_counter, ATOMIC_ROUNDS, &go, false};
    pthread_t thread;
    bool together = other >= 0;
    if (together && pthread_create(&thread, NULL, adderMain, &adder) != 0) {
        return 0;
    }
    while (together && !adder.ready) {
        svcSleepThread(1000000);
    }
    u64 start = armGetSystemTick();
    go = true;
    for (u32 i = 0; i < ATOMIC_ROUNDS; i++) {
        __atomic_fetch_add(&g_counter, 1, __ATOMIC_ACQ_REL);
    }
    u64 ticks = armGetSystemTick() - start;
    if (together) {
        pthread_join(thread, NULL);
    }
    return cycles(ticks, mhz, ATOMIC_ROUNDS);
}

void carafeBenchCores(unsigned int mhz, CarafeBenchTrace trace) {
    s32 home_core;
    u64 home_mask;
    s32 home_priority;
    if (R_FAILED(svcGetThreadCoreMask(&home_core, &home_mask, CUR_THREAD_HANDLE)) ||
        R_FAILED(svcGetThreadPriority(&home_priority, CUR_THREAD_HANDLE)) || !pinTo(HOME_CORE)) {
        return;
    }
    svcSetThreadPriority(CUR_THREAD_HANDLE, WORKER_PRIORITY);
    double spin_same = handoff(HOME_CORE, false, WORKER_PRIORITY, 200, mhz, false);
    double spin_0 = handoff(0, false, WORKER_PRIORITY, HANDOFF_ROUNDS, mhz, false);
    double spin_3 = handoff(3, false, WORKER_PRIORITY, HANDOFF_ROUNDS, mhz, false);
    double sleep_same = handoff(HOME_CORE, true, WORKER_PRIORITY, HANDOFF_ROUNDS, mhz, false);
    double sleep_0 = handoff(0, true, WORKER_PRIORITY, HANDOFF_ROUNDS, mhz, false);
    double sleep_3 = handoff(3, true, WORKER_PRIORITY, HANDOFF_ROUNDS, mhz, false);
    double busy_59 = handoff(0, true, GAME_PRIORITY, BUSY_ROUNDS, mhz, true);
    double busy_58 = handoff(0, true, WORKER_PRIORITY, BUSY_ROUNDS, mhz, true);
    double alone = contended(-1, mhz);
    double with_0 = contended(0, mhz);
    double with_3 = contended(3, mhz);
    __real_svcSetThreadCoreMask(CUR_THREAD_HANDLE, home_core, (u32)home_mask);
    svcSetThreadPriority(CUR_THREAD_HANDLE, home_priority);
    char text[512];
    snprintf(text, sizeof(text),
        "between cores at %u MHz, cycles one way: spinning on a line, core 2 to 2 %.0f, 2 to 0 %.0f, 2 to 3 %.0f; "
        "waking a sleeping thread, 2 to 2 %.0f, 2 to 0 %.0f, 2 to 3 %.0f; waking one on a core busy at 59, at 59 "
        "%.0f, at 58 %.0f; an atomic increment alone %.1f, with core 0 %.1f, with core 3 %.1f",
        mhz, spin_same, spin_0, spin_3, sleep_same, sleep_0, sleep_3, busy_59, busy_58, alone, with_0, with_3);
    trace("[A57] ", text);
}

static u32* lineRing(u8* buffer, size_t bytes) {
    size_t count = bytes / LINE_BYTES;
    u32* order = malloc(count * sizeof(u32));
    if (order == NULL) {
        return NULL;
    }
    for (size_t i = 0; i < count; i++) {
        order[i] = (u32)i;
    }
    u64 seed = 4242;
    for (size_t i = count - 1; i > 0; i--) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        size_t j = (size_t)((seed >> 33) % i);
        u32 t = order[i];
        order[i] = order[j];
        order[j] = t;
    }
    for (size_t i = 0; i < count; i++) {
        *(u32*)(buffer + (size_t)order[i] * LINE_BYTES) = order[(i + 1) % count] * LINE_BYTES;
    }
    return order;
}

typedef struct {
    double line_write;
    double word_write;
    double dependent_read;
    double stream_read;
} MemoryCosts;

static MemoryCosts memoryCosts(u8* buffer, size_t bytes, unsigned int mhz) {
    MemoryCosts costs = {0, 0, 0, 0};
    size_t lines = bytes / LINE_BYTES;
    u64 start = armGetSystemTick();
    for (size_t i = 0; i < lines; i++) {
        u64* line = (u64*)(buffer + i * LINE_BYTES);
        for (int w = 0; w < LINE_BYTES / 8; w++) {
            line[w] = i + (u64)w;
        }
    }
    __asm__ volatile("dsb sy" ::: "memory");
    costs.line_write = cycles(armGetSystemTick() - start, mhz, lines);
    u64 seed = 99;
    start = armGetSystemTick();
    for (size_t i = 0; i < lines; i++) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        *(volatile u32*)(buffer + ((seed >> 33) % (bytes / 4)) * 4) = (u32)i;
    }
    __asm__ volatile("dsb sy" ::: "memory");
    costs.word_write = cycles(armGetSystemTick() - start, mhz, lines);
    u32* order = lineRing(buffer, bytes);
    if (order != NULL) {
        __asm__ volatile("dsb sy" ::: "memory");
        u32 at = order[0] * LINE_BYTES;
        size_t steps = lines < 4096 ? lines : 4096;
        start = armGetSystemTick();
        for (size_t i = 0; i < steps; i++) {
            at = *(volatile u32*)(buffer + at);
        }
        costs.dependent_read = cycles(armGetSystemTick() - start, mhz, steps);
        __asm__ volatile("" : : "r"(at));
        free(order);
    }
    u64 sum = 0;
    start = armGetSystemTick();
    for (size_t i = 0; i < bytes / 8; i++) {
        sum += ((volatile u64*)buffer)[i];
    }
    costs.stream_read = cycles(armGetSystemTick() - start, mhz, lines);
    __asm__ volatile("" : : "r"(sum));
    return costs;
}

void carafeBenchMemory(unsigned int mhz, CarafeBenchTrace trace) {
    u8* buffer = aligned_alloc(0x1000, UNCACHED_BYTES);
    if (buffer == NULL) {
        return;
    }
    memset(buffer, 0, UNCACHED_BYTES);
    MemoryCosts cached = memoryCosts(buffer, UNCACHED_BYTES, mhz);
    memset(buffer, 0xa5, UNCACHED_BYTES);
    u64 start = armGetSystemTick();
    armDCacheClean(buffer, UNCACHED_BYTES);
    double clean = cycles(armGetSystemTick() - start, mhz, UNCACHED_BYTES / LINE_BYTES);
    armDCacheFlush(buffer, UNCACHED_BYTES);
    Result rc = svcSetMemoryAttribute(buffer, UNCACHED_BYTES, MemAttr_IsUncached, MemAttr_IsUncached);
    char text[512];
    if (R_FAILED(rc)) {
        snprintf(text, sizeof(text), "uncached memory unavailable: %#x; cleaning a dirty line %.1f cycles",
            (unsigned int)rc, clean);
        trace("[A57] ", text);
        free(buffer);
        return;
    }
    MemoryCosts uncached = memoryCosts(buffer, UNCACHED_BYTES, mhz);
    svcSetMemoryAttribute(buffer, UNCACHED_BYTES, MemAttr_IsUncached, 0);
    free(buffer);
    snprintf(text, sizeof(text),
        "memory as DXVK maps it at %u MHz, cycles cached / uncached: writing a whole line %.1f / %.1f, one word "
        "at random %.1f / %.1f, a dependent read %.1f / %.1f, reading a line in order %.1f / %.1f; cleaning a "
        "dirty line %.1f",
        mhz, cached.line_write, uncached.line_write, cached.word_write, uncached.word_write, cached.dependent_read,
        uncached.dependent_read, cached.stream_read, uncached.stream_read, clean);
    trace("[A57] ", text);
}

static double periodic(unsigned int period, unsigned int mhz) {
    static void* const targets[4] = {&&first, &&second, &&third, &&fourth};
    static const unsigned char pattern[8] = {0, 2, 1, 3, 3, 1, 0, 2};
    u64 sum = 0;
    u64 seed = 7;
    unsigned int step = 0;
    u64 start = armGetSystemTick();
    for (int i = 0; i < BRANCH_STEPS; i++) {
        unsigned int index;
        if (period == 0) {
            seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
            index = (unsigned int)(seed >> 33) & 3;
        } else {
            index = period == 8 ? pattern[step] : step & 3;
            step = step + 1 == period ? 0 : step + 1;
        }
        goto* targets[index];
    first:
        sum += 1;
        continue;
    second:
        sum += 3;
        continue;
    third:
        sum += 5;
        continue;
    fourth:
        sum += 7;
        continue;
    }
    u64 ticks = armGetSystemTick() - start;
    __asm__ volatile("" : : "r"(sum));
    return cycles(ticks, mhz, BRANCH_STEPS);
}

static double compareChain(unsigned int mhz) {
    u64 sum = 0;
    u64 seed = 7;
    u64 start = armGetSystemTick();
    for (int i = 0; i < BRANCH_STEPS; i++) {
        seed = seed * 6364136223846793005ULL + 1442695040888963407ULL;
        unsigned int index = (unsigned int)(seed >> 33) & 3;
        __asm__ volatile("" : "+r"(index));
        if (index == 0) {
            sum += 1;
        } else if (index == 1) {
            sum += 3;
        } else if (index == 2) {
            sum += 5;
        } else {
            sum += 7;
        }
    }
    u64 ticks = armGetSystemTick() - start;
    __asm__ volatile("" : : "r"(sum));
    return cycles(ticks, mhz, BRANCH_STEPS);
}

void carafeBenchBranches(unsigned int mhz, CarafeBenchTrace trace) {
    char text[256];
    snprintf(text, sizeof(text),
        "jump tables at %u MHz, cycles a step: an indirect branch cycling through 2 targets %.1f, 4 %.1f, "
        "8 entries over 4 targets %.1f, at random among 4 %.1f; a compare chain at random among 4 %.1f",
        mhz, periodic(2, mhz), periodic(4, mhz), periodic(8, mhz), periodic(0, mhz), compareChain(mhz));
    trace("[A57] ", text);
}
