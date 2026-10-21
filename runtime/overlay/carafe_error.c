#include <SDL.h>
#include <SDL_ttf.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <turbojpeg.h>

#include <switch.h>

#include "carafe_error.h"
#include "carafe_overlay.h"

#define ERROR_FILE "sdmc:/switch/carafe/%016lx/carafe-error.txt"
#define PATH_SIZE 128
#define ERROR_FLAG "--carafe-error"
#define RUNTIME_NRO "sdmc:/switch/wine/wine-nx-runtime.nro"
#define SCREEN_WIDTH 1280
#define SCREEN_HEIGHT 720
#define ICON_LIMIT (512 * 1024)
#define ARGV_SIZE 2048
#define FONT_SLOTS 12
#define NAME_SIZE 96
#define LINE_SIZE 160
#define MAX_BUTTONS 2
#define MAX_ROWS 4

#define FONT_LIMIT (4 * 1024 * 1024)

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
#define FOCUS_GAP 4
#define FOCUS_OUTER 7
#define CARD_WIDTH 440
#define CARD_X (SCREEN_WIDTH - MARGIN - CARD_WIDTH)
#define CARD_RADIUS 24
#define CARD_PAD_X 36
#define CARD_PAD_Y 40

#define COLOR_INK 0xFFFFF5F8u
#define COLOR_TEXT 0xFFEBD6DDu
#define COLOR_HINT 0xFFC9A6B3u
#define COLOR_CHIP 0xFFF0A3B5u
#define COLOR_FOCUS 0xFFDB6E87u
#define COLOR_GAP 0xFF2A0D19u
#define COLOR_PRIMARY_INK 0xFF3B0F20u
#define COLOR_PANEL 0xFF12060Au
#define COLOR_LABEL 0xFFB08C99u
#define COLOR_VALUE 0xFFF4E4EAu
#define COLOR_ACCENT 0xFF8E2846u
#define COLOR_CENTER 0xFF5A1A30u
#define COLOR_MIDDLE 0xFF2A0D19u
#define COLOR_EDGE 0xFF140709u
#define COLOR_WHITE 0xFFFFFFFFu

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
    KindText kinds[KindCount];
} Strings;

typedef struct {
    Kind kind;
    unsigned int code;
    unsigned int seconds;
    char module[NAME_SIZE];
    char missing[NAME_SIZE];
} Report;

typedef struct {
    uint32_t* pixels;
    int width;
    int height;
} Image;

typedef enum {
    WeightRegular,
    WeightBold,
    WeightHeavy,
    WeightCount,
} Weight;

typedef struct {
    TTF_Font* font;
    int size;
    Weight weight;
} FontSlot;

typedef struct {
    int size;
    Weight weight;
    int line_height;
    uint32_t color;
    bool centered;
} TextStyle;

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
    {
        {
            "The game stopped working",
            "The game hit a problem it couldn't recover from and had to close. Anything the game saved before this "
            "is kept.",
            "Try starting it again. If it keeps happening at the same spot, tell us about it.",
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
            "Lower the resolution or texture quality in the game's options, then start it again.",
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
    {
        {
            "Игра перестала работать",
            "Игра столкнулась с ошибкой, с которой не смогла справиться, и закрылась. Всё, что игра сохранила "
            "до этого, на месте.",
            "Попробуйте запустить её снова. Если ошибка повторяется в одном и том же месте, сообщите нам.",
        },
        {
            "Игра попыталась сделать слишком много сразу",
            "Switch разрешает игре ограниченное число одновременных задач. Этой игре понадобилась ещё одна, "
            "и ей пришлось закрыться.",
            "Обычно помогает запустить игру ещё раз. Если ошибка повторяется каждый раз, её может исправить "
            "новая версия Carafe.",
        },
        {
            "Игре не хватило памяти",
            "Она запросила больше памяти, чем этой игре доступно на Switch, и закрылась.",
            "Уменьшите разрешение или качество текстур в настройках игры и запустите её снова.",
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
extern int wine_nx_osk_font(const void** data, size_t* size);
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
static FontSlot g_fonts[FONT_SLOTS];
static int g_font_next;
static bool g_font_failed;
static Image g_icon;
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
    fclose(file);

    char line[LINE_SIZE];
    snprintf(line, sizeof(line), "[CARAFE] the game failed (kind %d, code 0x%08x%s%s)", (int)kind, code,
        module[0] != '\0' ? ", " : "", module);
    trace(line);
    return true;
}

bool carafeErrorRelaunch(const char* own_nro, unsigned int exit_code) {
    if (!writeReport(exit_code)) {
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
    if (!writeReport(exit_code)) {
        return false;
    }
    trace("[CARAFE] the loader cannot take the process back; restarting the application for the error screen");
    Result rc = appletRestartProgram(NULL, 0);
    if (R_FAILED(rc)) {
        char line[LINE_SIZE];
        snprintf(line, sizeof(line), "[CARAFE] the application cannot restart (rc=0x%08x); the error screen comes "
            "with the next start", (unsigned int)rc);
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
        }
    }
    fclose(file);
    return kind_seen;
}

static void chooseLanguage(void) {
    if (R_FAILED(setInitialize())) {
        return;
    }
    u64 code = 0;
    SetLanguage language = SetLanguage_ENUS;
    if (R_SUCCEEDED(setGetSystemLanguage(&code)) && R_SUCCEEDED(setMakeLanguage(code, &language))
        && language == SetLanguage_RU) {
        g_strings = &STRINGS_RU;
    }
    setExit();
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

static void loadIcon(void) {
    size_t size = 0;
    unsigned char* data = carafeOverlayReadRomfsFile("/carafe/icon.jpg", ICON_LIMIT, &size);
    if (data == NULL) {
        return;
    }
    tjhandle decoder = tjInitDecompress();
    int width = 0;
    int height = 0;
    int sampling = 0;
    int space = 0;
    if (decoder != NULL && tjDecompressHeader3(decoder, data, (unsigned long)size, &width, &height, &sampling, &space) == 0
        && width > 0 && height > 0 && width <= 1024 && height <= 1024) {
        uint32_t* pixels = malloc((size_t)width * height * 4);
        if (pixels != NULL
            && tjDecompress2(decoder, data, (unsigned long)size, (unsigned char*)pixels, width, 0, height, TJPF_BGRA, 0) == 0) {
            g_icon.pixels = pixels;
            g_icon.width = width;
            g_icon.height = height;
        } else {
            free(pixels);
        }
    }
    if (decoder != NULL) {
        tjDestroy(decoder);
    }
    free(data);
}

static bool fontData(Weight weight, const void** data, size_t* bytes, bool* own) {
    static const char* const FILES[WeightCount] = {
        "/carafe/fonts/MPLUS1p-Regular.ttf",
        "/carafe/fonts/MPLUS1p-Bold.ttf",
        "/carafe/fonts/MPLUS1p-ExtraBold.ttf",
    };
    static void* loaded[WeightCount];
    static size_t sizes[WeightCount];
    static bool tried[WeightCount];
    if (!tried[weight]) {
        tried[weight] = true;
        loaded[weight] = carafeOverlayReadRomfsFile(FILES[weight], FONT_LIMIT, &sizes[weight]);
    }
    if (loaded[weight] != NULL) {
        *data = loaded[weight];
        *bytes = sizes[weight];
        *own = true;
        return true;
    }
    *own = false;
    return wine_nx_osk_font(data, bytes);
}

static TTF_Font* fontAt(int size, Weight weight) {
    for (int i = 0; i < FONT_SLOTS; i++) {
        if (g_fonts[i].font != NULL && g_fonts[i].size == size && g_fonts[i].weight == weight) {
            return g_fonts[i].font;
        }
    }
    if (g_font_failed) {
        return NULL;
    }
    if (!TTF_WasInit() && TTF_Init() != 0) {
        g_font_failed = true;
        return NULL;
    }
    const void* data = NULL;
    size_t bytes = 0;
    bool own = false;
    if (!fontData(weight, &data, &bytes, &own)) {
        g_font_failed = true;
        return NULL;
    }
    FontSlot* slot = &g_fonts[g_font_next++ % FONT_SLOTS];
    if (slot->font != NULL) {
        TTF_CloseFont(slot->font);
    }
    slot->font = TTF_OpenFontRW(SDL_RWFromConstMem(data, (int)bytes), 1, size);
    slot->size = size;
    slot->weight = weight;
    if (slot->font != NULL && !own && weight != WeightRegular) {
        TTF_SetFontStyle(slot->font, TTF_STYLE_BOLD);
    }
    return slot->font;
}

static uint32_t mix(uint32_t under, uint32_t color, unsigned int alpha) {
    uint32_t r = ((under >> 16 & 0xFF) * (255 - alpha) + (color >> 16 & 0xFF) * alpha) / 255;
    uint32_t g = ((under >> 8 & 0xFF) * (255 - alpha) + (color >> 8 & 0xFF) * alpha) / 255;
    uint32_t b = ((under & 0xFF) * (255 - alpha) + (color & 0xFF) * alpha) / 255;
    return 0xFF000000u | r << 16 | g << 8 | b;
}

static void blendPixel(Image* picture, int x, int y, uint32_t color, unsigned int alpha) {
    if (alpha == 0 || x < 0 || y < 0 || x >= picture->width || y >= picture->height) {
        return;
    }
    uint32_t* pixel = &picture->pixels[y * picture->width + x];
    *pixel = alpha >= 255 ? color : mix(*pixel, color, alpha);
}

static void drawBackground(Image* picture) {
    const float cx = SCREEN_WIDTH * 0.78f;
    const float cy = SCREEN_HEIGHT * 0.46f;
    const float reach = sqrtf(cx * cx + (SCREEN_HEIGHT - cy) * (SCREEN_HEIGHT - cy));
    for (int y = 0; y < picture->height; y++) {
        for (int x = 0; x < picture->width; x++) {
            float t = sqrtf((x - cx) * (x - cx) + (y - cy) * (y - cy)) / reach;
            unsigned int inner = (unsigned int)(t / 0.45f * 255.0f);
            unsigned int outer = (unsigned int)(fminf(fmaxf((t - 0.45f) / 0.55f, 0.0f), 1.0f) * 255.0f);
            picture->pixels[y * picture->width + x] =
                t < 0.45f ? mix(COLOR_CENTER, COLOR_MIDDLE, inner) : mix(COLOR_MIDDLE, COLOR_EDGE, outer);
        }
    }
}

static float roundCoverage(float px, float py, int x, int y, int w, int h, int radius) {
    float cx = px < x + radius ? x + radius : px > x + w - radius ? x + w - radius : px;
    float cy = py < y + radius ? y + radius : py > y + h - radius ? y + h - radius : py;
    float d = sqrtf((px - cx) * (px - cx) + (py - cy) * (py - cy));
    float coverage = radius + 0.5f - d;
    return coverage <= 0.0f ? 0.0f : coverage >= 1.0f ? 1.0f : coverage;
}

static void fillRound(Image* picture, int x, int y, int w, int h, int radius, uint32_t color, unsigned int alpha) {
    for (int py = y; py < y + h; py++) {
        for (int px = x; px < x + w; px++) {
            float coverage = roundCoverage(px + 0.5f, py + 0.5f, x, y, w, h, radius);
            blendPixel(picture, px, py, color, (unsigned int)(coverage * alpha));
        }
    }
}

static void strokeRound(Image* picture, int x, int y, int w, int h, int radius, int thickness, uint32_t color,
    unsigned int alpha) {
    int inner = radius > thickness ? radius - thickness : 0;
    for (int py = y; py < y + h; py++) {
        for (int px = x; px < x + w; px++) {
            float outer = roundCoverage(px + 0.5f, py + 0.5f, x, y, w, h, radius);
            float hole = roundCoverage(px + 0.5f, py + 0.5f, x + thickness, y + thickness, w - 2 * thickness,
                h - 2 * thickness, inner);
            float coverage = outer - hole;
            if (coverage > 0.0f) {
                blendPixel(picture, px, py, color, (unsigned int)(coverage * alpha));
            }
        }
    }
}

static int textWidth(const char* text, int size, Weight weight) {
    TTF_Font* font = fontAt(size, weight);
    int w = 0;
    int h = 0;
    if (font == NULL || text[0] == '\0' || TTF_SizeUTF8(font, text, &w, &h) != 0) {
        return 0;
    }
    return w;
}

static void drawText(Image* picture, const char* text, int size, Weight weight, int x0, int top, uint32_t color) {
    TTF_Font* font = fontAt(size, weight);
    if (font == NULL || text[0] == '\0') {
        return;
    }
    SDL_Color white = { 255, 255, 255, 255 };
    SDL_Surface* drawn = TTF_RenderUTF8_Blended(font, text, white);
    if (drawn == NULL) {
        return;
    }
    SDL_Surface* surface = SDL_ConvertSurfaceFormat(drawn, SDL_PIXELFORMAT_ARGB8888, 0);
    SDL_FreeSurface(drawn);
    if (surface == NULL) {
        return;
    }
    for (int y = 0; y < surface->h; y++) {
        const uint32_t* row = (const uint32_t*)((const uint8_t*)surface->pixels + y * surface->pitch);
        for (int x = 0; x < surface->w; x++) {
            blendPixel(picture, x0 + x, top + y, color, row[x] >> 24);
        }
    }
    SDL_FreeSurface(surface);
}

static int lineTop(int top, int size, int line_height, Weight weight) {
    TTF_Font* font = fontAt(size, weight);
    int height = font != NULL ? TTF_FontHeight(font) : size;
    return top + (line_height - height) / 2;
}

static void drawLine(Image* picture, const char* line, const TextStyle* style, int x, int top, int width) {
    int left = style->centered ? x + (width - textWidth(line, style->size, style->weight)) / 2 : x;
    drawText(picture, line, style->size, style->weight, left,
        lineTop(top, style->size, style->line_height, style->weight), style->color);
}

static int drawWrapped(Image* picture, const char* text, const TextStyle* style, int x, int top, int width) {
    char line[LINE_SIZE * 2];
    size_t used = 0;
    const char* word = text;
    while (*word != '\0') {
        const char* end = strchr(word, ' ');
        size_t length = end != NULL ? (size_t)(end - word) : strlen(word);
        char candidate[sizeof(line)];
        int written = snprintf(candidate, sizeof(candidate), "%.*s%s%.*s", (int)used, line, used ? " " : "",
            (int)length, word);
        if (used && (written >= (int)sizeof(candidate) || textWidth(candidate, style->size, style->weight) > width)) {
            line[used] = '\0';
            drawLine(picture, line, style, x, top, width);
            top += style->line_height;
            used = (size_t)snprintf(line, sizeof(line), "%.*s", (int)length, word);
        } else {
            used = (size_t)snprintf(line, sizeof(line), "%s", candidate);
        }
        word += length;
        while (*word == ' ') {
            word++;
        }
    }
    if (used) {
        line[used] = '\0';
        drawLine(picture, line, style, x, top, width);
        top += style->line_height;
    }
    return top;
}

static int wrappedHeight(const char* text, const TextStyle* style, int width) {
    Image none = { NULL, 0, 0 };
    return drawWrapped(&none, text, style, 0, 0, width);
}

static void drawIcon(Image* picture, int left, int top) {
    if (g_icon.pixels == NULL) {
        fillRound(picture, left, top, ICON_SIZE, ICON_SIZE, ICON_RADIUS, COLOR_ACCENT, 255);
        char initial[8] = "";
        const unsigned char* first = (const unsigned char*)g_title;
        size_t length = *first >= 0xF0 ? 4 : *first >= 0xE0 ? 3 : *first >= 0xC0 ? 2 : 1;
        snprintf(initial, sizeof(initial), "%.*s", (int)length, g_title);
        TextStyle style = { 64, WeightHeavy, ICON_SIZE, COLOR_INK, true };
        drawLine(picture, initial, &style, left, top, ICON_SIZE);
        return;
    }
    for (int y = 0; y < ICON_SIZE; y++) {
        const uint32_t* row = g_icon.pixels + (size_t)(y * g_icon.height / ICON_SIZE) * g_icon.width;
        for (int x = 0; x < ICON_SIZE; x++) {
            float coverage = roundCoverage(x + 0.5f, y + 0.5f, 0, 0, ICON_SIZE, ICON_SIZE, ICON_RADIUS);
            blendPixel(picture, left + x, top + y, 0xFF000000u | row[x * g_icon.width / ICON_SIZE],
                (unsigned int)(coverage * 255));
        }
    }
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

static int layoutButtons(const Report* report, Button* buttons) {
    int count = 0;
    if (report->kind != KindSetup) {
        buttons[count++] = (Button){ ChoiceRestart, 0, 0, 0, g_strings->restart };
    }
    buttons[count++] = (Button){ ChoiceExit, 0, 0, 0, g_strings->exit };
    int x = MARGIN;
    for (int i = 0; i < count; i++) {
        buttons[i].x = x;
        buttons[i].y = SCREEN_HEIGHT - MARGIN - BUTTON_HEIGHT;
        buttons[i].width = BUTTON_FOCUS_PAD + GLYPH_SIZE + 12 + textWidth(buttons[i].label, 22, WeightBold)
            + BUTTON_PAD;
        x += buttons[i].width + BUTTON_GAP;
    }
    return count;
}

static void drawButton(Image* picture, const Button* button, bool focused) {
    TextStyle label = { 22, WeightBold, BUTTON_HEIGHT, focused ? COLOR_PRIMARY_INK : COLOR_INK, false };
    if (!focused) {
        fillRound(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS, COLOR_WHITE, 18);
        strokeRound(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS, 2, COLOR_WHITE, 36);
        label.centered = true;
        drawLine(picture, button->label, &label, button->x, button->y, button->width);
        return;
    }
    fillRound(picture, button->x - FOCUS_OUTER, button->y - FOCUS_OUTER, button->width + 2 * FOCUS_OUTER,
        BUTTON_HEIGHT + 2 * FOCUS_OUTER, BUTTON_RADIUS + FOCUS_OUTER, COLOR_FOCUS, 255);
    fillRound(picture, button->x - FOCUS_GAP, button->y - FOCUS_GAP, button->width + 2 * FOCUS_GAP,
        BUTTON_HEIGHT + 2 * FOCUS_GAP, BUTTON_RADIUS + FOCUS_GAP, COLOR_GAP, 255);
    fillRound(picture, button->x, button->y, button->width, BUTTON_HEIGHT, BUTTON_RADIUS, COLOR_INK, 255);
    int glyph_x = button->x + BUTTON_FOCUS_PAD;
    int glyph_y = button->y + (BUTTON_HEIGHT - GLYPH_SIZE) / 2;
    fillRound(picture, glyph_x, glyph_y, GLYPH_SIZE, GLYPH_SIZE, GLYPH_SIZE / 2, COLOR_PRIMARY_INK, 255);
    TextStyle glyph = { 19, WeightHeavy, GLYPH_SIZE, COLOR_INK, true };
    drawLine(picture, "A", &glyph, glyph_x, glyph_y, GLYPH_SIZE);
    drawLine(picture, button->label, &label, glyph_x + GLYPH_SIZE + 12, button->y, 0);
}

static void drawCard(Image* picture, const Report* report) {
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

    const TextStyle label_style = { 17, WeightRegular, 26, COLOR_LABEL, false };
    const TextStyle value_style = { 17, WeightRegular, 26, COLOR_VALUE, false };
    const int inner_x = CARD_X + CARD_PAD_X;
    const int inner_width = CARD_WIDTH - 2 * CARD_PAD_X;
    int label_width = 0;
    for (int i = 0; i < rows; i++) {
        int w = textWidth(labels[i], label_style.size, label_style.weight);
        label_width = w > label_width ? w : label_width;
    }
    const int value_x = inner_x + label_width + 18;
    const int value_width = inner_width - label_width - 18;
    int details_height = 0;
    for (int i = 0; i < rows; i++) {
        details_height += wrappedHeight(values[i], &value_style, value_width) + (i ? 6 : 0);
    }

    const int card_y = MARGIN;
    const int card_height = SCREEN_HEIGHT - 2 * MARGIN;
    fillRound(picture, CARD_X, card_y, CARD_WIDTH, card_height, CARD_RADIUS, COLOR_PANEL, 107);
    strokeRound(picture, CARD_X, card_y, CARD_WIDTH, card_height, CARD_RADIUS, 1, COLOR_WHITE, 20);

    int y = card_y + CARD_PAD_Y;
    drawIcon(picture, CARD_X + (CARD_WIDTH - ICON_SIZE) / 2, y);
    y += ICON_SIZE + 22;
    const TextStyle name = { 26, WeightBold, 31, COLOR_INK, true };
    y = drawWrapped(picture, g_title, &name, inner_x, y, inner_width) + 14;
    const TextStyle chip = { 16, WeightBold, 34, COLOR_CHIP, true };
    int chip_width = textWidth(g_strings->closed, chip.size, chip.weight) + 28;
    int chip_x = CARD_X + (CARD_WIDTH - chip_width) / 2;
    fillRound(picture, chip_x, y, chip_width, chip.line_height, chip.line_height / 2, COLOR_FOCUS, 41);
    drawLine(picture, g_strings->closed, &chip, chip_x, y, chip_width);

    y = card_y + card_height - CARD_PAD_Y - details_height;
    fillRound(picture, inner_x, y - 18, inner_width, 1, 0, COLOR_WHITE, 40);
    for (int i = 0; i < rows; i++) {
        drawLine(picture, labels[i], &label_style, inner_x, y, 0);
        y = drawWrapped(picture, values[i], &value_style, value_x, y, value_width) + 6;
    }
}

static void render(Image* picture, const Report* report, const Button* buttons, int count, int focus) {
    const KindText* text = &g_strings->kinds[report->kind];
    const TextStyle title = { 40, WeightHeavy, 46, COLOR_INK, false };
    const TextStyle body = { 22, WeightRegular, 32, COLOR_TEXT, false };
    const TextStyle hint = { 19, WeightRegular, 28, COLOR_HINT, false };
    drawBackground(picture);

    int y = MARGIN;
    y = drawWrapped(picture, text->title, &title, MARGIN, y, TEXT_WIDTH) + 18;
    y = drawWrapped(picture, text->body, &body, MARGIN, y, TEXT_WIDTH) + 18;
    drawWrapped(picture, text->hint, &hint, MARGIN, y, TEXT_WIDTH);

    for (int i = 0; i < count; i++) {
        drawButton(picture, &buttons[i], i == focus);
    }
    drawCard(picture, report);
}

static void present(Framebuffer* framebuffer, const Image* picture) {
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
            target[x] = (argb & 0xFF00FF00u) | (argb >> 16 & 0xFFu) | (argb & 0xFFu) << 16;
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

static Choice waitForChoice(Framebuffer* framebuffer, Image* picture, const Report* report) {
    Button buttons[MAX_BUTTONS];
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
        if (down & HidNpadButton_A) {
            return buttons[focus].choice;
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
                return buttons[hit].choice;
            }
        }
        touching = touched;
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
    if (!carafeOverlayReadRomfsText("/carafe/argv", argv, sizeof(argv))) {
        carafeOverlayLog("error screen: the game's arguments are not in RomFS; going back to the menu");
        return;
    }
    argv[strcspn(argv, "\r\n")] = '\0';
    if (R_FAILED(envSetNextLoad(RUNTIME_NRO, argv))) {
        carafeOverlayLog("error screen: the game cannot be started again; going back to the menu");
    }
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

    chooseLanguage();
    loadTitle();
    loadIcon();

    char line[LINE_SIZE];
    snprintf(line, sizeof(line), "error screen: kind %d, code 0x%08x, module %s", (int)report.kind, report.code,
        report.module[0] != '\0' ? report.module : "-");
    carafeOverlayLog(line);

    Framebuffer framebuffer;
    if (R_FAILED(framebufferCreate(&framebuffer, nwindowGetDefault(), SCREEN_WIDTH, SCREEN_HEIGHT,
            PIXEL_FORMAT_RGBA_8888, 2))) {
        carafeOverlayLog("error screen: no framebuffer; going back to the menu");
        return 0;
    }
    framebufferMakeLinear(&framebuffer);
    Image picture = { malloc((size_t)SCREEN_WIDTH * SCREEN_HEIGHT * 4), SCREEN_WIDTH, SCREEN_HEIGHT };
    Choice choice = ChoiceExit;
    if (picture.pixels != NULL) {
        choice = waitForChoice(&framebuffer, &picture, &report);
        free(picture.pixels);
    }
    framebufferClose(&framebuffer);

    if (choice == ChoiceRestart) {
        carafeOverlayLog("error screen: restart");
        restartGame();
    } else {
        carafeOverlayLog("error screen: exit");
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
