#include <FEXCore/Utils/LogManager.h>
#include <DummyHandlers.h>
#include <Common/CPUFeatures.h>
#include <FEXCore/Config/Config.h>
#include <FEXCore/Core/Context.h>
#include <FEXCore/Core/CoreState.h>
#include <FEXCore/Core/HostFeatures.h>
#include <FEXCore/Debug/InternalThreadState.h>

#include <windows.h>
#include <Common/OvercommitTracker.h>

#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

static FEX::Windows::OvercommitTracker* overcommitTracker;

class TestSyscalls final : public FEX::DummyHandlers::DummySyscallHandler {
public:
    void MarkOvercommitRange(uint64_t start, uint64_t length) override {
        overcommitTracker->MarkRange(start, length);
    }

    void UnmarkOvercommitRange(uint64_t start, uint64_t length) override {
        overcommitTracker->UnmarkRange(start, length);
    }
};

static void logMessage(LogMan::DebugLevels, const char* message) {
    std::fprintf(stderr, "%s\n", message);
}

static void logAssert(const char* message) {
    std::fprintf(stderr, "%s\n", message);
    std::fflush(stderr);
}

static LONG WINAPI nativeFault(EXCEPTION_POINTERS* exception) {
    if (exception->ExceptionRecord->ExceptionCode == EXCEPTION_ACCESS_VIOLATION && overcommitTracker
        && overcommitTracker->HandleAccessViolation(exception->ExceptionRecord->ExceptionInformation[1])) {
        return EXCEPTION_CONTINUE_EXECUTION;
    }
    HMODULE module = nullptr;
    char name[MAX_PATH] {};
    auto address = exception->ExceptionRecord->ExceptionAddress;
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<const char*>(address), &module);
    if (module) GetModuleFileNameA(module, name, sizeof(name));
    std::fprintf(stderr, "Native test fault=%08lx address=%p module=%s offset=%llx x0=%llx x1=%llx sp=%llx\n",
        exception->ExceptionRecord->ExceptionCode, address, name,
        reinterpret_cast<uintptr_t>(address) - reinterpret_cast<uintptr_t>(module),
        exception->ContextRecord->X0, exception->ContextRecord->X1, exception->ContextRecord->Sp);
    return EXCEPTION_CONTINUE_SEARCH;
}

static uint32_t compareFlags(uint32_t left) {
    uint32_t right = 8;
    uint32_t result = left - right;
    uint32_t flags = (left < right ? 1 : 0) | (!result ? 0x40 : 0) | ((result >> 31) ? 0x80 : 0);
    flags |= ((left ^ right ^ result) & 0x10);
    flags |= (((left ^ right) & (left ^ result)) >> 31) ? 0x800 : 0;
    unsigned int bits = result & 0xff;
    bits ^= bits >> 4;
    bits ^= bits >> 2;
    bits ^= bits >> 1;
    return flags | (!(bits & 1) ? 4 : 0);
}

static float roundedValue(float input, unsigned int mode) {
    if (mode == 1) return std::floor(input);
    if (mode == 2) return std::ceil(input);
    if (mode == 3) return std::trunc(input);
    float lower = std::floor(input);
    float fraction = input - lower;
    if (fraction < 0.5f) return lower;
    if (fraction > 0.5f) return lower + 1;
    return std::fmod(lower, 2.0f) == 0 ? lower : lower + 1;
}

int main(int argc, char** argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    FEX::Windows::OvercommitTracker overcommit(false);
    overcommitTracker = &overcommit;
    SetUnhandledExceptionFilter(nativeFault);
    AddVectoredExceptionHandler(1, nativeFault);
    LogMan::Msg::InstallHandler(logMessage);
    LogMan::Throw::InstallHandler(logAssert);
    FEXCore::Config::Initialize();
    FEXCore::Config::Load();
    using namespace FEXCore::Config;
    Set(CONFIG_IS64BIT_MODE, "0");
    Set(CONFIG_X87REDUCEDPRECISION, "1");
    Set(CONFIG_X87CONVERSIONROUNDING, "1");
    Set(CONFIG_X87FASTSTACK, "1");
    Set(CONFIG_X87SINGLEPRECISION, argc > 1 ? argv[1] : "0");
    Set(CONFIG_FLAGSWITHOUTFLAGM, "1");
    Set(CONFIG_MULTIBLOCK, "1");
    Set(CONFIG_MAXINST, argc > 2 ? argv[2] : "1000");
    Set(CONFIG_TSOENABLED, "0");
    Set(CONFIG_DISKCACHE, "0");

    constexpr uintptr_t codeAddress = 0x14000000;
    constexpr uintptr_t dataAddress = 0x15000000;
    constexpr uintptr_t stackAddress = 0x16000000;
    auto code = static_cast<uint8_t*>(VirtualAlloc(reinterpret_cast<void*>(codeAddress), 0x20000,
        MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
    auto data = static_cast<uint8_t*>(VirtualAlloc(reinterpret_cast<void*>(dataAddress), 0x10000,
        MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    auto stack = VirtualAlloc(reinterpret_cast<void*>(stackAddress), 0x10000,
        MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!code || !data || !stack) return 2;

    constexpr std::array<float, 6> values {-2.5f, -1.5f, -0.25f, 0.25f, 1.5f, 2.5f};
    constexpr std::array<uint32_t, 7> comparisons {0, 7, 8, 9, 0x80000000, 0x7fffffff, 0xffffffff};
    struct Case { unsigned int mode; unsigned int sseMode; float value; uint32_t comparison; };
    std::vector<Case> cases;
    size_t position = 0;
    auto byte = [&](uint8_t value) { code[position++] = value; };
    auto word = [&](uint32_t value) { std::memcpy(code + position, &value, 4); position += 4; };
    for (unsigned int mode = 0; mode < 4; mode++) {
        for (unsigned int sseMode = 0; sseMode < 4; sseMode++) {
            for (float value : values) {
                for (uint32_t comparison : comparisons) {
                    uint32_t address = dataAddress + static_cast<uint32_t>(cases.size()) * 24;
                    uint32_t control = 0x23f | (mode << 10);
                    std::memcpy(data + cases.size() * 24, &control, 4);
                    std::memcpy(data + cases.size() * 24 + 4, &value, 4);
                    uint32_t mxcsr = 0x1f80 | (sseMode << 13);
                    std::memcpy(data + cases.size() * 24 + 20, &mxcsr, 4);
                    byte(0x0f); byte(0xae); byte(0x15); word(address + 20);
                    byte(0xd9); byte(0x2d); word(address);
                    byte(0xd9); byte(0x05); word(address + 4);
                    byte(0xb9); word(comparison);
                    byte(0x83); byte(0xf9); byte(8);
                    byte(0xd9); byte(0xfc);
                    byte(0xd9); byte(0x15); word(address + 8);
                    byte(0x7c); byte(9);
                    byte(0xc6); byte(0x05); word(address + 16); byte(0);
                    byte(0xeb); byte(7);
                    byte(0xc6); byte(0x05); word(address + 16); byte(1);
                    byte(0x9c); byte(0x58);
                    byte(0xa3); word(address + 12);
                    byte(0xdd); byte(0xd8);
                    cases.push_back({mode, sseMode, value, comparison});
                }
            }
        }
    }
    byte(0xf4);
    FlushInstructionCache(GetCurrentProcess(), code, position);

    std::fprintf(stderr, "FEX test: create context\n");
    auto features = FEX::Windows::CPUFeatures::FetchHostFeatures(true, FEXCore::HostFeatures::HostTypeEnum::Wow64);
    auto context = FEXCore::Context::Context::CreateNewContext(features);
    auto signals = FEX::DummyHandlers::CreateSignalDelegator();
    auto syscalls = std::make_unique<TestSyscalls>();
    context->EnableExitOnHLT();
    context->SetSignalDelegator(signals.get());
    context->SetSyscallHandler(syscalls.get());
    std::fprintf(stderr, "FEX test: initialize core\n");
    if (!context->InitCore()) return 3;
    std::fprintf(stderr, "FEX test: create thread\n");
    auto thread = context->CreateThread();
    auto& state = thread->CurrentFrame->State;
    std::array<FEXCore::Core::CPUState::gdt_segment, 16> descriptors {};
    state.segment_arrays[FEXCore::Core::CPUState::SEGMENT_ARRAY_INDEX_GDT] = descriptors.data();
    state.segment_arrays[FEXCore::Core::CPUState::SEGMENT_ARRAY_INDEX_LDT] = descriptors.data();
    state.cs_idx = 0x23;
    state.rip = codeAddress;
    state.gregs[FEXCore::X86State::REG_RSP] = stackAddress + 0xfff0;
    std::fprintf(stderr, "FEX test: set GDT\n");
    auto descriptor = FEXCore::Core::CPUState::GetSegmentFromIndex(state, state.cs_idx);
    FEXCore::Core::CPUState::SetGDTBase(descriptor, 0);
    FEXCore::Core::CPUState::SetGDTLimit(descriptor, 0xfffff);
    descriptor->L = 0;
    descriptor->D = 1;
    state.cs_cached = FEXCore::Core::CPUState::CalculateGDTBase(*descriptor);
    std::fprintf(stderr, "FEX test: execute\n");
    context->ExecuteThread(thread);

    unsigned int failures = 0;
    for (size_t i = 0; i < cases.size(); i++) {
        auto test = cases[i];
        float actual;
        uint32_t flags;
        std::memcpy(&actual, data + i * 24 + 8, 4);
        std::memcpy(&flags, data + i * 24 + 12, 4);
        bool branch = data[i * 24 + 16] != 0;
        bool expectedBranch = static_cast<int32_t>(test.comparison) < 8;
        if (actual != roundedValue(test.value, test.mode) || (flags & 0x8d5) != compareFlags(test.comparison)
            || branch != expectedBranch) {
            if (failures++ < 8) std::printf("FAIL mode=%u sse=%u input=%g cmp=%08x flags=%03x expected=%03x branch=%u/%u round=%g/%g\n",
                test.mode, test.sseMode, test.value, test.comparison, flags & 0x8d5, compareFlags(test.comparison),
                branch, expectedBranch, actual, roundedValue(test.value, test.mode));
        }
    }
    std::printf("FEX FRNDINT: %zu cases, %u failed; single=%s maxinst=%s\n", cases.size(), failures,
        argc > 1 ? argv[1] : "0", argc > 2 ? argv[2] : "1000");
    context->DestroyThread(thread);
    return failures ? 1 : 0;
}
