#ifndef CARAFE_ORCHESTRATOR_POLICY_H
#define CARAFE_ORCHESTRATOR_POLICY_H

#include <stdbool.h>
#include <stddef.h>

#define ORCHESTRATOR_MAX_THREADS 128
#define ORCHESTRATOR_MAX_CORES 32
#define ORCHESTRATOR_DOMINANT_PERMILLE 850
#define ORCHESTRATOR_HEAVY_PERMILLE 300
#define ORCHESTRATOR_RELEASE_PERMILLE 600
#define ORCHESTRATOR_ENTER_PERIODS 2
#define ORCHESTRATOR_CHANGE_PERIODS 2
#define ORCHESTRATOR_LEAVE_PERIODS 3

typedef enum {
    OrchestratorModeWatch,
    OrchestratorModePlace,
} OrchestratorMode;

typedef struct {
    unsigned int tid;
    char kind;
    unsigned int permille;
    unsigned int mask;
    bool movable;
} OrchestratorThread;

typedef struct {
    unsigned int tid;
    int core;
    unsigned int mask;
} OrchestratorMove;

typedef struct {
    unsigned int tid;
    int core;
    unsigned int permille;
} OrchestratorSeat;

typedef struct {
    OrchestratorMode mode;
    OrchestratorSeat seats[ORCHESTRATOR_MAX_CORES];
    size_t seatCount;
    unsigned int candidateTid;
    unsigned int heavyPeriods;
    unsigned int changePeriods;
    unsigned int lightPeriods;
} OrchestratorState;

typedef struct {
    OrchestratorMode mode;
    bool modeChanged;
    bool seatsChanged;
    unsigned int mainTid;
    int mainCore;
    unsigned int mainPermille;
    unsigned int secondPermille;
    OrchestratorSeat seats[ORCHESTRATOR_MAX_CORES];
    size_t seatCount;
    unsigned int lightMask;
    size_t moveCount;
    OrchestratorMove moves[ORCHESTRATOR_MAX_THREADS];
} OrchestratorPlan;

void orchestratorStart(OrchestratorState* state);
void orchestratorDecide(const OrchestratorThread* threads, size_t count, unsigned int appMask,
    OrchestratorState* state, OrchestratorPlan* plan);
const char* orchestratorModeName(OrchestratorMode mode);

#endif
