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
    CHECK(plan.mode == OrchestratorModeSingle && plan.modeChanged);
    CHECK(plan.mainTid == 4 && plan.mainCore == 1);
    CHECK(moveOf(&plan, 4) == NULL);
    const OrchestratorMove* audio = moveOf(&plan, 104);
    CHECK(audio != NULL && audio->mask == 5 && audio->core == 2);
    CHECK(moveOf(&plan, 40) == NULL && moveOf(&plan, 36) == NULL && moveOf(&plan, 44) == NULL);
    orchestratorDecide(threads, 6, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeSingle && !plan.modeChanged);
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

static void twoBusyThreadsStayWatching(void) {
    OrchestratorThread threads[] = {program(4, 900, 2), program(8, 500, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    for (int i = 0; i < 4; i++) {
        orchestratorDecide(threads, 2, 7, &state, &plan);
    }
    CHECK(plan.mode == OrchestratorModeWatch);
}

static void aServerThreadCountsForItsClient(void) {
    OrchestratorThread threads[] = {program(4, 820, 2), server(4, 60, 2), program(8, 380, 1), server(8, 30, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    for (int i = 0; i < 3; i++) {
        orchestratorDecide(threads, 4, 7, &state, &plan);
    }
    CHECK(plan.mode == OrchestratorModeWatch);
    CHECK(plan.mainPermille == 880 && plan.secondPermille == 410);
}

static void aMainThreadOnEveryCoreIsPinnedToTheQuietestOne(void) {
    OrchestratorThread threads[] = {program(4, 960, 7), program(40, 70, 1), program(36, 25, 4)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeSingle && plan.mainCore == 1);
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
    CHECK(plan.mode == OrchestratorModeSingle && moveOf(&plan, 12) == NULL);
}

static void aMainThreadTheProgramPinnedKeepsItsCore(void) {
    OrchestratorThread threads[] = {{4, 'w', 960, 4, false}, program(40, 70, 4), program(36, 25, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    orchestratorDecide(threads, 3, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeSingle && plan.mainCore == 2 && moveOf(&plan, 4) == NULL);
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
    CHECK(plan.mode == OrchestratorModeSingle);
    orchestratorDecide(quiet, 2, 7, &state, &plan);
    orchestratorDecide(quiet, 2, 7, &state, &plan);
    CHECK(plan.mode == OrchestratorModeSingle);
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

static void oneCoreMovesNothing(void) {
    OrchestratorThread threads[] = {program(4, 960, 1), program(40, 70, 1)};
    OrchestratorState state;
    OrchestratorPlan plan;
    orchestratorStart(&state);
    orchestratorDecide(threads, 2, 1, &state, &plan);
    orchestratorDecide(threads, 2, 1, &state, &plan);
    CHECK(plan.mode == OrchestratorModeSingle && plan.moveCount == 0);
}

int main(void) {
    aRaceGivesTheMainThreadItsCore();
    loadingWithShaderWorkStaysWatching();
    twoBusyThreadsStayWatching();
    aServerThreadCountsForItsClient();
    aMainThreadOnEveryCoreIsPinnedToTheQuietestOne();
    aThreadTheProgramPlacedIsLeftAlone();
    aMainThreadTheProgramPinnedKeepsItsCore();
    aMainThreadTheProgramSpreadIsLeftWatching();
    theModeEndsWhenTheMainThreadGoesQuiet();
    theModeEndsWhenTheMainThreadIsGone();
    oneCoreMovesNothing();
    if (failures != 0) {
        printf("%d checks failed\n", failures);
        return 1;
    }
    printf("orchestrator policy: all checks passed\n");
    return 0;
}
