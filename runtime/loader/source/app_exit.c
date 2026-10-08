#include "app_exit.h"

#define CMD_OPEN_APPLICATION_PROXY 0
#define CMD_GET_SELF_CONTROLLER 1
#define CMD_GET_APPLICATION_FUNCTIONS 20
#define CMD_EXIT 0
#define CMD_EXECUTE_PROGRAM 120
#define CMD_CLEAR_USER_CHANNEL 121
#define PROGRAM_SPECIFY_KIND_RESTART 2
#define RESTART_WAIT_NS 10000000000LL

static Result openProxy(Service* applet_oe, Service* proxy, Handle process) {
    Result rc = smGetService(applet_oe, "appletOE");
    if (R_FAILED(rc)) {
        return rc;
    }

    const u64 reserved = 0;
    rc = serviceDispatchIn(
        applet_oe,
        CMD_OPEN_APPLICATION_PROXY,
        reserved,
        .in_send_pid = true,
        .in_num_handles = 1,
        .in_handles = { process },
        .out_num_objects = 1,
        .out_objects = proxy,
    );
    if (R_FAILED(rc)) {
        serviceClose(applet_oe);
    }
    return rc;
}

static Result requestExit(Service* proxy) {
    Service self_controller;
    Result rc = serviceDispatch(
        proxy,
        CMD_GET_SELF_CONTROLLER,
        .out_num_objects = 1,
        .out_objects = &self_controller,
    );
    if (R_SUCCEEDED(rc)) {
        rc = serviceDispatch(&self_controller, CMD_EXIT);
        serviceClose(&self_controller);
    }
    return rc;
}

static Result requestRestartProgram(Service* proxy) {
    Service functions;
    Result rc = serviceDispatch(
        proxy,
        CMD_GET_APPLICATION_FUNCTIONS,
        .out_num_objects = 1,
        .out_objects = &functions,
    );
    if (R_SUCCEEDED(rc)) {
        rc = serviceDispatch(&functions, CMD_CLEAR_USER_CHANNEL);
        if (R_SUCCEEDED(rc)) {
            const struct {
                u32 kind;
                u64 value;
            } in = { PROGRAM_SPECIFY_KIND_RESTART, 0 };
            rc = serviceDispatchIn(&functions, CMD_EXECUTE_PROGRAM, in);
        }
        serviceClose(&functions);
    }
    return rc;
}

static void NX_NORETURN sleepForever(void) {
    for (;;) {
        svcSleepThread(INT64_MAX);
    }
}

void NX_NORETURN appExitToHomeMenu(Handle process) {
    Result rc = smInitialize();
    if (R_SUCCEEDED(rc)) {
        Service applet_oe;
        Service proxy;
        rc = openProxy(&applet_oe, &proxy, process);
        if (R_SUCCEEDED(rc)) {
            rc = requestExit(&proxy);
            serviceClose(&proxy);
            serviceClose(&applet_oe);
        }
        smExit();
    }
    if (R_FAILED(rc)) {
        diagAbortWithResult(rc);
    }
    sleepForever();
}

Result appRestartProgram(Handle process) {
    Result rc = smInitialize();
    if (R_FAILED(rc)) {
        return rc;
    }

    Service applet_oe;
    Service proxy;
    rc = openProxy(&applet_oe, &proxy, process);
    if (R_SUCCEEDED(rc)) {
        rc = requestRestartProgram(&proxy);
        if (R_SUCCEEDED(rc)) {
            svcSleepThread(RESTART_WAIT_NS);
            rc = MAKERESULT(Module_HomebrewLoader, AppRestartError_NotRestarted);
        }
        serviceClose(&proxy);
        serviceClose(&applet_oe);
    }
    smExit();
    return rc;
}
