#include <FEXCore/Utils/Horizon.h>
#include <windows.h>
#include "unixlib.h"

namespace FEXCore::Allocator {
bool HorizonInitialize() {
    return true;
}

bool HorizonAttachThread() {
    return true;
}

bool HorizonProtectCode(uint64_t address, bool enable) {
    DWORD previous;
    return VirtualProtect(reinterpret_cast<void*>(address), 4096,
        enable ? PAGE_EXECUTE_READ : PAGE_EXECUTE_READWRITE, &previous);
}

void* HorizonAllocateJit(size_t size) {
    return VirtualAlloc(nullptr, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
}

bool HorizonFreeJit(void* pointer) {
    return VirtualFree(pointer, 0, MEM_RELEASE);
}

void* HorizonWritableAddress(const void* pointer) {
    return const_cast<void*>(pointer);
}

void HorizonFlushICache(void* pointer, size_t size) {
    FlushInstructionCache(GetCurrentProcess(), pointer, size);
}
}

namespace FEX::Windows::HorizonHeap {
NTSTATUS Call(unsigned int function, void* args) {
    auto* request = static_cast<wine_nx_fex_heap*>(args);
    auto heap = GetProcessHeap();
    auto flags = request->zero ? HEAP_ZERO_MEMORY : 0;
    auto pointer = reinterpret_cast<void*>(request->pointer);
    if (function == WINE_NX_FEX_HEAP_ALLOC) {
        pointer = HeapAlloc(heap, flags, request->size);
    } else if (function == WINE_NX_FEX_HEAP_REALLOC) {
        pointer = HeapReAlloc(heap, flags, pointer, request->size);
    } else if (function == WINE_NX_FEX_HEAP_FREE) {
        return HeapFree(heap, 0, pointer) ? 0 : static_cast<NTSTATUS>(0xc000000d);
    } else {
        return static_cast<NTSTATUS>(0xc000000d);
    }
    request->pointer = reinterpret_cast<uintptr_t>(pointer);
    return pointer ? 0 : static_cast<NTSTATUS>(0xc0000017);
}
}
