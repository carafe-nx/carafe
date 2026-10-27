#include <switch.h>

#include <stdio.h>
#include <string.h>

#include "../overlay/carafe_settings.h"
#include "orchestrator_policy.h"

struct wine_nx_thread_view {
    unsigned int tid;
    char kind;
    char name[32];
    unsigned int permille;
    u64 mask;
    int movable;
};

unsigned int wine_nx_thread_views(struct wine_nx_thread_view* views, unsigned int max);
int wine_nx_thread_place(unsigned int tid, s32 core, u32 mask);
void wine_nx_thread_reserve_core(s32 core);
void wine_nx_runtime_trace(const char* msg) __attribute__((weak));
void __real_wine_nx_thread_balance(void);

static int enabled = -1;
static OrchestratorState state;
static struct wine_nx_thread_view views[ORCHESTRATOR_MAX_THREADS];
static OrchestratorThread threads[ORCHESTRATOR_MAX_THREADS];

static void trace(const char* line) {
    if (wine_nx_runtime_trace) {
        wine_nx_runtime_trace(line);
    }
}

static bool orchestrating(void) {
    if (enabled < 0) {
        enabled = carafeOrchestrating() ? 1 : 0;
        if (enabled) {
            orchestratorStart(&state);
            trace("[ORCHESTRATOR] on: places threads by their load, the busiest one alone on its core");
        }
    }
    return enabled == 1;
}

static const char* nameOf(unsigned int tid, unsigned int count) {
    for (unsigned int i = 0; i < count; i++) {
        if (views[i].kind == 'w' && views[i].tid == tid) {
            return views[i].name;
        }
    }
    return "";
}

static void traceMode(const OrchestratorPlan* plan, unsigned int count) {
    char line[256];
    if (plan->mode == OrchestratorModeSingle) {
        const char* name = nameOf(plan->mainTid, count);
        snprintf(line, sizeof(line),
            "[ORCHESTRATOR] %s: %uw%s%s on core %d alone (%u.%u%% of a core, next %u.%u%%); "
            "shader workers kept off it",
            orchestratorModeName(plan->mode), plan->mainTid, name[0] ? ":" : "", name, plan->mainCore,
            plan->mainPermille / 10, plan->mainPermille % 10, plan->secondPermille / 10, plan->secondPermille % 10);
    } else {
        snprintf(line, sizeof(line),
            "[ORCHESTRATOR] %s: busiest %u.%u%%, next %u.%u%%; the runtime's balancer places threads",
            orchestratorModeName(plan->mode), plan->mainPermille / 10, plan->mainPermille % 10,
            plan->secondPermille / 10, plan->secondPermille % 10);
    }
    trace(line);
}

void __wrap_wine_nx_thread_balance(void) {
    u64 processMask;
    if (!orchestrating() || R_FAILED(svcGetInfo(&processMask, InfoType_CoreMask, CUR_PROCESS_HANDLE, 0))) {
        __real_wine_nx_thread_balance();
        return;
    }
    unsigned int count = wine_nx_thread_views(views, ORCHESTRATOR_MAX_THREADS);
    if (count == 0) {
        __real_wine_nx_thread_balance();
        return;
    }
    for (unsigned int i = 0; i < count; i++) {
        threads[i] = (OrchestratorThread){views[i].tid, views[i].kind, views[i].permille, (unsigned int)views[i].mask,
            views[i].movable != 0};
    }
    OrchestratorPlan plan;
    orchestratorDecide(threads, count, (unsigned int)(processMask & 7), &state, &plan);
    if (plan.modeChanged) {
        wine_nx_thread_reserve_core(plan.mode == OrchestratorModeSingle ? plan.mainCore : -1);
        traceMode(&plan, count);
    }
    if (plan.mode != OrchestratorModeSingle) {
        __real_wine_nx_thread_balance();
        return;
    }
    for (size_t i = 0; i < plan.moveCount; i++) {
        const OrchestratorMove* move = &plan.moves[i];
        const char* name = nameOf(move->tid, count);
        char line[160];
        bool placed = wine_nx_thread_place(move->tid, move->core, move->mask) != 0;
        snprintf(line, sizeof(line), "[ORCHESTRATOR] %uw%s%s -> cores %#x, core %d first%s", move->tid,
            name[0] ? ":" : "", name, move->mask, move->core, placed ? "" : ": refused");
        trace(line);
    }
}
