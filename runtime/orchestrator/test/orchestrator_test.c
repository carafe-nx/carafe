#include "../orchestrator_policy.h"

#include <stdio.h>

static int failures;

#define CHECK(condition)                                                        \
    do {                                                                        \
        if (!(condition)) {                                                     \
            printf("%s:%d: %s\n", __FILE__, __LINE__, #condition);              \
            failures++;                                                         \
        }                                                                       \
    } while (0)

static OrchestratorThread program(unsigned int tid, unsigned int permille, unsigned int mask) {
    return (OrchestratorThread){tid, 'w', permille, mask, true};
}

static OrchestratorThread server(unsigned int tid, unsigned int permille, unsigned int mask) {
    return (OrchestratorThread){tid, 's', permille, mask, false};
}

static OrchestratorThread helper(unsigned int tid, unsigned int permille) {
    return (OrchestratorThread){tid, 'w', permille, 8, false};
}

static const OrchestratorMove* moveOf(const OrchestratorPlan* plan, unsigned int tid) {
    for (size_t i = 0; i < plan->moveCount; i++) {
        if (plan->moves[i].tid == tid) {
            return &plan->moves[i];
        }
    }
    return NULL;
}

static int seatCore(const OrchestratorPlan* plan, unsigned int tid) {
    for (size_t i = 0; i < plan->seatCount; i++) {
        if (plan->seats[i].tid == tid) {
            return plan->seats[i].core;
        }
    }
    return -1;
}

static void apply(OrchestratorThread* threads, size_t count, const OrchestratorPlan* plan) {
    for (size_t i = 0; i < plan->moveCount; i++) {
        for (size_t j = 0; j < count; j++) {
            if (threads[j].kind == 'w' && threads[j].tid == plan->moves[i].tid) {
                threads[j].mask = plan->moves[i].mask;
            }
        }
    }
}

static void aRaceGivesTheMainThreadItsCore(void) {
    OrchestratorThread threads[] = {
        program(4, 970, 2), helper(44, 370), program(40, 70, 1), program(104, 50, 2), program(36, 25, 4),
        server(4, 5, 2),
    };
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 6, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch && plan.moveCount == 0);
    orchestratorDecide(threads, 6, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.modeChanged && plan.seatsChanged);
    CHECK(plan.mainTid == 4 && plan.mainCore == 1 && plan.seatCount == 1);
    CHECK(moveOf(&plan, 4) == NULL);
    const OrchestratorMove* audio = moveOf(&plan, 104);
    CHECK(audio != NULL && audio->mask == 5 && audio->core == 2);
    CHECK(moveOf(&plan, 40) == NULL && moveOf(&plan, 36) == NULL && moveOf(&plan, 44) == NULL);
    orchestratorDecide(threads, 6, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && !plan.modeChanged && !plan.seatsChanged);
}

static void loadingWithShaderWorkStaysWatching(void) {
    OrchestratorThread threads[] = {
        program(4, 800, 2), program(48, 220, 7), program(56, 140, 7), helper(44, 80),
    };
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    for (int i = 0; i < 4; i++) {
        orchestratorDecide(threads, 4, 7, &state, &plan);
    }
    CHECK(plan.mode == OrchestratorModeWatch && plan.moveCount == 0);
}

static void twoBusyThreadsGetACoreEach(void) {
    OrchestratorThread threads[] = {program(4, 900, 2), program(8, 500, 1), program(40, 70, 7)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.seatCount == 2);
    CHECK(seatCore(&plan, 4) == 1 && seatCore(&plan, 8) == 0);
    CHECK(plan.lightMask == 4);
    const OrchestratorMove* light = moveOf(&plan, 40);
    CHECK(light != NULL && light->mask == 4 && light->core == 2);
    CHECK(moveOf(&plan, 4) == NULL && moveOf(&plan, 8) == NULL);
}

static void aServerThreadCountsForItsClient(void) {
    OrchestratorThread threads[] = {program(4, 820, 2), server(4, 60, 2), program(8, 380, 1), server(8, 30, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    for (int i = 0; i < 3; i++) {
        orchestratorDecide(threads, 4, 7, &state, &plan);
    }
    CHECK(plan.mode == OrchestratorModePlace && plan.seatCount == 2);
    CHECK(plan.mainPermille == 880 && plan.secondPermille == 410);
    CHECK(plan.seats[0].tid == 4 && plan.seats[0].permille == 880 && plan.seats[1].permille == 410);
}

static void aSecondBusyThreadGetsItsOwnCoreAfterTwoPeriods(void) {
    OrchestratorThread threads[] = {program(4, 960, 2), program(48, 100, 5), program(40, 60, 5)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.seatCount == 1);
    threads[1].permille = 600;
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.seatCount == 1 && !plan.seatsChanged);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.seatCount == 2 && plan.seatsChanged && seatCore(&plan, 4) == 1);
    int frontEnd = seatCore(&plan, 48);
    CHECK(frontEnd == 0 || frontEnd == 2);
    const OrchestratorMove* worker = moveOf(&plan, 48);
    CHECK(worker != NULL && worker->mask == (1u << frontEnd));
    const OrchestratorMove* light = moveOf(&plan, 40);
    CHECK(light != NULL && light->mask == (5u & ~(1u << frontEnd)));
}

static void threeBusyThreadsLeaveLightOnesWithTheLightest(void) {
    OrchestratorThread threads[] = {
        program(4, 950, 2), program(48, 600, 4), program(52, 400, 1), program(40, 60, 7), program(36, 20, 2),
    };
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 5, 7, &state, &plan);
    orchestratorDecide(threads, 5, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.seatCount == 3);
    CHECK(seatCore(&plan, 4) == 1 && seatCore(&plan, 48) == 2 && seatCore(&plan, 52) == 0);
    CHECK(plan.lightMask == 1);
    const OrchestratorMove* wide = moveOf(&plan, 40);
    const OrchestratorMove* onMain = moveOf(&plan, 36);
    CHECK(wide != NULL && wide->mask == 1 && onMain != NULL && onMain->mask == 1);
}

static void aQuietThreadLosesItsSeat(void) {
    OrchestratorThread threads[] = {program(4, 950, 2), program(48, 600, 4), program(40, 60, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    apply(threads, 3, &plan);
    CHECK(plan.seatCount == 2);
    threads[1].permille = 80;
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.seatCount == 2);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.seatCount == 1 && plan.seatsChanged && plan.lightMask == 5);
    CHECK(moveOf(&plan, 48) == NULL && moveOf(&plan, 40) == NULL);
}

static void tooManyBusyThreadsLeaveItToTheBalancer(void) {
    OrchestratorThread busy[] = {program(4, 950, 2), program(48, 600, 4), program(52, 400, 1)};
    OrchestratorThread crowded[] = {
        program(4, 950, 2), program(48, 600, 4), program(52, 400, 1), program(60, 500, 7),
    };
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(busy, 3, 7, &state, &plan);
    orchestratorDecide(busy, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace);
    for (int i = 0; i < 2; i++) {
        orchestratorDecide(crowded, 4, 7, &state, &plan);
        CHECK(plan.mode == OrchestratorModePlace);
    }
    orchestratorDecide(crowded, 4, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch && plan.modeChanged && plan.moveCount == 0);
    orchestratorDecide(crowded, 4, 7, &state, &plan);
    orchestratorDecide(crowded, 4, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch);
}

static void noSeatLeftLeavesItToTheBalancer(void) {
    OrchestratorThread threads[] = {program(4, 950, 2), {12, 'w', 100, 7, false}, program(40, 60, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.seatCount == 1);
    threads[0].permille = 100;
    threads[1].permille = 900;
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch && plan.modeChanged && plan.moveCount == 0);
}

static void aMainThreadOnEveryCoreIsPinnedToTheQuietestOne(void) {
    OrchestratorThread threads[] = {program(4, 960, 7), program(40, 70, 1), program(36, 25, 4)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.mainCore == 1);
    const OrchestratorMove* main = moveOf(&plan, 4);
    CHECK(main != NULL && main->core == 1 && main->mask == 2);
}

static void aThreadTheProgramPlacedIsLeftAlone(void) {
    OrchestratorThread threads[] = {program(4, 960, 2), {12, 'w', 40, 2, false}};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 2, 7, &state, &plan);
    orchestratorDecide(threads, 2, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && moveOf(&plan, 12) == NULL);
}

static void aMainThreadTheProgramPinnedKeepsItsCore(void) {
    OrchestratorThread threads[] = {{4, 'w', 960, 4, false}, program(40, 70, 4), program(36, 25, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.mainCore == 2 && moveOf(&plan, 4) == NULL);
    const OrchestratorMove* other = moveOf(&plan, 40);
    CHECK(other != NULL && other->mask == 3 && other->core == 1);
}

static void aMainThreadTheProgramSpreadIsLeftWatching(void) {
    OrchestratorThread threads[] = {{4, 'w', 960, 7, false}, program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    for (int i = 0; i < 3; i++) {
        orchestratorDecide(threads, 2, 7, &state, &plan);
    }
    CHECK(plan.mode == OrchestratorModeWatch && plan.moveCount == 0);
}

static void theModeEndsWhenTheMainThreadGoesQuiet(void) {
    OrchestratorThread busy[] = {program(4, 960, 2), program(40, 70, 1)};
    OrchestratorThread quiet[] = {program(4, 500, 2), program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(busy, 2, 7, &state, &plan);
    orchestratorDecide(busy, 2, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace);
    orchestratorDecide(quiet, 2, 7, &state, &plan);
    orchestratorDecide(quiet, 2, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace);
    orchestratorDecide(quiet, 2, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch && plan.modeChanged && plan.moveCount == 0);
}

static void theModeEndsWhenTheMainThreadIsGone(void) {
    OrchestratorThread busy[] = {program(4, 960, 2), program(40, 70, 1)};
    OrchestratorThread gone[] = {program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(busy, 2, 7, &state, &plan);
    orchestratorDecide(busy, 2, 7, &state, &plan);
    orchestratorDecide(gone, 1, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeWatch && plan.modeChanged);
}

static void aBusyGraphicsWorkerOnCoreThreeKeepsTheMode(void) {
    OrchestratorThread threads[] = {program(4, 960, 2), helper(44, 440), program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.secondPermille == 70 && plan.seatCount == 1);
    for (int period = 0; period < 5; period++) {
        orchestratorDecide(threads, 3, 7, &state, &plan);
        CHECK(plan.mode == OrchestratorModePlace && !plan.modeChanged && !plan.seatsChanged);
    }
}

static void oneCoreMovesNothing(void) {
    OrchestratorThread threads[] = {program(4, 960, 1), program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 2, 1, &state, &plan);
    orchestratorDecide(threads, 2, 1, &state, &plan);
    CHECK(plan.mode == OrchestratorModePlace && plan.moveCount == 0);
}

int main(void) {
    aRaceGivesTheMainThreadItsCore();
    loadingWithShaderWorkStaysWatching();
    twoBusyThreadsGetACoreEach();
    aServerThreadCountsForItsClient();
    aSecondBusyThreadGetsItsOwnCoreAfterTwoPeriods();
    threeBusyThreadsLeaveLightOnesWithTheLightest();
    aQuietThreadLosesItsSeat();
    tooManyBusyThreadsLeaveItToTheBalancer();
    noSeatLeftLeavesItToTheBalancer();
    aMainThreadOnEveryCoreIsPinnedToTheQuietestOne();
    aThreadTheProgramPlacedIsLeftAlone();
    aMainThreadTheProgramPinnedKeepsItsCore();
    aMainThreadTheProgramSpreadIsLeftWatching();
    theModeEndsWhenTheMainThreadGoesQuiet();
    theModeEndsWhenTheMainThreadIsGone();
    aBusyGraphicsWorkerOnCoreThreeKeepsTheMode();
    oneCoreMovesNothing();
    if (failures != 0) {
        printf("%d checks failed\n", failures);
        return 1;
    }
    printf("orchestrator policy: all checks passed\n");
    return 0;
}
