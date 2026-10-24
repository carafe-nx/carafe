#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include <switch.h>

#include "carafe_draw.h"
#include "carafe_error.h"
#include "carafe_menu.h"
#include "carafe_overlay.h"
#include "carafe_registry.h"
#include "carafe_settings.h"
#include "osk.h"

#define ERROR_FILE "sdmc:/switch/carafe/%016lx/carafe-error.txt"
#define PATH_SIZE 128
#define ERROR_FLAG "--carafe-error"
#define RUNTIME_NRO "sdmc:/switch/wine/wine-nx-runtime.nro"
#define SCREEN_WIDTH CARAFE_SCREEN_WIDTH
#define SCREEN_HEIGHT CARAFE_SCREEN_HEIGHT
#define ARGV_SIZE 2048
#define NAME_SIZE 96
#define LINE_SIZE 160
#define MAX_BUTTONS 3
#define MAX_ROWS 4
#define CHANGES_SIZE (64 * 1024)

#define MARGIN 72
#define TEXT_WIDTH 600
#define ICON_SIZE 168
#define ICON_RADIUS 32
#define BUTTON_HEIGHT 64
#define BUTTON_RADIUS 16
#define BUTTON_GAP 16
#define BUTTON_PAD 30
#define BUTTON_FOCUS_PAD 14
#define GLYPH_SIZE 34
#define CARD_WIDTH 440
#define CARD_X (SCREEN_WIDTH - MARGIN - CARD_WIDTH)
#define CARD_RADIUS 24
#define CARD_PAD_X 36
#define CARD_PAD_Y 40

typedef enum {
    KindCrash,
    KindThreads,
    KindMemory,
    KindSetup,
    KindSystem,
    KindCount,
} Kind;

typedef enum {
    ChoiceNone,
    ChoiceRestart,
    ChoiceExit,
    ChoiceSettings,
    ChoiceRevert,
    ChoiceKeep,
    ChoiceRecommended,
} Choice;

typedef struct {
    const char* title;
    const char* body;
    const char* hint;
} KindText;

typedef struct {
    const char* fallback_title;
    const char* closed;
    const char* restart;
    const char* exit;
    const char* code;
    const char* module;
    const char* missing;
    const char* played;
    const char* settings;
    const char* revert;
    const char* keep;
    const char* recommended;
    const char* trial_chip;
    const char* again;
    KindText kinds[KindCount];
    KindText trial;
} Strings;

typedef struct {
    Kind kind;
    unsigned int code;
    unsigned int seconds;
    bool trial;
    char module[NAME_SIZE];
    char missing[NAME_SIZE];
} Report;

typedef struct {
    Choice choice;
    int x;
    int y;
    int width;
    const char* label;
} Button;

static const Strings STRINGS_EN = {
    "Windows game",
    "Closed with an error",
    "Restart game",
    "Exit",
    "Code",
    "Module",
    "Missing",
    "Played",
    "Settings",
    "Bring back",
    "Keep new",
    "Recommended",
    "Trial run failed",
    "The game has failed more than once in a row. You can go back to the settings Carafe built it with.",
    {
        {
            "The game stopped working",
            "The game hit a problem it couldn't recover from and had to close. Anything the game saved before this "
            "is kept.",
            "Try starting it again. If it keeps happening at the same spot, look in the settings — they have tips "
            "for this error.",
        },
        {
            "The game tried to do too much at once",
            "The Switch lets a game run only a limited number of tasks at the same time. This game needed one more "
            "and had to close.",
            "Starting the game again usually helps. If it happens every time, a newer version of Carafe may fix it.",
        },
        {
            "The game ran out of memory",
            "It asked for more memory than this game can use on the Switch and had to close.",
            "Lower the resolution or texture quality in the game's options, or look in the settings, then start it "
            "again.",
        },
        {
            "Some of the game's files are missing",
            "The game needs a Windows file that isn't in this package, so it can't start.",
            "On your computer, open the game in Carafe and build it again. Check that the game folder is complete.",
        },
        {
            "The Windows environment stopped",
            "The part of Carafe that runs Windows games on the Switch hit an error, and the game had to close.",
            "Start the game again. If it happens every time, a newer version of Carafe may fix it.",
        },
    },
    {
        "The game crashed with the new settings",
        "This was a trial run: the game closed with an error after its settings were changed. The settings it ran "
        "with before are kept.",
        "Bring them back to start the game as it worked before the change, or keep the new ones if the error does "
        "not seem to be theirs.",
    },
};

static const Strings STRINGS_RU = {
    "Игра для Windows",
    "Закрыта из-за ошибки",
    "Перезапустить",
    "Выйти",
    "Код",
    "Модуль",
    "Нет файла",
    "В игре",
    "Настройки",
    "Вернуть прежние",
    "Оставить новые",
    "Рекомендованные",
    "Пробный запуск не удался",
    "Игра падает не первый раз подряд. Можно вернуть настройки, с которыми её собрал Carafe.",
    {
        {
            "Игра перестала работать",
            "Игра столкнулась с ошибкой, с которой не смогла справиться, и закрылась. Всё, что игра сохранила "
            "до этого, на месте.",
            "Попробуйте запустить её снова. Если ошибка повторяется в одном и том же месте, загляните в настройки — "
            "там есть советы для этой ошибки.",
        },
        {
            "Игра попыталась сделать слишком много сразу",
            "Switch разрешает игре ограниченное число одновременных задач. Этой игре понадобилась ещё одна, "
            "и ей пришлось закрыться.",
            "Обычно помогает запустить игру ещё раз. Если ошибка повторяется каждый раз, загляните в настройки или "
            "дождитесь новой версии Carafe.",
        },
        {
            "Игре не хватило памяти",
            "Она запросила больше памяти, чем этой игре доступно на Switch, и закрылась.",
            "Уменьшите разрешение или качество текстур в настройках самой игры или загляните в настройки Carafe и "
            "запустите её снова.",
        },
        {
            "Не хватает файлов игры",
            "Игре нужен файл Windows, которого нет в этом пакете, поэтому она не может запуститься.",
            "Откройте игру в Carafe на компьютере и соберите её заново. Проверьте, что папка с игрой полная.",
        },
        {
            "Среда Windows остановилась",
            "Часть Carafe, которая запускает Windows-игры на Switch, столкнулась с ошибкой, и игра закрылась.",
            "Запустите игру снова. Если ошибка повторяется каждый раз, её может исправить новая версия Carafe.",
        },
    },
    {
        "Игра упала с новыми настройками",
        "Это был пробный запуск: игра закрылась из-за ошибки после того, как её настройки поменяли. Прежние "
        "настройки сохранены.",
        "Верните прежние — игра запустится так, как работала до изменений. Или оставьте новые, если ошибка, похоже, "
        "не связана с ними.",
    },
};

static const struct {
    unsigned int code;
    const char* name;
} CODE_NAMES[] = {
    { 0x00000003u, "abort" },
    { 0x80000003u, "breakpoint" },
    { 0xC0000005u, "access violation" },
    { 0xC0000017u, "no memory" },
    { 0xC000001Du, "illegal instruction" },
    { 0xC000007Bu, "bad image" },
    { 0xC0000094u, "integer divide by zero" },
    { 0xC00000FDu, "stack overflow" },
    { 0xC000012Du, "commitment limit" },
    { 0xC0000135u, "DLL not found" },
    { 0xC0000139u, "entry point not found" },
    { 0xC0000142u, "DLL init failed" },
    { 0xC0000409u, "stack buffer overrun" },
    { 0xE06D7363u, "C++ exception" },
};

extern char __end__[];
extern int wine_nx_image_at(const void* addr, void** base, char* name, size_t size) __attribute__((weak));
extern void wine_nx_runtime_trace(const char* msg) __attribute__((weak));
int __real_main(int argc, char** argv);

static u64 g_start_tick;
static unsigned int g_exception_code;
static char g_exception_module[NAME_SIZE];
static bool g_exception_seen;
static char g_missing[NAME_SIZE];
static unsigned int g_threads_refused;
static unsigned int g_native_esr;
static uintptr_t g_native_pc;
static bool g_native_seen;
static bool g_error_mode;

static const Strings* g_strings = &STRINGS_EN;
static CarafeImage g_icon;
static char g_title[NAME_SIZE];

static void trace(const char* line) {
    if (wine_nx_runtime_trace != NULL) {
        wine_nx_runtime_trace(line);
    }
    carafeOverlayLog(line);
}

static void gameExecutable(char* out, size_t size) {
    char argv[ARGV_SIZE];
    snprintf(out, size, "exe");
    if (!carafeOverlayReadRomfsText("/carafe/argv", argv, sizeof(argv))) {
        return;
    }
    char* end = NULL;
    for (char* at = argv; (at = strchr(at, '.')) != NULL; at++) {
        if (strncasecmp(at, ".exe", 4) == 0) {
            end = at + 4;
        }
    }
    if (end == NULL) {
        return;
    }
    char* start = end - 4;
    while (start > argv && start[-1] != '/' && start[-1] != '\\' && start[-1] != '"') {
        start--;
    }
    snprintf(out, size, "%.*s", (int)(end - start), start);
}

void carafeErrorException(unsigned int code, const void* address) {
    g_exception_code = code;
    g_exception_module[0] = '\0';
    void* base = NULL;
    char name[NAME_SIZE] = "";
    if (wine_nx_image_at != NULL && wine_nx_image_at(address, &base, name, sizeof(name))) {
        if (name[0] == '\0') {
            gameExecutable(name, sizeof(name));
        }
        const char* leaf = strrchr(name, '\\');
        snprintf(g_exception_module, sizeof(g_exception_module), "%s + 0x%lx", leaf != NULL ? leaf + 1 : name,
            (unsigned long)((uintptr_t)address - (uintptr_t)base));
    } else {
        snprintf(g_exception_module, sizeof(g_exception_module), "0x%lx", (unsigned long)(uintptr_t)address);
    }
    g_exception_seen = true;
}

void carafeErrorMissingDll(const char* name) {
    snprintf(g_missing, sizeof(g_missing), "%s", name);
}

void carafeErrorThreadRefused(void) {
    __atomic_add_fetch(&g_threads_refused, 1, __ATOMIC_RELAXED);
}

bool carafeErrorNative(unsigned int esr, unsigned long pc) {
    if (g_error_mode || __atomic_exchange_n(&g_native_seen, true, __ATOMIC_SEQ_CST)) {
        return false;
    }
    g_native_esr = esr;
    g_native_pc = pc;
    return true;
}

static bool exitFailed(unsigned int code) {
    return code == 3u || code >= 0x80000000u;
}

static Kind classify(unsigned int code) {
    if (g_native_seen) {
        return KindSystem;
    }
    switch (code) {
    case 0xC0000017u:
    case 0xC000012Du:
        return KindMemory;
    case 0xC000007Bu:
    case 0xC0000135u:
    case 0xC0000139u:
    case 0xC0000142u:
        return KindSetup;
    default:
        break;
    }
    return __atomic_load_n(&g_threads_refused, __ATOMIC_RELAXED) ? KindThreads : KindCrash;
}

static void describeRuntimeAddress(uintptr_t address, char* out, size_t size) {
    MemoryInfo info;
    u32 page = 0;
    if (R_SUCCEEDED(svcQueryMemory(&info, &page, (u64)(uintptr_t)&describeRuntimeAddress))
        && address >= info.addr && address < (uintptr_t)__end__) {
        snprintf(out, size, "wine-nx-runtime + 0x%lx", (unsigned long)(address - info.addr));
    } else {
        snprintf(out, size, "0x%lx", (unsigned long)address);
    }
}

static void describeModule(Kind kind, unsigned int code, char* out, size_t size) {
    out[0] = '\0';
    if (kind == KindSystem) {
        describeRuntimeAddress(g_native_pc, out, size);
        return;
    }
    if (g_exception_seen && g_exception_code == code) {
        snprintf(out, size, "%s", g_exception_module);
    }
}

static void reportPath(char* out, size_t size) {
    u64 program_id = 0;
    svcGetInfo(&program_id, InfoType_ProgramId, CUR_PROCESS_HANDLE, 0);
    snprintf(out, size, ERROR_FILE, program_id);
}

static bool writeReport(unsigned int exit_code) {
    if (!g_native_seen && (exit_code == ~0u || !exitFailed(exit_code))) {
        return false;
    }

    Kind kind = classify(exit_code);
    unsigned int code = kind == KindSystem ? g_native_esr : exit_code;
    char module[NAME_SIZE];
    describeModule(kind, exit_code, module, sizeof(module));
    unsigned int seconds = (unsigned int)(armTicksToNs(armGetSystemTick() - g_start_tick) / 1000000000ULL);

    char path[PATH_SIZE];
    reportPath(path, sizeof(path));
    FILE* file = fopen(path, "w");
    if (file == NULL) {
        trace("[CARAFE] the game failed, but its error could not be written down");
        return false;
    }
    fprintf(file, "kind=%d\ncode=%08x\nseconds=%u\n", (int)kind, code, seconds);
    if (module[0] != '\0') {
        fprintf(file, "module=%s\n", module);
    }
    if (kind == KindSetup && g_missing[0] != '\0') {
        fprintf(file, "missing=%s\n", g_missing);
    }
    if (carafeTrialActive()) {
        fprintf(file, "trial=1\n");
    }
    fclose(file);
    unsigned int failures = carafeFailuresAdd();

    char line[LINE_SIZE];
    snprintf(line, sizeof(line), "[CARAFE] the game failed (kind %d, code 0x%08x%s%s), %u in a row", (int)kind, code,
        module[0] != '\0' ? ", " : "", module, failures);
    trace(line);
    return true;
}

static bool startGameNext(const char* own_nro) {
    char argv[ARGV_SIZE];
    if (!carafeGameArgv(argv, sizeof(argv))) {
        return false;
    }
    return envHasNextLoad() && R_SUCCEEDED(envSetNextLoad(own_nro, argv));
}

bool carafeErrorRelaunch(const char* own_nro, unsigned int exit_code) {
    if (carafeRestartRequested()) {
        if (startGameNext(own_nro)) {
            trace("[CARAFE] starting the game again with the new settings");
            return true;
        }
        trace("[CARAFE] the game cannot be started again from here");
        return false;
    }
    if (!writeReport(exit_code)) {
        carafeFailuresClear();
        return false;
    }
    char argv[ARGV_SIZE];
    snprintf(argv, sizeof(argv), "%s %s", own_nro, ERROR_FLAG);
    if (!envHasNextLoad() || R_FAILED(envSetNextLoad(own_nro, argv))) {
        trace("[CARAFE] the error screen cannot be started from here; it comes with the next start");
        return false;
    }
    trace("[CARAFE] starting this program again for the error screen");
    return true;
}

bool carafeErrorRestart(unsigned int exit_code) {
    if (!carafeRestartRequested() && !writeReport(exit_code)) {
        return false;
    }
    trace(carafeRestartRequested()
            ? "[CARAFE] the loader cannot take the process back; restarting the application with the new settings"
            : "[CARAFE] the loader cannot take the process back; restarting the application for the error screen");
    Result rc = appletRestartProgram(NULL, 0);
    if (R_FAILED(rc)) {
        char line[LINE_SIZE];
        snprintf(line, sizeof(line), "[CARAFE] the application cannot restart (rc=0x%08x)", (unsigned int)rc);
        trace(line);
    }
    return R_SUCCEEDED(rc);
}

static bool readReport(Report* report) {
    char path[PATH_SIZE];
    reportPath(path, sizeof(path));
    FILE* file = fopen(path, "r");
    if (file == NULL) {
        return false;
    }
    memset(report, 0, sizeof(*report));
    char line[LINE_SIZE];
    bool kind_seen = false;
    while (fgets(line, sizeof(line), file) != NULL) {
        line[strcspn(line, "\r\n")] = '\0';
        char* value = strchr(line, '=');
        if (value == NULL) {
            continue;
        }
        *value++ = '\0';
        if (strcmp(line, "kind") == 0) {
            int kind = atoi(value);
            if (kind >= 0 && kind < KindCount) {
                report->kind = (Kind)kind;
                kind_seen = true;
            }
        } else if (strcmp(line, "code") == 0) {
            report->code = (unsigned int)strtoul(value, NULL, 16);
        } else if (strcmp(line, "seconds") == 0) {
            report->seconds = (unsigned int)strtoul(value, NULL, 10);
        } else if (strcmp(line, "module") == 0) {
            snprintf(report->module, sizeof(report->module), "%s", value);
        } else if (strcmp(line, "missing") == 0) {
            snprintf(report->missing, sizeof(report->missing), "%s", value);
        } else if (strcmp(line, "trial") == 0) {
            report->trial = atoi(value) != 0;
        }
    }
    fclose(file);
    return kind_seen;
}

static void loadTitle(void) {
    snprintf(g_title, sizeof(g_title), "%s", g_strings->fallback_title);
    char title[NAME_SIZE];
    if (carafeOverlayReadRomfsText("/carafe/title", title, sizeof(title))) {
        title[strcspn(title, "\r\n")] = '\0';
        if (title[0] != '\0') {
            snprintf(g_title, sizeof(g_title), "%s", title);
        }
    }
}

static void drawIcon(CarafeImage* picture, int left, int top) {
    if (g_icon.pixels == NULL) {
        carafeDrawRound(picture, left, top, ICON_SIZE, ICON_SIZE, ICON_RADIUS, CARAFE_COLOR_ACCENT, 255);
        char initial[8] = "";
        const unsigned char* first = (const unsigned char*)g_title;
        size_t length = *first >= 0xF0 ? 4 : *first >= 0xE0 ? 3 : *first >= 0xC0 ? 2 : 1;
        snprintf(initial, sizeof(initial), "%.*s", (int)length, g_title);
        CarafeTextStyle style = { 64, CarafeHeavy, ICON_SIZE, CARAFE_COLOR_INK, true };
        carafeDrawLine(picture, initial, &style, left, top, ICON_SIZE);
        return;
    }
    carafeDrawPicture(picture, &g_icon, left, top, ICON_SIZE, ICON_RADIUS);
}

static void codeText(const Report* report, char* out, size_t size) {
    if (report->kind == KindSystem) {
        snprintf(out, size, "ESR 0x%08X", report->code);
        return;
    }
    for (size_t i = 0; i < sizeof(CODE_NAMES) / sizeof(CODE_NAMES[0]); i++) {
        if (CODE_NAMES[i].code == report->code) {
            snprintf(out, size, report->code < 0x100 ? "%u %s" : "0x%08X %s", report->code, CODE_NAMES[i].name);
            return;
        }
    }
    snprintf(out, size, report->code < 0x100 ? "%u" : "0x%08X", report->code);
}

static bool repeated(const Report* report) {
    return !report->trial && report->kind != KindSetup && carafeFailures() >= 2 && carafeChangesExist();
}

static bool settingsAvailable(const Report* report) {
    char probe[16];
    return report->kind != KindSetup && carafeOverlayReadRomfsText("/carafe/menu.txt", probe, sizeof(probe));
}

static int layoutButtons(const Report* report, Button* buttons) {
    int count = 0;
    if (report->trial) {
        buttons[count++] = (Button){ ChoiceRevert, 0, 0, 0, g_strings->revert };
        buttons[count++] = (Button){ ChoiceKeep, 0, 0, 0, g_strings->keep };
    } else if (repeated(report)) {
        buttons[count++] = (Button){ ChoiceRecommended, 0, 0, 0, g_strings->recommended };
        if (settingsAvailable(report)) {
            buttons[count++] = (Button){ ChoiceSettings, 0, 0, 0, g_strings->settings };
        }
    } else {
        if (report->kind != KindSetup) {
            buttons[count++] = (Button){ ChoiceRestart, 0, 0, 0, g_strings->restart };
        }
        if (settingsAvailable(report)) {
            buttons[count++] = (Button){ ChoiceSettings, 0, 0, 0, g_strings->settings };
        }
    }
    buttons[count++] = (Button){ ChoiceExit, 0, 0, 0, g_strings->exit };
    int x = MARGIN;
    for (int i = 0; i < count; i++) {
        buttons[i].x = x;
        buttons[i].y = SCREEN_HEIGHT - MARGIN - BUTTON_HEIGHT;
        buttons[i].width = BUTTON_FOCUS_PAD + GLYPH_SIZE + 12 + carafeTextWidth(buttons[i].label, 22, CarafeBold)
            + BUTTON_PAD;
        x += buttons[i].width + BUTTON_GAP;
    }
    return count;
}

static void drawButton(CarafeImage* picture, const Button* button, bool focused) {
    CarafeTextStyle label = { 22, CarafeBold, BUTTON_HEIGHT, focused ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_INK,
        false };
    if (!focused) {
        carafeDrawRound(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS, CARAFE_COLOR_WHITE,
            18);
        carafeDrawRoundStroke(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS, 2,
            CARAFE_COLOR_WHITE, 36);
        label.centered = true;
        carafeDrawLine(picture, button->label, &label, button->x, button->y, button->width);
        return;
    }
    carafeDrawFocusRing(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS);
    int glyph_x = button->x + BUTTON_FOCUS_PAD;
    int glyph_y = button->y + (BUTTON_HEIGHT - GLYPH_SIZE) / 2;
    carafeDrawRound(picture, glyph_x, glyph_y, GLYPH_SIZE, GLYPH_SIZE, GLYPH_SIZE / 2, CARAFE_COLOR_PRIMARY_INK, 255);
    CarafeTextStyle glyph = { 19, CarafeHeavy, GLYPH_SIZE, CARAFE_COLOR_INK, true };
    carafeDrawLine(picture, "A", &glyph, glyph_x, glyph_y, GLYPH_SIZE);
    carafeDrawLine(picture, button->label, &label, glyph_x + GLYPH_SIZE + 12, button->y, 0);
}

static void drawCard(CarafeImage* picture, const Report* report) {
    const char* labels[MAX_ROWS];
    char values[MAX_ROWS][NAME_SIZE];
    int rows = 0;
    if (report->missing[0] != '\0') {
        labels[rows] = g_strings->missing;
        snprintf(values[rows++], NAME_SIZE, "%s", report->missing);
    }
    labels[rows] = g_strings->code;
    codeText(report, values[rows++], NAME_SIZE);
    if (report->module[0] != '\0') {
        labels[rows] = g_strings->module;
        snprintf(values[rows++], NAME_SIZE, "%s", report->module);
    }
    labels[rows] = g_strings->played;
    snprintf(values[rows++], NAME_SIZE, "%02u:%02u", report->seconds / 60, report->seconds % 60);

    const CarafeTextStyle label_style = { 17, CarafeRegular, 26, CARAFE_COLOR_LABEL, false };
    const CarafeTextStyle value_style = { 17, CarafeRegular, 26, CARAFE_COLOR_VALUE, false };
    const int inner_x = CARD_X + CARD_PAD_X;
    const int inner_width = CARD_WIDTH - 2 * CARD_PAD_X;
    int label_width = 0;
    for (int i = 0; i < rows; i++) {
        int w = carafeTextWidth(labels[i], label_style.size, label_style.weight);
        label_width = w > label_width ? w : label_width;
    }
    const int value_x = inner_x + label_width + 18;
    const int value_width = inner_width - label_width - 18;
    int details_height = 0;
    for (int i = 0; i < rows; i++) {
        details_height += carafeWrappedHeight(values[i], &value_style, value_width) + (i ? 6 : 0);
    }

    const int card_y = MARGIN;
    const int card_height = SCREEN_HEIGHT - 2 * MARGIN;
    carafeDrawRound(picture, CARD_X, card_y, CARD_WIDTH, card_height, CARD_RADIUS, CARAFE_COLOR_PANEL, 107);
    carafeDrawRoundStroke(picture, CARD_X, card_y, CARD_WIDTH, card_height, CARD_RADIUS, 1, CARAFE_COLOR_WHITE, 20);

    int y = card_y + CARD_PAD_Y;
    drawIcon(picture, CARD_X + (CARD_WIDTH - ICON_SIZE) / 2, y);
    y += ICON_SIZE + 22;
    const CarafeTextStyle name = { 26, CarafeBold, 31, CARAFE_COLOR_INK, true };
    y = carafeDrawWrapped(picture, g_title, &name, inner_x, y, inner_width) + 14;
    const char* chip_text = report->trial ? g_strings->trial_chip : g_strings->closed;
    const CarafeTextStyle chip = { 16, CarafeBold, 34, CARAFE_COLOR_CHIP, true };
    int chip_width = carafeTextWidth(chip_text, chip.size, chip.weight) + 28;
    int chip_x = CARD_X + (CARD_WIDTH - chip_width) / 2;
    carafeDrawRound(picture, chip_x, y, chip_width, chip.line_height, chip.line_height / 2, CARAFE_COLOR_FOCUS, 41);
    carafeDrawLine(picture, chip_text, &chip, chip_x, y, chip_width);

    y = card_y + card_height - CARD_PAD_Y - details_height;
    carafeDrawRound(picture, inner_x, y - 18, inner_width, 1, 0, CARAFE_COLOR_WHITE, 40);
    for (int i = 0; i < rows; i++) {
        carafeDrawLine(picture, labels[i], &label_style, inner_x, y, 0);
        y = carafeDrawWrapped(picture, values[i], &value_style, value_x, y, value_width) + 6;
    }
}

static void render(CarafeImage* picture, const Report* report, const Button* buttons, int count, int focus) {
    const KindText* text = report->trial ? &g_strings->trial : &g_strings->kinds[report->kind];
    const CarafeTextStyle title = { 40, CarafeHeavy, 46, CARAFE_COLOR_INK, false };
    const CarafeTextStyle body = { 22, CarafeRegular, 32, CARAFE_COLOR_TEXT, false };
    const CarafeTextStyle hint = { 19, CarafeRegular, 28, CARAFE_COLOR_HINT, false };
    carafeDrawBackground(picture, 255);

    int y = MARGIN;
    y = carafeDrawWrapped(picture, text->title, &title, MARGIN, y, TEXT_WIDTH) + 18;
    y = carafeDrawWrapped(picture, text->body, &body, MARGIN, y, TEXT_WIDTH) + 18;
    carafeDrawWrapped(picture, repeated(report) ? g_strings->again : text->hint, &hint, MARGIN, y, TEXT_WIDTH);

    for (int i = 0; i < count; i++) {
        drawButton(picture, &buttons[i], i == focus);
    }
    drawCard(picture, report);
}

static void present(Framebuffer* framebuffer, const CarafeImage* picture) {
    u32 stride = 0;
    uint32_t* out = framebufferBegin(framebuffer, &stride);
    if (out == NULL) {
        return;
    }
    for (int y = 0; y < picture->height; y++) {
        const uint32_t* row = picture->pixels + y * picture->width;
        uint32_t* target = (uint32_t*)((uint8_t*)out + (size_t)y * stride);
        for (int x = 0; x < picture->width; x++) {
            uint32_t argb = row[x];
            target[x] = 0xFF000000u | (argb & 0x0000FF00u) | (argb >> 16 & 0xFFu) | (argb & 0xFFu) << 16;
        }
    }
    framebufferEnd(framebuffer);
}

static int buttonAt(const Button* buttons, int count, int x, int y) {
    for (int i = 0; i < count; i++) {
        if (x >= buttons[i].x && x < buttons[i].x + buttons[i].width && y >= buttons[i].y
            && y < buttons[i].y + BUTTON_HEIGHT) {
            return i;
        }
    }
    return -1;
}

static unsigned int oskButtons(u64 held) {
    static const struct {
        u64 button;
        unsigned int osk;
    } MAP[] = {
        { HidNpadButton_Up, OSK_UP }, { HidNpadButton_Down, OSK_DOWN }, { HidNpadButton_Left, OSK_LEFT },
        { HidNpadButton_Right, OSK_RIGHT }, { HidNpadButton_A, OSK_A }, { HidNpadButton_B, OSK_B },
        { HidNpadButton_X, OSK_X }, { HidNpadButton_Y, OSK_Y }, { HidNpadButton_L, OSK_L },
        { HidNpadButton_R, OSK_R }, { HidNpadButton_ZL, OSK_ZL }, { HidNpadButton_ZR, OSK_ZR },
        { HidNpadButton_Plus, OSK_PLUS }, { HidNpadButton_Minus, OSK_MINUS },
        { HidNpadButton_StickL, OSK_STICKL }, { HidNpadButton_StickR, OSK_STICKR },
    };
    unsigned int bits = 0;
    for (size_t i = 0; i < sizeof(MAP) / sizeof(MAP[0]); i++) {
        if (held & MAP[i].button) {
            bits |= MAP[i].osk;
        }
    }
    return bits;
}

static CarafeFailure failureOf(Kind kind) {
    switch (kind) {
    case KindMemory:
        return CarafeKindMemory;
    case KindThreads:
        return CarafeKindThreads;
    case KindCrash:
    case KindSystem:
        return CarafeKindCrash;
    default:
        return CarafeKindNone;
    }
}

static bool settingsLoop(Framebuffer* framebuffer, CarafeImage* picture, PadState* pad, const Report* report) {
    if (!carafeMenuOpenError(failureOf(report->kind))) {
        return false;
    }
    char* before = calloc(1, CHANGES_SIZE);
    if (before != NULL) {
        carafeReadChangesText(before, CHANGES_SIZE);
    }
    unsigned int drawn = 0;
    bool restart = false;
    while (appletMainLoop()) {
        padUpdate(pad);
        u64 held = padGetButtons(pad);
        HidAnalogStickState stick = padGetStickPos(pad, 0);
        unsigned int bits = oskButtons(held);
        if (stick.y > 16000) {
            bits |= OSK_UP;
        } else if (stick.y < -16000) {
            bits |= OSK_DOWN;
        }
        if (stick.x > 16000) {
            bits |= OSK_RIGHT;
        } else if (stick.x < -16000) {
            bits |= OSK_LEFT;
        }
        HidTouchScreenState touch = { 0 };
        bool touched = hidGetTouchScreenStates(&touch, 1) && touch.count > 0;
        CarafeMenuAction action = carafeMenuInput(bits, touched, touched ? (int)touch.touches[0].x : 0,
            touched ? (int)touch.touches[0].y : 0, armTicksToNs(armGetSystemTick()));
        if (action == CarafeMenuBack) {
            break;
        }
        if (action == CarafeMenuRestart) {
            restart = true;
            break;
        }
        unsigned int generation = carafeMenuGeneration();
        if (generation != drawn) {
            carafeMenuRender(picture);
            drawn = generation;
        }
        present(framebuffer, picture);
    }
    carafeMenuClose();
    char* after = calloc(1, CHANGES_SIZE);
    if (restart && before != NULL && after != NULL) {
        carafeReadChangesText(after, CHANGES_SIZE);
        carafeTrialWrite(strcmp(before, after) != 0);
    }
    free(before);
    free(after);
    return restart;
}

static Choice waitForChoice(Framebuffer* framebuffer, CarafeImage* picture, const Report* report) {
    Button buttons[MAX_BUTTONS + 1];
    int count = layoutButtons(report, buttons);
    int focus = 0;
    int drawn = -1;
    bool touching = true;
    bool notified = false;
    PadState pad;
    padConfigureInput(1, HidNpadStyleSet_NpadStandard);
    padInitializeDefault(&pad);
    hidInitializeTouchScreen();

    while (appletMainLoop()) {
        padUpdate(&pad);
        u64 down = padGetButtonsDown(&pad);
        Choice chosen = ChoiceNone;
        if (down & HidNpadButton_A) {
            chosen = buttons[focus].choice;
        }
        if (down & (HidNpadButton_B | HidNpadButton_Plus)) {
            return ChoiceExit;
        }
        if ((down & HidNpadButton_AnyLeft) && focus > 0) {
            focus--;
        }
        if ((down & HidNpadButton_AnyRight) && focus < count - 1) {
            focus++;
        }
        HidTouchScreenState touch = { 0 };
        bool touched = hidGetTouchScreenStates(&touch, 1) && touch.count > 0;
        if (touched && !touching) {
            int hit = buttonAt(buttons, count, (int)touch.touches[0].x, (int)touch.touches[0].y);
            if (hit >= 0) {
                chosen = buttons[hit].choice;
            }
        }
        touching = touched;
        if (chosen == ChoiceSettings) {
            if (settingsLoop(framebuffer, picture, &pad, report)) {
                return ChoiceRestart;
            }
            drawn = -1;
            touching = true;
            continue;
        }
        if (chosen != ChoiceNone) {
            return chosen;
        }
        if (drawn != focus) {
            render(picture, report, buttons, count, focus);
            drawn = focus;
        }
        present(framebuffer, picture);
        if (!notified) {
            appletNotifyRunning(NULL);
            notified = true;
        }
    }
    return ChoiceExit;
}

static void restartGame(void) {
    char argv[ARGV_SIZE];
    if (!carafeGameArgv(argv, sizeof(argv))) {
        carafeOverlayLog("error screen: the game's arguments are not in RomFS; going back to the menu");
        return;
    }
    carafeOverlayCommit("before starting the game again");
    if (R_FAILED(envSetNextLoad(RUNTIME_NRO, argv))) {
        carafeOverlayLog("error screen: the game cannot be started again; going back to the menu");
    }
}

static void bringBack(void) {
    char* text = malloc(CHANGES_SIZE);
    if (text != NULL && carafeTrialRead(text, CHANGES_SIZE)) {
        carafeWriteChangesText(text);
        carafeOverlayLog("error screen: the settings from before the trial run are back");
    }
    free(text);
    carafeRegistryBringBack();
    carafeTrialClear();
}

static int errorScreen(void) {
    Report report;
    char path[PATH_SIZE];
    reportPath(path, sizeof(path));
    bool reported = readReport(&report);
    remove(path);
    if (!reported) {
        carafeOverlayLog("error screen: no report; going back to the menu");
        return 0;
    }

    if (carafeLanguage() == 0) {
        g_strings = &STRINGS_RU;
    }
    loadTitle();
    carafeLoadIcon(&g_icon);

    char line[LINE_SIZE * 2];
    snprintf(line, sizeof(line), "error screen: kind %d, code 0x%08x, module %s%s", (int)report.kind, report.code,
        report.module[0] != '\0' ? report.module : "-", report.trial ? ", after a trial run" : "");
    carafeOverlayLog(line);

    Framebuffer framebuffer;
    if (R_FAILED(framebufferCreate(&framebuffer, nwindowGetDefault(), SCREEN_WIDTH, SCREEN_HEIGHT,
            PIXEL_FORMAT_RGBA_8888, 2))) {
        carafeOverlayLog("error screen: no framebuffer; going back to the menu");
        return 0;
    }
    framebufferMakeLinear(&framebuffer);
    CarafeImage picture = { malloc((size_t)SCREEN_WIDTH * SCREEN_HEIGHT * 4), SCREEN_WIDTH, SCREEN_HEIGHT };
    Choice choice = ChoiceExit;
    if (picture.pixels != NULL) {
        choice = waitForChoice(&framebuffer, &picture, &report);
        free(picture.pixels);
    }
    framebufferClose(&framebuffer);

    switch (choice) {
    case ChoiceRestart:
        carafeOverlayLog("error screen: restart");
        restartGame();
        break;
    case ChoiceRevert:
        bringBack();
        restartGame();
        break;
    case ChoiceKeep:
        carafeOverlayLog("error screen: the new settings stay");
        carafeTrialClear();
        restartGame();
        break;
    case ChoiceRecommended:
        carafeOverlayLog("error screen: recommended settings");
        carafeResetAll(NULL, false);
        carafeFailuresClear();
        restartGame();
        break;
    default:
        carafeOverlayLog("error screen: exit");
        break;
    }
    return 0;
}

int __wrap_main(int argc, char** argv) {
    if (argc > 1 && strcmp(argv[argc - 1], ERROR_FLAG) == 0) {
        g_error_mode = true;
        return errorScreen();
    }
    return __real_main(argc, argv);
}

__attribute__((constructor(103))) static void carafeErrorInstall(void) {
    g_start_tick = armGetSystemTick();
}
