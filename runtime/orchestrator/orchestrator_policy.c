#include "orchestrator_policy.h"

#include <string.h>

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

static bool eligible(const OrchestratorThread* thread, unsigned int appMask) {
    return thread->kind == 'w' && (thread->mask == 0 || (thread->mask & appMask) != 0);
}

static bool placeable(const OrchestratorThread* thread, unsigned int appMask) {
    return thread->movable || singleCore(thread->mask & appMask) >= 0;
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

static void busiest(const OrchestratorThread* threads, size_t count, unsigned int skipTid, unsigned int appMask,
    size_t* index, unsigned int* load) {
    *load = 0;
    *index = count;
    for (size_t i = 0; i < count; i++) {
        if (!eligible(&threads[i], appMask) || threads[i].tid == skipTid) {
            continue;
        }
        unsigned int l = clientLoad(threads, count, i);
        if (*index == count || l > *load) {
            *index = i;
            *load = l;
        }
    }
}

static size_t heavyThreads(const OrchestratorThread* threads, size_t count, unsigned int appMask, size_t* heavy,
    unsigned int* loads) {
    size_t found = 0;
    for (size_t i = 0; i < count && found < ORCHESTRATOR_MAX_THREADS; i++) {
        if (!eligible(&threads[i], appMask)) {
            continue;
        }
        unsigned int load = clientLoad(threads, count, i);
        if (load < ORCHESTRATOR_HEAVY_PERMILLE) {
            continue;
        }
        size_t at = found++;
        while (at > 0 && loads[at - 1] < load) {
            heavy[at] = heavy[at - 1];
            loads[at] = loads[at - 1];
            at--;
        }
        heavy[at] = i;
        loads[at] = load;
    }
    return found;
}

static unsigned int coreCount(unsigned int mask) {
    return (unsigned int)__builtin_popcount(mask);
}

static unsigned int coreLoad(const OrchestratorThread* threads, size_t count, int core, unsigned int skipTid) {
    unsigned int load = 0;
    for (size_t i = 0; i < count; i++) {
        if (threads[i].kind == 'w' && threads[i].tid != skipTid && singleCore(threads[i].mask) == core) {
            load += clientLoad(threads, count, i);
        }
    }
    return load;
}

static int quietestCore(const OrchestratorThread* threads, size_t count, unsigned int cores, unsigned int skipTid) {
    int best = -1;
    unsigned int bestLoad = 0;
    for (int core = 0; core < ORCHESTRATOR_MAX_CORES; core++) {
        if ((cores & (1u << core)) == 0) {
            continue;
        }
        unsigned int load = coreLoad(threads, count, core, skipTid);
        if (best < 0 || load < bestLoad) {
            best = core;
            bestLoad = load;
        }
    }
    return best;
}

static int firstCoreFor(const OrchestratorThread* threads, size_t count, size_t index, unsigned int freeCores) {
    int core = singleCore(threads[index].mask & freeCores);
    if (core >= 0 && (threads[index].mask == (1u << core) || !threads[index].movable)) {
        return core;
    }
    if (!threads[index].movable) {
        return -1;
    }
    return quietestCore(threads, count, freeCores, threads[index].tid);
}

static const OrchestratorSeat* seatOf(const OrchestratorState* state, unsigned int tid) {
    for (size_t i = 0; i < state->seatCount; i++) {
        if (state->seats[i].tid == tid) {
            return &state->seats[i];
        }
    }
    return NULL;
}

static size_t wantedSeats(const OrchestratorThread* threads, size_t count, unsigned int appMask, size_t* wanted,
    unsigned int* loads) {
    size_t heavy[ORCHESTRATOR_MAX_THREADS];
    unsigned int heavyLoads[ORCHESTRATOR_MAX_THREADS];
    size_t found = heavyThreads(threads, count, appMask, heavy, heavyLoads);
    size_t taken = 0;
    unsigned int cores = coreCount(appMask);
    for (size_t i = 0; i < found && taken < cores; i++) {
        if (placeable(&threads[heavy[i]], appMask)) {
            wanted[taken] = heavy[i];
            loads[taken] = heavyLoads[i];
            taken++;
        }
    }
    return taken;
}

static bool sameSeats(const OrchestratorState* state, const OrchestratorThread* threads, const size_t* wanted,
    size_t wantedCount) {
    if (wantedCount != state->seatCount) {
        return false;
    }
    for (size_t i = 0; i < wantedCount; i++) {
        if (seatOf(state, threads[wanted[i]].tid) == NULL) {
            return false;
        }
    }
    return true;
}

static void seat(const OrchestratorThread* threads, size_t count, unsigned int appMask, const size_t* wanted,
    const unsigned int* loads, size_t wantedCount, OrchestratorState* state) {
    OrchestratorSeat seats[ORCHESTRATOR_MAX_CORES];
    size_t seated = 0;
    unsigned int freeCores = appMask;
    bool placed[ORCHESTRATOR_MAX_CORES] = {false};
    for (size_t i = 0; i < wantedCount; i++) {
        const OrchestratorSeat* old = seatOf(state, threads[wanted[i]].tid);
        if (old != NULL && (freeCores & (1u << old->core)) != 0 && threads[wanted[i]].movable) {
            seats[seated++] = (OrchestratorSeat){old->tid, old->core, loads[i]};
            freeCores &= ~(1u << old->core);
            placed[i] = true;
        }
    }
    for (size_t i = 0; i < wantedCount && freeCores != 0; i++) {
        if (placed[i]) {
            continue;
        }
        int core = firstCoreFor(threads, count, wanted[i], freeCores);
        if (core < 0) {
            continue;
        }
        seats[seated++] = (OrchestratorSeat){threads[wanted[i]].tid, core, loads[i]};
        freeCores &= ~(1u << core);
    }
    for (size_t i = 1; i < seated; i++) {
        OrchestratorSeat moving = seats[i];
        size_t at = i;
        while (at > 0 && seats[at - 1].permille < moving.permille) {
            seats[at] = seats[at - 1];
            at--;
        }
        seats[at] = moving;
    }
    memcpy(state->seats, seats, sizeof(seats[0]) * seated);
    state->seatCount = seated;
}

static void refreshLoads(const OrchestratorThread* threads, size_t count, OrchestratorState* state) {
    for (size_t i = 0; i < state->seatCount; i++) {
        size_t index;
        if (findThread(threads, count, state->seats[i].tid, &index)) {
            state->seats[i].permille = clientLoad(threads, count, index);
        }
    }
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
}

static unsigned int lightCores(const OrchestratorState* state, unsigned int appMask) {
    unsigned int seated = 0;
    for (size_t i = 0; i < state->seatCount; i++) {
        seated |= 1u << state->seats[i].core;
    }
    unsigned int light = appMask & ~seated;
    if (light == 0 && state->seatCount > 1) {
        light = 1u << state->seats[state->seatCount - 1].core;
    }
    return light != 0 ? light : appMask;
}

void orchestratorDecide(const OrchestratorThread* threads, size_t count, unsigned int appMask,
    OrchestratorState* state, OrchestratorPlan* plan) {
    memset(plan, 0, sizeof(*plan));
    plan->mainCore = -1;

    size_t top;
    unsigned int topLoad;
    busiest(threads, count, 0, appMask, &top, &topLoad);
    size_t second;
    unsigned int secondLoad = 0;
    if (top < count) {
        busiest(threads, count, threads[top].tid, appMask, &second, &secondLoad);
    }

    size_t heavy[ORCHESTRATOR_MAX_THREADS];
    unsigned int heavyLoads[ORCHESTRATOR_MAX_THREADS];
    bool crowded = heavyThreads(threads, count, appMask, heavy, heavyLoads) > coreCount(appMask);
    size_t wanted[ORCHESTRATOR_MAX_CORES];
    unsigned int wantedLoads[ORCHESTRATOR_MAX_CORES];
    size_t wantedCount = wantedSeats(threads, count, appMask, wanted, wantedLoads);

    if (state->mode == OrchestratorModeWatch) {
        bool dominant = top < count && placeable(&threads[top], appMask) &&
            topLoad >= ORCHESTRATOR_DOMINANT_PERMILLE && !crowded;
        if (!dominant) {
            state->candidateTid = 0;
            state->heavyPeriods = 0;
        } else if (state->candidateTid == threads[top].tid) {
            state->heavyPeriods++;
        } else {
            state->candidateTid = threads[top].tid;
            state->heavyPeriods = 1;
        }
        if (state->heavyPeriods >= ORCHESTRATOR_ENTER_PERIODS) {
            seat(threads, count, appMask, wanted, wantedLoads, wantedCount, state);
            if (state->seatCount > 0) {
                state->mode = OrchestratorModePlace;
                state->lightPeriods = 0;
                state->changePeriods = 0;
                plan->modeChanged = true;
                plan->seatsChanged = true;
            }
        }
    } else {
        size_t main;
        if (state->seatCount == 0 || !findThread(threads, count, state->seats[0].tid, &main) ||
            (appMask & (1u << state->seats[0].core)) == 0) {
            leave(state, plan);
        } else {
            bool light = topLoad < ORCHESTRATOR_RELEASE_PERMILLE || crowded;
            state->lightPeriods = light ? state->lightPeriods + 1 : 0;
            if (state->lightPeriods >= ORCHESTRATOR_LEAVE_PERIODS) {
                leave(state, plan);
            } else if (!light && !sameSeats(state, threads, wanted, wantedCount)) {
                if (++state->changePeriods >= ORCHESTRATOR_CHANGE_PERIODS) {
                    seat(threads, count, appMask, wanted, wantedLoads, wantedCount, state);
                    state->changePeriods = 0;
                    plan->seatsChanged = true;
                    if (state->seatCount == 0) {
                        leave(state, plan);
                    }
                }
            } else {
                state->changePeriods = 0;
            }
            refreshLoads(threads, count, state);
        }
    }

    plan->mode = state->mode;
    plan->mainPermille = topLoad;
    plan->secondPermille = secondLoad;
    if (state->mode != OrchestratorModePlace) {
        return;
    }
    plan->mainTid = state->seats[0].tid;
    plan->mainCore = state->seats[0].core;
    memcpy(plan->seats, state->seats, sizeof(state->seats[0]) * state->seatCount);
    plan->seatCount = state->seatCount;
    plan->lightMask = lightCores(state, appMask);

    for (size_t i = 0; i < count; i++) {
        const OrchestratorThread* thread = &threads[i];
        if (thread->kind != 'w' || !thread->movable) {
            continue;
        }
        const OrchestratorSeat* own = seatOf(state, thread->tid);
        if (own != NULL) {
            if (thread->mask != (1u << own->core)) {
                addMove(plan, thread->tid, own->core, 1u << own->core);
            }
        } else if ((thread->mask & ~plan->lightMask & appMask) != 0) {
            addMove(plan, thread->tid, quietestCore(threads, count, plan->lightMask, thread->tid), plan->lightMask);
        }
    }
}

const char* orchestratorModeName(OrchestratorMode mode) {
    return mode == OrchestratorModePlace ? "placing" : "watching";
}
