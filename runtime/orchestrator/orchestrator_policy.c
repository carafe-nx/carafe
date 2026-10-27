#include "orchestrator_policy.h"

#include <string.h>

#define ORCHESTRATOR_CORES 32

static unsigned int clientLoad(const OrchestratorThread* threads, size_t count, size_t index) {
    unsigned int load = threads[index].permille;
    for (size_t i = 0; i < count; i++) {
        if (i != index && threads[i].kind == 's' && threads[i].tid == threads[index].tid) {
            load += threads[i].permille;
        }
    }
    return load;
}

static int singleCore(unsigned int mask) {
    return mask != 0 && (mask & (mask - 1)) == 0 ? __builtin_ctz(mask) : -1;
}

static int quietestCore(const OrchestratorThread* threads, size_t count, unsigned int cores, unsigned int skipTid) {
    unsigned int loads[ORCHESTRATOR_CORES] = {0};
    for (size_t i = 0; i < count; i++) {
        int core = singleCore(threads[i].mask);
        if (threads[i].kind != 'w' || threads[i].tid == skipTid || core < 0 || core >= ORCHESTRATOR_CORES) {
            continue;
        }
        loads[core] += clientLoad(threads, count, i);
    }
    int best = -1;
    for (int core = 0; core < ORCHESTRATOR_CORES; core++) {
        if ((cores & (1u << core)) != 0 && (best < 0 || loads[core] < loads[best])) {
            best = core;
        }
    }
    return best;
}

static bool findThread(const OrchestratorThread* threads, size_t count, unsigned int tid, size_t* index) {
    for (size_t i = 0; i < count; i++) {
        if (threads[i].kind == 'w' && threads[i].tid == tid) {
            *index = i;
            return true;
        }
    }
    return false;
}

static void busiest(const OrchestratorThread* threads, size_t count, unsigned int skipTid, size_t* index,
    unsigned int* load) {
    *load = 0;
    *index = count;
    for (size_t i = 0; i < count; i++) {
        if (threads[i].kind != 'w' || threads[i].tid == skipTid) {
            continue;
        }
        unsigned int l = clientLoad(threads, count, i);
        if (*index == count || l > *load) {
            *index = i;
            *load = l;
        }
    }
}

static int mainCoreFor(const OrchestratorThread* threads, size_t count, size_t index, unsigned int appMask) {
    int core = singleCore(threads[index].mask & appMask);
    if (core >= 0 && (threads[index].mask == (1u << core) || !threads[index].movable)) {
        return core;
    }
    return quietestCore(threads, count, appMask, threads[index].tid);
}

static void addMove(OrchestratorPlan* plan, unsigned int tid, int core, unsigned int mask) {
    if (plan->moveCount < ORCHESTRATOR_MAX_THREADS) {
        plan->moves[plan->moveCount++] = (OrchestratorMove){tid, core, mask};
    }
}

static void leave(OrchestratorState* state, OrchestratorPlan* plan) {
    orchestratorStart(state);
    plan->modeChanged = true;
}

void orchestratorStart(OrchestratorState* state) {
    memset(state, 0, sizeof(*state));
    state->mode = OrchestratorModeWatch;
    state->mainCore = -1;
}

void orchestratorDecide(const OrchestratorThread* threads, size_t count, unsigned int appMask,
    OrchestratorState* state, OrchestratorPlan* plan) {
    memset(plan, 0, sizeof(*plan));
    plan->mainCore = -1;

    size_t top;
    unsigned int topLoad;
    busiest(threads, count, 0, &top, &topLoad);
    size_t second;
    unsigned int secondLoad = 0;
    if (top < count) {
        busiest(threads, count, threads[top].tid, &second, &secondLoad);
    }

    if (state->mode == OrchestratorModeWatch) {
        bool placeable = top < count &&
            (threads[top].movable || singleCore(threads[top].mask & appMask) >= 0);
        bool heavy = placeable && topLoad >= ORCHESTRATOR_DOMINANT_PERMILLE &&
            secondLoad < ORCHESTRATOR_SECOND_PERMILLE;
        if (!heavy) {
            state->candidateTid = 0;
            state->heavyPeriods = 0;
        } else if (state->candidateTid == threads[top].tid) {
            state->heavyPeriods++;
        } else {
            state->candidateTid = threads[top].tid;
            state->heavyPeriods = 1;
        }
        if (state->heavyPeriods >= ORCHESTRATOR_ENTER_PERIODS) {
            int core = mainCoreFor(threads, count, top, appMask);
            if (core >= 0) {
                state->mode = OrchestratorModeSingle;
                state->mainTid = threads[top].tid;
                state->mainCore = core;
                state->lightPeriods = 0;
                plan->modeChanged = true;
            }
        }
    } else {
        size_t main;
        if (!findThread(threads, count, state->mainTid, &main) || (appMask & (1u << state->mainCore)) == 0) {
            leave(state, plan);
        } else {
            unsigned int mainLoad = clientLoad(threads, count, main);
            size_t other;
            unsigned int otherLoad;
            busiest(threads, count, state->mainTid, &other, &otherLoad);
            bool light = mainLoad < ORCHESTRATOR_RELEASE_PERMILLE || otherLoad >= ORCHESTRATOR_SECOND_PERMILLE;
            state->lightPeriods = light ? state->lightPeriods + 1 : 0;
            if (state->lightPeriods >= ORCHESTRATOR_LEAVE_PERIODS) {
                leave(state, plan);
            }
        }
    }

    plan->mode = state->mode;
    plan->mainTid = state->mainTid;
    plan->mainCore = state->mainCore;
    plan->mainPermille = topLoad;
    plan->secondPermille = secondLoad;
    if (state->mode != OrchestratorModeSingle) {
        return;
    }

    unsigned int mainMask = 1u << state->mainCore;
    unsigned int rest = appMask & ~mainMask;
    for (size_t i = 0; i < count; i++) {
        const OrchestratorThread* thread = &threads[i];
        if (thread->kind != 'w' || !thread->movable) {
            continue;
        }
        if (thread->tid == state->mainTid) {
            if (thread->mask != mainMask) {
                addMove(plan, thread->tid, state->mainCore, mainMask);
            }
        } else if (rest != 0 && (thread->mask & mainMask) != 0) {
            addMove(plan, thread->tid, quietestCore(threads, count, rest, state->mainTid), rest);
        }
    }
}

const char* orchestratorModeName(OrchestratorMode mode) {
    return mode == OrchestratorModeSingle ? "one dominant thread" : "watching";
}
