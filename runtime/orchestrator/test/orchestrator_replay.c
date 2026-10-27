#include "../orchestrator_policy.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static size_t parseThreads(const char* line, OrchestratorThread* threads) {
    const char* p = strstr(line, "cores:");
    size_t count = 0;
    if (p == NULL) {
        return 0;
    }
    p += strlen("cores:");
    while (*p != '\0' && count < ORCHESTRATOR_MAX_THREADS) {
        while (*p == ' ') {
            p++;
        }
        char* end;
        unsigned long tid = strtoul(p, &end, 10);
        if (end == p) {
            break;
        }
        char kind = *end;
        const char* at = strchr(end, '@');
        if (at == NULL) {
            break;
        }
        long core = strtol(at + 1, &end, 10);
        double percent = strtod(end, &end);
        unsigned int mask = core >= 0 ? 1u << core : 7u;
        threads[count++] = (OrchestratorThread){(unsigned int)tid, kind, (unsigned int)(percent * 10 + 0.5), mask,
            kind == 'w' && core != 3};
        p = end;
        while (*p == '%') {
            p++;
        }
    }
    return count;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s autorun_runtime.log\n", argv[0]);
        return 2;
    }
    FILE* file = fopen(argv[1], "r");
    if (file == NULL) {
        perror(argv[1]);
        return 2;
    }
    OrchestratorState state;
    orchestratorStart(&state);
    OrchestratorMove placed[ORCHESTRATOR_MAX_THREADS];
    size_t placedCount = 0;
    char line[8192];
    unsigned int number = 0, reports = 0, single = 0;
    while (fgets(line, sizeof(line), file) != NULL) {
        number++;
        if (strncmp(line, "[THREADS]", 9) != 0) {
            continue;
        }
        OrchestratorThread threads[ORCHESTRATOR_MAX_THREADS];
        size_t count = parseThreads(line, threads);
        for (size_t i = 0; i < count; i++) {
            for (size_t j = 0; j < placedCount; j++) {
                if (threads[i].kind == 'w' && threads[i].tid == placed[j].tid) {
                    threads[i].mask = placed[j].mask;
                }
            }
        }
        OrchestratorPlan plan;
        orchestratorDecide(threads, count, 7, &state, &plan);
        reports++;
        if (plan.mode == OrchestratorModeSingle) {
            single++;
        }
        if (plan.modeChanged) {
            printf("%u: %s, main %u on core %d, busiest %u.%u%%, next %u.%u%%\n", number,
                orchestratorModeName(plan.mode), plan.mainTid, plan.mainCore, plan.mainPermille / 10,
                plan.mainPermille % 10, plan.secondPermille / 10, plan.secondPermille % 10);
        }
        if (plan.modeChanged && plan.mode == OrchestratorModeWatch) {
            placedCount = 0;
        }
        for (size_t i = 0; i < plan.moveCount; i++) {
            printf("%u:   move %u to core %d, mask %#x\n", number, plan.moves[i].tid, plan.moves[i].core,
                plan.moves[i].mask);
            size_t j = 0;
            while (j < placedCount && placed[j].tid != plan.moves[i].tid) {
                j++;
            }
            if (j == placedCount && placedCount < ORCHESTRATOR_MAX_THREADS) {
                placedCount++;
            }
            placed[j] = plan.moves[i];
        }
    }
    fclose(file);
    printf("%u reports, %u with one dominant thread\n", reports, single);
    return 0;
}
