#include <string.h>

#include <switch.h>

#include "app_exit.h"
#include "fixed_image.h"
#include "romfs_reader.h"

#define RUNTIME_NRO "sdmc:/switch/wine/wine-nx-runtime.nro"
#define SD_RUNTIME_PREFIX "sdmc:/switch/wine/"
#define ROMFS_RUNTIME_PREFIX "/switch/wine/"
#define ROMFS_ARGV "/carafe/argv"
#define RUN_NEXT_PATH "/switch/wine/run-next.txt"
#define HEAP_RESERVE 0x6000000ULL

enum {
    LoaderError_Sm = 101,
    LoaderError_Fs = 102,
    LoaderError_Heap = 103,
    LoaderError_Session = 104,
    LoaderError_Thread = 105,
    LoaderError_Receive = 106,
    LoaderError_NoHandle = 107,
    LoaderError_Unmap = 108,
    LoaderError_Path = 109,
    LoaderError_Romfs = 110,
    LoaderError_NroRead = 111,
    LoaderError_NroMagic = 112,
    LoaderError_NroLayout = 113,
    LoaderError_NroTooLarge = 114,
    LoaderError_Map = 115,
    LoaderError_Permission = 116,
    LoaderError_Returned = 117,
    LoaderError_Exit = 118,
};

typedef enum {
    CodeMemoryUnavailable = 0,
    CodeMemoryForeignProcess = BIT(0),
    CodeMemorySameProcess = BIT(0) | BIT(1),
} CodeMemoryCapability;

const char g_noticeText[] = "Carafe loader " VERSION;

static char g_argv[2048];
static char g_nextArgv[2048];
static char g_nextNroPath[512];
static u64 g_nroSize = 0;
static NroHeader g_nroHeader;
static bool g_started = false;
static bool g_launchesGame = false;
static CodeMemoryCapability g_codeMemoryCapability = CodeMemoryUnavailable;
static Handle g_procHandle = INVALID_HANDLE;
static void* g_heapAddr;
static size_t g_heapSize;
static u128 g_userIdStorage;
static u8 g_savedTls[0x100];

u64 g_nroAddr = 0;
Result g_lastRet = 0;

u32 __nx_fs_num_sessions = 1;
u32 __nx_fsdev_direntry_cache_size = 1;
bool __nx_fsdev_support_cwd = false;

void NX_NORETURN nroEntrypointTrampoline(const ConfigEntry* entries, u64 handle, u64 entrypoint);
void NX_NORETURN loadNro(void);

static void NX_NORETURN fail(u32 code) {
    diagAbortWithResult(MAKERESULT(Module_HomebrewLoader, code));
}

void __libnx_initheap(void) {
    static char inner_heap[0x10000];

    extern char* fake_heap_start;
    extern char* fake_heap_end;

    fake_heap_start = &inner_heap[0];
    fake_heap_end = &inner_heap[sizeof(inner_heap)];
}

void __appInit(void) {
    Handle ams_port;
    Result rc = svcConnectToNamedPort(&ams_port, "ams");
    u32 ams_flag = R_VALUE(rc) != KERNELRESULT(NotFound) ? BIT(31) : 0;
    if (R_SUCCEEDED(rc)) {
        svcCloseHandle(ams_port);
    }

    if (R_FAILED(smInitialize())) {
        fail(LoaderError_Sm);
    }

    if (R_SUCCEEDED(setsysInitialize())) {
        SetSysFirmwareVersion firmware;
        if (R_SUCCEEDED(setsysGetFirmwareVersion(&firmware))) {
            hosversionSet(ams_flag | MAKEHOSVERSION(firmware.major, firmware.minor, firmware.micro));
        }
        setsysExit();
    }

    if (R_FAILED(fsInitialize())) {
        fail(LoaderError_Fs);
    }
}

void __wrap_exit(void) {
    fail(LoaderError_Exit);
}

static void setupHeap(void) {
    u64 available = 0;
    u64 used = 0;
    svcGetInfo(&available, InfoType_TotalMemorySize, CUR_PROCESS_HANDLE, 0);
    svcGetInfo(&used, InfoType_UsedMemorySize, CUR_PROCESS_HANDLE, 0);

    u64 size = 0;
    if (available > used + 0x200000) {
        size = (available - used - 0x200000) & ~0x1FFFFFULL;
    }
    if (size == 0) {
        size = 0x2000000ULL * 16;
    }
    if (size > HEAP_RESERVE) {
        size -= HEAP_RESERVE;
    }

    void* addr = NULL;
    if (R_FAILED(svcSetHeapSize(&addr, size)) || addr == NULL) {
        fail(LoaderError_Heap);
    }

    g_heapAddr = addr;
    g_heapSize = size;
}

static void receiveProcessHandle(void* arg) {
    Handle session = (Handle)(uintptr_t)arg;
    void* tls = armGetTls();
    hipcMakeRequestInline(tls);

    s32 index = 0;
    if (R_FAILED(svcReplyAndReceive(&index, &session, 1, INVALID_HANDLE, UINT64_MAX))) {
        fail(LoaderError_Receive);
    }

    HipcParsedRequest request = hipcParseRequest(tls);
    if (request.meta.num_copy_handles != 1) {
        fail(LoaderError_NoHandle);
    }

    g_procHandle = request.data.copy_handles[0];
    svcCloseHandle(session);
}

static void getOwnProcessHandle(void) {
    Handle server;
    Handle client;
    if (R_FAILED(svcCreateSession(&server, &client, 0, 0))) {
        fail(LoaderError_Session);
    }

    Thread thread;
    if (R_FAILED(threadCreate(&thread, receiveProcessHandle, (void*)(uintptr_t)server, NULL, 0x1000, 0x20, 0))
        || R_FAILED(threadStart(&thread))) {
        fail(LoaderError_Thread);
    }

    hipcMakeRequestInline(armGetTls(), .num_copy_handles = 1).copy_handles[0] = CUR_PROCESS_HANDLE;
    svcSendSyncRequest(client);
    svcCloseHandle(client);

    threadWaitForExit(&thread);
    threadClose(&thread);
}

static bool isKernel5xOrLater(void) {
    u64 dummy = 0;
    Result rc = svcGetInfo(&dummy, InfoType_UserExceptionContextAddress, INVALID_HANDLE, 0);
    return R_VALUE(rc) != KERNELRESULT(InvalidEnumValue);
}

static bool isKernel4x(void) {
    u64 dummy = 0;
    Result rc = svcGetInfo(&dummy, InfoType_InitialProcessIdRange, INVALID_HANDLE, 0);
    return R_VALUE(rc) != KERNELRESULT(InvalidEnumValue);
}

static void getCodeMemoryCapability(void) {
    if (detectMesosphere() || (!isKernel5xOrLater() && isKernel4x())) {
        g_codeMemoryCapability = CodeMemorySameProcess;
        return;
    }
    if (!isKernel5xOrLater()) {
        g_codeMemoryCapability = CodeMemoryUnavailable;
        return;
    }

    Handle code;
    if (R_SUCCEEDED(svcCreateCodeMemory(&code, g_heapAddr, 0x1000))) {
        Result rc = svcControlCodeMemory(code, (CodeMapOperation)-1, 0, 0x1000, 0);
        svcCloseHandle(code);
        g_codeMemoryCapability = R_VALUE(rc) == KERNELRESULT(InvalidEnumValue)
            ? CodeMemorySameProcess
            : CodeMemoryForeignProcess;
    }
}

static size_t alignPage(size_t size) {
    return (size + 0xFFF) & ~(size_t)0xFFF;
}

static void unmapNro(void) {
    const NroHeader* header = &g_nroHeader;
    const u64 heap = (u64)g_heapAddr;
    const size_t rw_size = alignPage(header->segments[2].size + header->bss_size);

    svcBreak(BreakReason_NotificationOnlyFlag | BreakReason_PreUnloadDll, g_nroAddr, g_nroSize);

    for (int i = 0; i < 3; i++) {
        const u64 offset = header->segments[i].file_off;
        const u64 size = i == 2 ? rw_size : header->segments[i].size;
        if (R_FAILED(svcUnmapProcessCodeMemory(g_procHandle, g_nroAddr + offset, heap + offset, size))) {
            fail(LoaderError_Unmap);
        }
    }

    svcBreak(BreakReason_NotificationOnlyFlag | BreakReason_PostUnloadDll, g_nroAddr, g_nroSize);
    g_nroAddr = 0;
    g_nroSize = 0;
}

static bool toRomfsPath(const char* nro_path, char* out, size_t out_size) {
    const size_t prefix_len = strlen(SD_RUNTIME_PREFIX);
    if (strncmp(nro_path, SD_RUNTIME_PREFIX, prefix_len) != 0) {
        return false;
    }

    const char* rest = nro_path + prefix_len;
    if (strlen(ROMFS_RUNTIME_PREFIX) + strlen(rest) + 1 > out_size) {
        return false;
    }

    strcpy(out, ROMFS_RUNTIME_PREFIX);
    strcat(out, rest);
    return true;
}

static void trimLineEnd(char* text) {
    size_t len = strlen(text);
    while (len > 0 && (text[len - 1] == '\n' || text[len - 1] == '\r')) {
        text[--len] = '\0';
    }
}

static bool runNextPending(void) {
    u64 program_id = 0;
    svcGetInfo(&program_id, InfoType_ProgramId, CUR_PROCESS_HANDLE, 0);

    FsFileSystem save;
    if (R_FAILED(fsOpen_DeviceSaveData(&save, program_id))) {
        return false;
    }

    FsDirEntryType type;
    bool pending = R_SUCCEEDED(fsFsGetEntryType(&save, RUN_NEXT_PATH, &type));
    fsFsClose(&save);
    return pending;
}

static bool isBareRuntimeRestart(void) {
    return strcmp(g_nextNroPath, RUNTIME_NRO) == 0 && strcmp(g_nextArgv, RUNTIME_NRO) == 0;
}

static void setDefaultLaunch(void) {
    strcpy(g_nextNroPath, RUNTIME_NRO);
    strcpy(g_nextArgv, RUNTIME_NRO);

    RomfsReader reader;
    if (R_FAILED(romfsReaderOpen(&reader))) {
        fail(LoaderError_Romfs);
    }

    u64 offset = 0;
    u64 size = 0;
    if (R_SUCCEEDED(romfsReaderFind(&reader, ROMFS_ARGV, &offset, &size)) && size < sizeof(g_nextArgv)) {
        memset(g_nextArgv, 0, sizeof(g_nextArgv));
        if (R_FAILED(romfsReaderRead(&reader, offset, g_nextArgv, size))) {
            fail(LoaderError_Romfs);
        }
        trimLineEnd(g_nextArgv);
        g_launchesGame = strcmp(g_nextArgv, RUNTIME_NRO) != 0;
    }

    romfsReaderClose(&reader);
}

static NroHeader* readNro(const char* romfs_path) {
    RomfsReader reader;
    if (R_FAILED(romfsReaderOpen(&reader))) {
        fail(LoaderError_Romfs);
    }

    u64 offset = 0;
    u64 file_size = 0;
    if (R_FAILED(romfsReaderFind(&reader, romfs_path, &offset, &file_size))) {
        fail(LoaderError_Path);
    }

    u8* nro = (u8*)g_heapAddr;
    NroHeader* header = (NroHeader*)(nro + sizeof(NroStart));
    const size_t head_size = sizeof(NroStart) + sizeof(NroHeader);

    if (file_size < head_size || R_FAILED(romfsReaderRead(&reader, offset, nro, head_size))) {
        fail(LoaderError_NroRead);
    }
    if (header->magic != NROHEADER_MAGIC) {
        fail(LoaderError_NroMagic);
    }
    if (header->size < head_size || header->size > file_size) {
        fail(LoaderError_NroLayout);
    }
    if (alignPage((size_t)header->size + header->bss_size) >= g_heapSize) {
        fail(LoaderError_NroTooLarge);
    }
    if (R_FAILED(romfsReaderRead(&reader, offset + head_size, nro + head_size, header->size - head_size))) {
        fail(LoaderError_NroRead);
    }

    romfsReaderClose(&reader);

    for (int i = 0; i < 3; i++) {
        const u64 segment_off = header->segments[i].file_off;
        const u64 segment_size = header->segments[i].size;
        if (segment_off >= header->size || segment_size > header->size - segment_off) {
            fail(LoaderError_NroLayout);
        }
    }

    return header;
}

static void setPermission(u64 address, u64 size, u32 permission) {
    if (R_FAILED(svcSetProcessMemoryPermission(g_procHandle, address, size, permission))) {
        fail(LoaderError_Permission);
    }
}

void NX_NORETURN loadNro(void) {
    memcpy((u8*)armGetTls() + 0x100, g_savedTls, 0x100);

    if (g_nroSize > 0) {
        unmapNro();
    }

    if (!g_started) {
        g_started = true;
    } else if (g_nextNroPath[0] == '\0' || (g_launchesGame && isBareRuntimeRestart() && !runNextPending())) {
        appExitToHomeMenu(g_procHandle);
    }

    char romfs_path[sizeof(g_nextNroPath)];
    if (!toRomfsPath(g_nextNroPath, romfs_path, sizeof(romfs_path))) {
        fail(LoaderError_Path);
    }
    g_nextNroPath[0] = '\0';

    memcpy(g_argv, g_nextArgv, sizeof(g_argv));
    svcBreak(BreakReason_NotificationOnlyFlag | BreakReason_PreLoadDll, (uintptr_t)g_argv, sizeof(g_argv));

    memcpy(&g_nroHeader, readNro(romfs_path), sizeof(g_nroHeader));
    const NroHeader* header = &g_nroHeader;

    const size_t total_size = alignPage((size_t)header->size + header->bss_size);
    const size_t rw_size = alignPage(header->segments[2].size + header->bss_size);

    virtmemLock();
    void* map_addr = virtmemFindCodeMemory(total_size, 0);
    Result rc = svcMapProcessCodeMemory(g_procHandle, (u64)map_addr, (u64)g_heapAddr, total_size);
    virtmemUnlock();
    if (R_FAILED(rc)) {
        fail(LoaderError_Map);
    }

    const u64 base = (u64)map_addr;
    setPermission(base + header->segments[0].file_off, header->segments[0].size, Perm_R | Perm_X);
    setPermission(base + header->segments[1].file_off, header->segments[1].size, Perm_R);
    setPermission(base + header->segments[2].file_off, rw_size, Perm_Rw);

    const u64 nro_size = header->segments[2].file_off + rw_size;
    const u64 nro_heap_start = (u64)g_heapAddr + nro_size;
    const u64 nro_heap_size = g_heapSize - nro_size;

    static ConfigEntry entries[] = {
        { EntryType_MainThreadHandle, 0, { 0, 0 } },
        { EntryType_ProcessHandle, 0, { 0, 0 } },
        { EntryType_AppletType, 0, { AppletType_SystemApplication, EnvAppletFlags_ApplicationOverride } },
        { EntryType_OverrideHeap, EntryFlag_IsMandatory, { 0, 0 } },
        { EntryType_Argv, 0, { 0, 0 } },
        { EntryType_NextLoadPath, 0, { 0, 0 } },
        { EntryType_LastLoadResult, 0, { 0, 0 } },
        { EntryType_SyscallAvailableHint, 0, { UINT64_MAX, UINT64_MAX } },
        { EntryType_SyscallAvailableHint2, 0, { UINT64_MAX, 0 } },
        { EntryType_RandomSeed, 0, { 0, 0 } },
        { EntryType_UserIdStorage, 0, { 0, 0 } },
        { EntryType_HosVersion, 0, { 0, 0 } },
        { EntryType_EndOfList, 0, { 0, 0 } },
    };

    ConfigEntry* syscalls = &entries[7];
    if (!(g_codeMemoryCapability & BIT(0))) {
        syscalls->Value[0x4B / 64] &= ~(1ULL << (0x4B % 64));
    }
    if (!(g_codeMemoryCapability & BIT(1))) {
        syscalls->Value[0x4C / 64] &= ~(1ULL << (0x4C % 64));
    }

    entries[0].Value[0] = envGetMainThreadHandle();
    entries[1].Value[0] = g_procHandle;
    entries[3].Value[0] = nro_heap_start;
    entries[3].Value[1] = nro_heap_size;
    entries[4].Value[1] = (u64)(uintptr_t)g_argv;
    entries[5].Value[0] = (u64)(uintptr_t)g_nextNroPath;
    entries[5].Value[1] = (u64)(uintptr_t)g_nextArgv;
    entries[6].Value[0] = g_lastRet;
    entries[9].Value[0] = randomGet64();
    entries[9].Value[1] = randomGet64();
    entries[10].Value[0] = (u64)(uintptr_t)&g_userIdStorage;
    entries[11].Value[0] = hosversionGet();
    entries[11].Value[1] = hosversionIsAtmosphere() ? 0x41544d4f53504852ULL : 0;
    entries[12].Value[0] = (u64)(uintptr_t)g_noticeText;
    entries[12].Value[1] = sizeof(g_noticeText);

    g_nroAddr = base;
    g_nroSize = nro_size;

    svcBreak(BreakReason_NotificationOnlyFlag | BreakReason_PostLoadDll, g_nroAddr, g_nroSize);
    nroEntrypointTrampoline(entries, -1, g_nroAddr);
}

int main(void) {
    memcpy(g_savedTls, (u8*)armGetTls() + 0x100, 0x100);

    setupHeap();
    setDefaultLaunch();
    fixedImageClaim(g_nextArgv);
    smExit();
    getOwnProcessHandle();
    getCodeMemoryCapability();
    loadNro();
}
