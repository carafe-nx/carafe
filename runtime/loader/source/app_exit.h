#pragma once

#include <switch.h>

enum {
    AppRestartError_NotRestarted = 119,
};

void NX_NORETURN appExitToHomeMenu(Handle process);

Result appRestartProgram(Handle process);
