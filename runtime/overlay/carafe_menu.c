#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <switch.h>

#include "carafe_menu.h"
#include "carafe_overlay.h"
#include "osk.h"

#define MENU_OPACITY 214
#define LIST_X 72
#define LIST_W 640
#define LIST_TOP 112
#define LIST_TOP_PAGES 152
#define ROW_H 42
#define ROW_GAP 2
#define LIST_BOTTOM 584
#define CARD_X 768
#define CARD_W 440
#define BUTTONS_Y 600
#define BUTTON_H 60
#define MAX_ITEMS 128
#define STRIP_MAX 4
#define MAX_HITS 160
#define REPEAT_FIRST_NS 380000000ULL
#define REPEAT_NEXT_NS 110000000ULL
#define TEXT_SIZE 256
#define CHORD_BUTTONS (HidNpadButton_Minus | HidNpadButton_Plus | HidNpadButton_L | HidNpadButton_R)

typedef enum {
    ItemSetting,
    ItemGroup,
    ItemPreset,
    ItemReset,
    ItemSuggest,
} ItemKind;

typedef struct {
    ItemKind kind;
    int index;
    int value;
} Item;

typedef enum {
    HitTab,
    HitRow,
    HitPage,
    HitButton,
    HitStrip,
    HitSource,
    HitKey,
} HitKind;

typedef enum {
    ButtonRestart,
    ButtonGame,
    ButtonBack,
    ButtonDone,
    ButtonClear,
    ButtonConfirm,
    ButtonLater,
    ButtonUndo,
    ButtonTrial,
} ButtonId;

typedef struct {
    HitKind kind;
    int value;
    int x;
    int y;
    int w;
    int h;
} Hit;

typedef struct {
    const char* back;
    const char* restart;
    const char* restart_count;
    const char* to_game;
    const char* to_error;
    const char* change;
    const char* reset;
    const char* open;
    const char* sections;
    const char* pages;
    const char* now;
    const char* after_restart;
    const char* after_restart_error;
    const char* inactive;
    const char* default_mark;
    const char* preset;
    const char* custom;
    const char* preset_hint;
    const char* reset_all;
    const char* reset_all_hint;
    const char* reset_again;
    const char* confirm_title;
    const char* confirm_body;
    const char* trial;
    const char* trial_hint;
    const char* later;
    const char* undo;
    const char* pending[3];
    const char* running;
    const char* failed;
    const char* what_to_try;
    const char* button;
    const char* done;
    const char* clear;
    const char* pick_keys;
    const char* pick_pad;
    const char* toggle;
    const char* choose;
    const char* cancel;
    const char* live_controls;
    const char* gamepad;
    const char* keyboard;
    const char* rows[10][2];
    const char* stick_press;
    const char* none;
    const char* trial_toggle;
} Strings;

static const Strings STRINGS[CARAFE_LANGUAGES] = {
    {
        "Назад", "Перезапустить", "Перезапустить · %d", "В игру", "К ошибке", "Изменить", "Сбросить", "Открыть",
        "Разделы", "Страницы", "Действует сразу — видно в игре.", "Применится после перезапуска игры.",
        "Применится, когда игра запустится снова.", "Сейчас не действует.", "по умолчанию", "Набор", "Свой",
        "Готовые сочетания опций. Можно выбрать набор и поправить опции вручную — тогда набор станет «Свой».",
        "Вернуть рекомендованные", "Все настройки этой игры станут такими, как их собрал Carafe.",
        "Нажмите A ещё раз, чтобы вернуть", "Перезапустить игру?",
        "Эти настройки начнут действовать с нового запуска. Прогресс, который игра не сохранила, пропадёт.",
        "Пробный запуск", "Если игра упадёт с новыми настройками — предложить вернуть прежние.", "Позже", "Отменить",
        { "%d изменение ждёт перезапуска", "%d изменения ждут перезапуска", "%d изменений ждут перезапуска" },
        "Игра идёт под меню · закрыть: −, +, L и R вместе", "Игра закрылась из-за ошибки", "Что попробовать",
        "Кнопка %s", "Готово", "Очистить", "Выберите клавиши и кнопки мыши. Несколько — получится сочетание.",
        "Какой кнопкой геймпада Xbox станет эта кнопка Switch.", "Добавить или убрать", "Выбрать", "Отмена",
        "Действует сразу — проверьте в игре.", "геймпад Xbox 360", "клавиатура и мышь",
        {
            { "ZL", "курок" }, { "L", "плечо" }, { "−", "минус" }, { "Левый стик", "наклон и нажатие" },
            { "Крестовина", "четыре кнопки" }, { "ZR", "курок" }, { "R", "плечо" }, { "+", "плюс" },
            { "A B X Y", "кнопки действия" }, { "Правый стик", "наклон и нажатие" },
        },
        "нажатие", "Нет", "Пробный запуск",
    },
    {
        "Back", "Restart", "Restart · %d", "Back to game", "Back", "Change", "Reset", "Open", "Sections", "Pages",
        "Takes effect now — you can see it in the game.", "Takes effect when the game restarts.",
        "Takes effect when the game starts again.", "Has no effect right now.", "default", "Preset", "Custom",
        "Ready-made sets of options. Pick one and adjust the options by hand — the preset then shows as Custom.",
        "Restore recommended", "Every setting of this game goes back to how Carafe built it.",
        "Press A again to restore", "Restart the game?",
        "These settings take effect on the next start. Progress the game has not saved will be lost.",
        "Trial run", "If the game crashes with the new settings, offer to bring back the old ones.", "Later", "Undo",
        { "%d change waits for a restart", "%d changes wait for a restart", "%d changes wait for a restart" },
        "The game keeps running · close: −, +, L and R together", "The game closed with an error", "What to try",
        "Button %s", "Done", "Clear", "Choose keys and mouse buttons. Several make a combination.",
        "Which Xbox controller button this Switch button becomes.", "Add or remove", "Choose", "Cancel",
        "Takes effect now — try it in the game.", "Xbox 360 controller", "keyboard and mouse",
        {
            { "ZL", "trigger" }, { "L", "shoulder" }, { "−", "minus" }, { "Left stick", "tilt and press" },
            { "D-pad", "four buttons" }, { "ZR", "trigger" }, { "R", "shoulder" }, { "+", "plus" },
            { "A B X Y", "action buttons" }, { "Right stick", "tilt and press" },
        },
        "press", "None", "Trial run",
    },
    {
        "返回", "重新启动", "重新启动 · %d", "回到游戏", "返回", "更改", "重置", "打开", "分区", "页面",
        "立即生效——可在游戏中看到。", "重新启动游戏后生效。", "游戏再次启动时生效。", "当前不起作用。", "默认", "预设",
        "自定义", "现成的选项组合。可以选择预设后再手动调整，此时预设显示为“自定义”。", "恢复推荐设置",
        "此游戏的所有设置恢复为 Carafe 构建时的样子。", "再按一次 A 以恢复", "重新启动游戏？",
        "这些设置将在下次启动时生效。游戏未保存的进度将会丢失。", "试运行", "如果游戏在新设置下崩溃，提供恢复旧设置的选项。",
        "稍后", "撤销", { "%d 项更改等待重新启动", "%d 项更改等待重新启动", "%d 项更改等待重新启动" },
        "游戏仍在运行 · 关闭：同时按 −、+、L 和 R", "游戏因错误而关闭", "可以尝试", "按钮 %s", "完成", "清除",
        "选择按键和鼠标按钮。选择多个即组成组合键。", "这个 Switch 按钮对应 Xbox 手柄的哪个按钮。", "添加或移除", "选择",
        "取消", "立即生效——请在游戏中试试。", "Xbox 360 手柄", "键盘和鼠标",
        {
            { "ZL", "扳机" }, { "L", "肩键" }, { "−", "减号" }, { "左摇杆", "推动与按下" }, { "十字键", "四个按钮" },
            { "ZR", "扳机" }, { "R", "肩键" }, { "+", "加号" }, { "A B X Y", "动作按钮" }, { "右摇杆", "推动与按下" },
        },
        "按下", "无", "试运行",
    },
};

typedef struct {
    int source;
    int column;
    int row;
    const char* mark;
} Cell;

typedef struct {
    int label;
    bool cross;
    Cell cells[5];
    int count;
} MapRow;

enum {
    S_A, S_B, S_X, S_Y, S_L, S_R, S_ZL, S_ZR, S_PLUS, S_MINUS, S_STICKL, S_STICKR,
    S_UP, S_DOWN, S_LEFT, S_RIGHT, S_LUP, S_LDOWN, S_LLEFT, S_LRIGHT, S_RUP, S_RDOWN, S_RLEFT, S_RRIGHT,
};

static const MapRow MAP[2][5] = {
    {
        { 0, false, { { S_ZL, 0, 0, "" } }, 1 },
        { 1, false, { { S_L, 0, 0, "" } }, 1 },
        { 2, false, { { S_MINUS, 0, 0, "" } }, 1 },
        { 3, true, { { S_LUP, 1, 0, "↑" }, { S_LLEFT, 0, 1, "←" }, { S_STICKL, 1, 1, "" }, { S_LRIGHT, 2, 1, "→" },
              { S_LDOWN, 1, 2, "↓" } }, 5 },
        { 4, true, { { S_UP, 1, 0, "↑" }, { S_LEFT, 0, 1, "←" }, { S_RIGHT, 2, 1, "→" }, { S_DOWN, 1, 2, "↓" } }, 4 },
    },
    {
        { 5, false, { { S_ZR, 0, 0, "" } }, 1 },
        { 6, false, { { S_R, 0, 0, "" } }, 1 },
        { 7, false, { { S_PLUS, 0, 0, "" } }, 1 },
        { 8, true, { { S_X, 1, 0, "X" }, { S_Y, 0, 1, "Y" }, { S_A, 2, 1, "A" }, { S_B, 1, 2, "B" } }, 4 },
        { 9, true, { { S_RUP, 1, 0, "↑" }, { S_RLEFT, 0, 1, "←" }, { S_STICKR, 1, 1, "" }, { S_RRIGHT, 2, 1, "→" },
              { S_RDOWN, 1, 2, "↓" } }, 5 },
    },
};

static pthread_mutex_t g_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t g_picture_lock = PTHREAD_MUTEX_INITIALIZER;
static bool g_open;
static bool g_error_mode;
static CarafeFailure g_failure = CarafeKindNone;
static CarafeCatalog* g_catalog;
static int g_language = 1;
static unsigned int g_generation = 1;
static unsigned int g_rendered;
static CarafeImage g_picture;
static CarafeImage g_icon;
static char g_title[128];
static int g_tab;
static int g_focus;
static int g_scroll;
static int g_group = -1;
static int g_page;
static int g_group_focus;
static int g_group_scroll;
static int g_pad_focus = STRIP_MAX;
static bool g_picker;
static int g_picker_source;
static bool g_picker_pad;
static CarafeChord g_picker_chord;
static int g_picker_focus;
static bool g_confirm;
static int g_confirm_focus;
static bool g_trial = true;
static bool g_reset_armed;
static unsigned int g_held;
static unsigned int g_repeat_bit;
static uint64_t g_repeat_at;
static bool g_touching;
static u64 g_chord_held;
static Hit g_hits[MAX_HITS];
static int g_hit_count;

static void note(const char* fmt, ...) __attribute__((format(printf, 1, 2)));

static void note(const char* fmt, ...) {
    char line[256];
    va_list args;
    va_start(args, fmt);
    vsnprintf(line, sizeof(line), fmt, args);
    va_end(args);
    carafeOverlayLog(line);
}

static const Strings* text(void) {
    return &STRINGS[g_language];
}

static const char* pick(const char* const* texts) {
    return texts[g_language][0] != '\0' ? texts[g_language] : texts[1];
}

static void changed(void) {
    g_generation++;
    if (g_generation == 0) {
        g_generation = 1;
    }
}

static int tabCount(void) {
    return g_catalog->section_count + (g_error_mode ? 1 : 0);
}

static const CarafeSection* tabSection(int tab) {
    int index = g_error_mode ? tab - 1 : tab;
    return index >= 0 && index < g_catalog->section_count ? &g_catalog->sections[index] : NULL;
}

static bool tabIsTry(int tab) {
    return g_error_mode && tab == 0;
}

static bool tabIsControls(int tab) {
    const CarafeSection* section = tabSection(tab);
    return section != NULL && strcmp(section->id, "controls") == 0;
}

static bool settingActive(const CarafeSetting* setting) {
    return carafeWhenHolds(g_catalog, &setting->when, CarafeWhenOn);
}

static bool groupVisible(const CarafeGroup* group) {
    return carafeWhenHolds(g_catalog, &group->when, CarafeWhenShow);
}

static int buildItems(Item* items) {
    int count = 0;
    if (g_group >= 0) {
        const CarafeGroup* group = &g_catalog->groups[g_group];
        if (g_page >= group->page_count) {
            return 0;
        }
        const CarafePage* page = &group->pages[g_page];
        for (int i = 0; i < page->row_count && count < MAX_ITEMS; i++) {
            const CarafeSetting* setting = &g_catalog->settings[page->rows[i].index];
            if (page->rows[i].kind == CarafeRowSetting && carafeSettingVisible(g_catalog, setting)) {
                items[count++] = (Item){ ItemSetting, page->rows[i].index, 0 };
            }
        }
        return count;
    }
    if (tabIsTry(g_tab)) {
        for (int i = 0; i < g_catalog->suggest_count && count < MAX_ITEMS; i++) {
            const CarafeSuggest* suggest = &g_catalog->suggests[i];
            if (suggest->kind == g_failure && carafeSettingVisible(g_catalog, &g_catalog->settings[suggest->setting])) {
                items[count++] = (Item){ ItemSuggest, i, suggest->value };
            }
        }
        return count;
    }
    const CarafeSection* section = tabSection(g_tab);
    if (section == NULL) {
        return 0;
    }
    if (section->preset_count > 0) {
        items[count++] = (Item){ ItemPreset, 0, 0 };
    }
    for (int i = 0; i < section->row_count && count < MAX_ITEMS; i++) {
        const CarafeRow* row = &section->rows[i];
        if (row->kind == CarafeRowSetting) {
            if (carafeSettingVisible(g_catalog, &g_catalog->settings[row->index])) {
                items[count++] = (Item){ ItemSetting, row->index, 0 };
            }
        } else if (groupVisible(&g_catalog->groups[row->index])) {
            items[count++] = (Item){ ItemGroup, row->index, 0 };
        }
    }
    if (strcmp(section->id, "system") == 0 && count < MAX_ITEMS) {
        items[count++] = (Item){ ItemReset, 0, 0 };
    }
    return count;
}

static int* focusRef(void) {
    return g_group >= 0 ? &g_group_focus : &g_focus;
}

static int* scrollRef(void) {
    return g_group >= 0 ? &g_group_scroll : &g_scroll;
}

static int defaultIndex(const CarafeSetting* setting) {
    if (setting->base >= 0 && setting->base < setting->value_count) {
        return setting->base;
    }
    for (int i = 0; i < setting->value_count; i++) {
        if (strcmp(setting->values[i].text, "-") == 0) {
            return i;
        }
    }
    return setting->default_index >= 0 && setting->default_index < setting->value_count ? setting->default_index : 0;
}

static void stepSetting(int index, int delta) {
    CarafeSetting* setting = &g_catalog->settings[index];
    if (!settingActive(setting) || setting->value_count == 0) {
        return;
    }
    int current = setting->current < 0 ? defaultIndex(setting) : setting->current;
    int next = (current + delta + setting->value_count) % setting->value_count;
    carafeSettingSet(g_catalog, index, next);
}

static void stepPreset(int delta) {
    const CarafeSection* section = tabSection(g_tab);
    if (section == NULL || section->preset_count == 0) {
        return;
    }
    int current = carafePresetCurrent(g_catalog, section);
    int next = current < 0 ? (delta > 0 ? 0 : section->preset_count - 1)
                           : (current + delta + section->preset_count) % section->preset_count;
    carafePresetApply(g_catalog, &section->presets[next]);
}

static void activateItem(const Item* item, int delta) {
    switch (item->kind) {
    case ItemSetting:
        stepSetting(item->index, delta);
        break;
    case ItemGroup:
        if (delta > 0) {
            g_group = item->index;
            g_page = 0;
            g_group_focus = 0;
            g_group_scroll = 0;
        }
        break;
    case ItemPreset:
        stepPreset(delta);
        break;
    case ItemReset:
        if (delta > 0) {
            if (g_reset_armed) {
                carafeResetAll(g_catalog, !g_error_mode);
                g_reset_armed = false;
                note("[CARAFE] settings menu: recommended settings restored");
            } else {
                g_reset_armed = true;
            }
        }
        break;
    case ItemSuggest: {
        const CarafeSuggest* suggest = &g_catalog->suggests[item->index];
        CarafeSetting* setting = &g_catalog->settings[suggest->setting];
        int target = setting->current == suggest->value ? (setting->start != suggest->value && setting->start >= 0
                                                                   ? setting->start
                                                                   : defaultIndex(setting))
                                                        : suggest->value;
        carafeSettingSet(g_catalog, suggest->setting, target);
        break;
    }
    }
}

static void resetItem(const Item* item) {
    if (item->kind == ItemSetting) {
        CarafeSetting* setting = &g_catalog->settings[item->index];
        if (settingActive(setting)) {
            carafeSettingSet(g_catalog, item->index, defaultIndex(setting));
        }
    }
}

static bool keyboardMode(void) {
    int index = carafeCatalogFindKey(g_catalog, CarafeFileKeys, "input-mode");
    if (index < 0) {
        return false;
    }
    const CarafeSetting* setting = &g_catalog->settings[index];
    return setting->current >= 0 && strcmp(setting->values[setting->current].text, "2") == 0;
}

static int stripItems(int* indices) {
    int count = 0;
    const CarafeSection* section = tabSection(g_tab);
    for (int i = 0; section != NULL && i < section->row_count && count < 4; i++) {
        if (section->rows[i].kind == CarafeRowSetting
            && carafeSettingVisible(g_catalog, &g_catalog->settings[section->rows[i].index])) {
            indices[count++] = section->rows[i].index;
        }
    }
    return count;
}

static int columnSources(int column, int* out) {
    int count = 0;
    for (int r = 0; r < 5; r++) {
        for (int c = 0; c < MAP[column][r].count; c++) {
            out[count++] = MAP[column][r].cells[c].source;
        }
    }
    return count;
}

static CarafeChord* chordOf(int source, bool pad) {
    return pad ? &g_catalog->pad_current[source] : &g_catalog->key_current[source];
}

static void openPicker(int source) {
    g_picker = true;
    g_picker_source = source;
    g_picker_pad = !keyboardMode();
    g_picker_chord = *chordOf(source, g_picker_pad);
    g_picker_focus = 0;
}

static int pickerCount(void) {
    return g_picker_pad ? g_catalog->pad_count : g_catalog->key_count;
}

static unsigned int pickerCode(int index) {
    return g_picker_pad ? g_catalog->pads[index].code : g_catalog->keys[index].code;
}

static void pickerToggle(int index) {
    if (index < 0 || index >= pickerCount()) {
        return;
    }
    unsigned int code = pickerCode(index);
    if (g_picker_pad) {
        g_picker_chord.count = 1;
        g_picker_chord.code[0] = (uint16_t)code;
        return;
    }
    for (int i = 0; i < g_picker_chord.count; i++) {
        if (g_picker_chord.code[i] == code) {
            memmove(&g_picker_chord.code[i], &g_picker_chord.code[i + 1],
                (size_t)(g_picker_chord.count - i - 1) * sizeof(uint16_t));
            g_picker_chord.count--;
            return;
        }
    }
    if (g_picker_chord.count < CARAFE_CHORD_MAX) {
        g_picker_chord.code[g_picker_chord.count++] = (uint16_t)code;
    }
}

static void pickerDone(void) {
    carafeBindingSet(g_catalog, g_picker_pad, g_picker_source, &g_picker_chord);
    g_picker = false;
}

static void closeLocked(void) {
    if (!g_open) {
        return;
    }
    g_open = false;
    carafeCatalogFree(g_catalog);
    g_catalog = NULL;
    pthread_mutex_lock(&g_picture_lock);
    free(g_picture.pixels);
    g_picture.pixels = NULL;
    pthread_mutex_unlock(&g_picture_lock);
    free(g_icon.pixels);
    g_icon.pixels = NULL;
    carafeDrawRelease();
    g_rendered = 0;
    changed();
    note("[CARAFE] settings menu closed");
}

static bool openLocked(bool error_mode) {
    if (g_open) {
        return true;
    }
    g_catalog = carafeCatalogLoad();
    if (g_catalog == NULL) {
        return false;
    }
    g_language = carafeLanguage();
    g_error_mode = error_mode;
    g_tab = error_mode && g_failure != CarafeKindNone ? 0 : (error_mode ? 1 : 0);
    g_focus = g_scroll = 0;
    g_group = -1;
    g_picker = false;
    g_confirm = false;
    g_reset_armed = false;
    g_pad_focus = STRIP_MAX;
    g_held = error_mode ? OSK_A : OSK_MINUS | OSK_PLUS | OSK_L | OSK_R;
    g_repeat_bit = 0;
    g_touching = true;
    if (!carafeOverlayReadRomfsText("/carafe/title", g_title, sizeof(g_title))) {
        snprintf(g_title, sizeof(g_title), "Windows");
    }
    g_title[strcspn(g_title, "\r\n")] = '\0';
    carafeLoadIcon(&g_icon);
    g_open = true;
    changed();
    note("[CARAFE] settings menu opened%s", error_mode ? " from the error screen" : "");
    return true;
}

static void moveFocus(int delta, int count) {
    int* focus = focusRef();
    if (count <= 0) {
        *focus = 0;
        return;
    }
    *focus += delta;
    if (*focus < 0) {
        *focus = 0;
    }
    if (*focus >= count) {
        *focus = count - 1;
    }
    g_reset_armed = false;
}

static void switchTab(int delta) {
    if (g_group >= 0) {
        const CarafeGroup* group = &g_catalog->groups[g_group];
        if (group->page_count > 1) {
            g_page = (g_page + delta + group->page_count) % group->page_count;
            g_group_focus = 0;
            g_group_scroll = 0;
        }
        return;
    }
    int count = tabCount();
    g_tab = (g_tab + delta + count) % count;
    g_focus = 0;
    g_scroll = 0;
    g_pad_focus = STRIP_MAX;
    g_reset_armed = false;
}

static CarafeMenuAction pressConfirm(unsigned int down) {
    if (down & OSK_B) {
        g_confirm = false;
        return CarafeMenuStay;
    }
    if (down & OSK_X) {
        g_trial = !g_trial;
    }
    if (down & OSK_LEFT && g_confirm_focus > 0) {
        g_confirm_focus--;
    }
    if (down & OSK_RIGHT && g_confirm_focus < 2) {
        g_confirm_focus++;
    }
    if (down & OSK_A) {
        g_confirm = false;
        if (g_confirm_focus == 0) {
            return CarafeMenuRestart;
        }
        if (g_confirm_focus == 1) {
            return CarafeMenuBack;
        }
        carafeUndoPending(g_catalog);
    }
    return CarafeMenuStay;
}

static CarafeMenuAction pressPicker(unsigned int down) {
    const int columns = g_picker_pad ? 4 : 8;
    int count = pickerCount();
    if (down & OSK_B) {
        g_picker = false;
    } else if (down & OSK_PLUS) {
        pickerDone();
    } else if (down & OSK_Y) {
        g_picker_chord.count = 0;
    } else if (down & OSK_A) {
        pickerToggle(g_picker_focus);
        if (g_picker_pad) {
            pickerDone();
        }
    } else if (down & OSK_LEFT && g_picker_focus > 0) {
        g_picker_focus--;
    } else if (down & OSK_RIGHT && g_picker_focus < count - 1) {
        g_picker_focus++;
    } else if (down & OSK_UP && g_picker_focus >= columns) {
        g_picker_focus -= columns;
    } else if (down & OSK_DOWN && g_picker_focus + columns < count) {
        g_picker_focus += columns;
    }
    return CarafeMenuStay;
}

static void pressPad(unsigned int down) {
    int strip[4];
    int strips = stripItems(strip);
    if (g_pad_focus < STRIP_MAX) {
        if (g_pad_focus >= strips) {
            g_pad_focus = strips > 0 ? strips - 1 : STRIP_MAX;
        }
        if (down & OSK_LEFT && g_pad_focus > 0) {
            g_pad_focus--;
        } else if (down & OSK_RIGHT && g_pad_focus < strips - 1) {
            g_pad_focus++;
        } else if (down & OSK_DOWN) {
            g_pad_focus = STRIP_MAX;
        } else if (down & OSK_A) {
            stepSetting(strip[g_pad_focus], 1);
        } else if (down & OSK_Y) {
            Item item = { ItemSetting, strip[g_pad_focus], 0 };
            resetItem(&item);
        }
        return;
    }
    int index = g_pad_focus - STRIP_MAX;
    int left[16];
    int right[16];
    int left_count = columnSources(0, left);
    int right_count = columnSources(1, right);
    int column = index >= left_count ? 1 : 0;
    int position = column ? index - left_count : index;
    if (down & OSK_UP) {
        if (position == 0) {
            g_pad_focus = 0;
            return;
        }
        position--;
    } else if (down & OSK_DOWN) {
        position++;
    } else if (down & (OSK_LEFT | OSK_RIGHT)) {
        column = down & OSK_LEFT ? 0 : 1;
    } else if (down & OSK_A) {
        openPicker(column ? right[position] : left[position]);
        return;
    }
    int limit = column ? right_count : left_count;
    if (position >= limit) {
        position = limit - 1;
    }
    g_pad_focus = STRIP_MAX + (column ? left_count + position : position);
}

static CarafeMenuAction pressList(unsigned int down) {
    Item items[MAX_ITEMS];
    int count = buildItems(items);
    int* focus = focusRef();
    if (*focus >= count) {
        *focus = count > 0 ? count - 1 : 0;
    }
    if (down & OSK_UP) {
        moveFocus(-1, count);
    } else if (down & OSK_DOWN) {
        moveFocus(1, count);
    } else if (count > 0 && down & OSK_A) {
        activateItem(&items[*focus], 1);
    } else if (count > 0 && down & OSK_RIGHT && items[*focus].kind != ItemGroup && items[*focus].kind != ItemReset) {
        activateItem(&items[*focus], 1);
    } else if (count > 0 && down & OSK_LEFT && items[*focus].kind != ItemGroup && items[*focus].kind != ItemReset) {
        activateItem(&items[*focus], -1);
    } else if (count > 0 && down & OSK_Y) {
        resetItem(&items[*focus]);
    }
    return CarafeMenuStay;
}

static CarafeMenuAction pressKeys(unsigned int down) {
    if (g_confirm) {
        return pressConfirm(down);
    }
    if (g_picker) {
        return pressPicker(down);
    }
    if (down & OSK_B) {
        if (g_group >= 0) {
            g_group = -1;
            return CarafeMenuStay;
        }
        return CarafeMenuBack;
    }
    if (down & OSK_PLUS) {
        if (g_error_mode) {
            return CarafeMenuRestart;
        }
        if (carafePendingCount(g_catalog) > 0) {
            g_confirm = true;
            g_confirm_focus = 0;
        }
        return CarafeMenuStay;
    }
    if (down & OSK_L) {
        switchTab(-1);
        return CarafeMenuStay;
    }
    if (down & OSK_R) {
        switchTab(1);
        return CarafeMenuStay;
    }
    if (g_group < 0 && tabIsControls(g_tab)) {
        pressPad(down);
        return CarafeMenuStay;
    }
    return pressList(down);
}

static int hitAt(int x, int y) {
    for (int i = g_hit_count - 1; i >= 0; i--) {
        const Hit* hit = &g_hits[i];
        if (x >= hit->x && x < hit->x + hit->w && y >= hit->y && y < hit->y + hit->h) {
            return i;
        }
    }
    return -1;
}

static CarafeMenuAction pressButton(int id) {
    switch (id) {
    case ButtonRestart:
        return pressKeys(OSK_PLUS);
    case ButtonGame:
    case ButtonBack:
        return CarafeMenuBack;
    case ButtonDone:
        pickerDone();
        return CarafeMenuStay;
    case ButtonClear:
        g_picker_chord.count = 0;
        return CarafeMenuStay;
    case ButtonConfirm:
        g_confirm = false;
        return CarafeMenuRestart;
    case ButtonLater:
        g_confirm = false;
        return CarafeMenuBack;
    case ButtonUndo:
        g_confirm = false;
        carafeUndoPending(g_catalog);
        return CarafeMenuStay;
    case ButtonTrial:
        g_trial = !g_trial;
        return CarafeMenuStay;
    }
    return CarafeMenuStay;
}

static CarafeMenuAction tap(int x, int y) {
    int index = hitAt(x, y);
    if (index < 0) {
        return CarafeMenuStay;
    }
    Hit hit = g_hits[index];
    switch (hit.kind) {
    case HitTab:
        if (g_group < 0 && !g_picker && !g_confirm) {
            g_tab = hit.value;
            g_focus = g_scroll = 0;
            g_pad_focus = STRIP_MAX;
        }
        break;
    case HitPage:
        g_page = hit.value;
        g_group_focus = g_group_scroll = 0;
        break;
    case HitRow: {
        Item items[MAX_ITEMS];
        int count = buildItems(items);
        if (hit.value < count) {
            if (*focusRef() == hit.value) {
                activateItem(&items[hit.value], 1);
            } else {
                *focusRef() = hit.value;
            }
        }
        break;
    }
    case HitButton:
        return pressButton(hit.value);
    case HitStrip: {
        int strip[4];
        int strips = stripItems(strip);
        if (hit.value < strips) {
            if (g_pad_focus == hit.value) {
                stepSetting(strip[hit.value], 1);
            }
            g_pad_focus = hit.value;
        }
        break;
    }
    case HitSource:
        if (g_pad_focus == hit.value) {
            int left[16];
            int right[16];
            int left_count = columnSources(0, left);
            columnSources(1, right);
            int position = hit.value - STRIP_MAX;
            openPicker(position < left_count ? left[position] : right[position - left_count]);
        } else {
            g_pad_focus = hit.value;
        }
        break;
    case HitKey:
        g_picker_focus = hit.value;
        pickerToggle(hit.value);
        if (g_picker_pad) {
            pickerDone();
        }
        break;
    }
    return CarafeMenuStay;
}

CarafeMenuAction carafeMenuInput(unsigned int held, int touching, int touch_x, int touch_y, uint64_t now_ns) {
    pthread_mutex_lock(&g_lock);
    if (!g_open || g_catalog == NULL) {
        pthread_mutex_unlock(&g_lock);
        return CarafeMenuStay;
    }
    unsigned int down = held & ~g_held;
    const unsigned int repeats = OSK_UP | OSK_DOWN | OSK_LEFT | OSK_RIGHT;
    if (down & repeats) {
        g_repeat_bit = down & repeats;
        g_repeat_at = now_ns + REPEAT_FIRST_NS;
    } else if (g_repeat_bit && (held & g_repeat_bit) == g_repeat_bit && now_ns >= g_repeat_at) {
        down |= g_repeat_bit;
        g_repeat_at = now_ns + REPEAT_NEXT_NS;
    } else if ((held & g_repeat_bit) != g_repeat_bit) {
        g_repeat_bit = 0;
    }
    g_held = held;
    CarafeMenuAction action = CarafeMenuStay;
    if (down != 0) {
        action = pressKeys(down);
        changed();
    }
    if (touching && !g_touching) {
        CarafeMenuAction tapped = tap(touch_x, touch_y);
        if (tapped != CarafeMenuStay) {
            action = tapped;
        }
        changed();
    }
    g_touching = touching;
    pthread_mutex_unlock(&g_lock);
    return action;
}

static void drawGlyph(CarafeImage* picture, int x, int y, const char* label, uint32_t fill, uint32_t ink) {
    int width = carafeTextWidth(label, 12, CarafeHeavy);
    int w = width + 12 < 24 ? 24 : width + 12;
    carafeDrawRound(picture, x, y, w, 24, 12, fill, 255);
    CarafeTextStyle style = { 12, CarafeHeavy, 24, ink, true };
    carafeDrawLine(picture, label, &style, x, y, w);
}

static int drawHint(CarafeImage* picture, int x, int y, const char* key, const char* label) {
    drawGlyph(picture, x, y, key, CARAFE_COLOR_PRIMARY_INK, CARAFE_COLOR_INK);
    int glyph = carafeTextWidth(key, 12, CarafeHeavy) + 12;
    glyph = glyph < 24 ? 24 : glyph;
    CarafeTextStyle style = { 14, CarafeRegular, 24, CARAFE_COLOR_HINT, false };
    carafeDrawLine(picture, label, &style, x + glyph + 6, y, 0);
    return x + glyph + 6 + carafeTextWidth(label, 14, CarafeRegular) + 18;
}

static void addHit(HitKind kind, int value, int x, int y, int w, int h) {
    if (g_hit_count < MAX_HITS) {
        g_hits[g_hit_count++] = (Hit){ kind, value, x, y, w, h };
    }
}

static int drawButtonAt(CarafeImage* picture, int x, const char* glyph, const char* label, int style, ButtonId id) {
    int glyph_w = glyph != NULL ? 34 + 12 : 0;
    int width = (glyph != NULL ? 14 : 30) + glyph_w + carafeTextWidth(label, 22, CarafeBold) + 30;
    uint32_t fill = style == 1 ? CARAFE_COLOR_INK : style == 2 ? CARAFE_COLOR_RESTART : CARAFE_COLOR_WHITE;
    uint32_t ink = style == 1 ? CARAFE_COLOR_PRIMARY_INK : style == 2 ? CARAFE_COLOR_RESTART_INK : CARAFE_COLOR_INK;
    if (style == 0) {
        carafeDrawRound(picture, x, BUTTONS_Y, width, BUTTON_H, 16, CARAFE_COLOR_WHITE, 18);
        carafeDrawRoundStroke(picture, x, BUTTONS_Y, width, BUTTON_H, 16, 2, CARAFE_COLOR_WHITE, 36);
    } else {
        carafeDrawRound(picture, x, BUTTONS_Y, width, BUTTON_H, 16, fill, 255);
    }
    int text_x = x + (glyph != NULL ? 14 : 30);
    if (glyph != NULL) {
        uint32_t glyph_fill = style == 2 ? CARAFE_COLOR_RESTART_INK : CARAFE_COLOR_PRIMARY_INK;
        uint32_t glyph_ink = style == 2 ? CARAFE_COLOR_RESTART : CARAFE_COLOR_INK;
        carafeDrawCircle(picture, text_x + 17, BUTTONS_Y + BUTTON_H / 2, 17, glyph_fill, 255);
        CarafeTextStyle g = { 19, CarafeHeavy, 34, glyph_ink, true };
        carafeDrawLine(picture, glyph, &g, text_x, BUTTONS_Y + (BUTTON_H - 34) / 2, 34);
        text_x += glyph_w;
    }
    CarafeTextStyle s = { 22, CarafeBold, BUTTON_H, ink, false };
    carafeDrawLine(picture, label, &s, text_x, BUTTONS_Y, 0);
    addHit(HitButton, id, x, BUTTONS_Y, width, BUTTON_H);
    return x + width + 16;
}

static void drawHeader(CarafeImage* picture) {
    const Strings* s = text();
    int x = LIST_X;
    if (g_icon.pixels != NULL) {
        carafeDrawPicture(picture, &g_icon, x, 36, 48, 12);
    } else {
        carafeDrawRound(picture, x, 36, 48, 48, 12, CARAFE_COLOR_ACCENT, 255);
    }
    x += 64;
    CarafeTextStyle name = { 21, CarafeBold, 26, CARAFE_COLOR_INK, false };
    carafeDrawLine(picture, g_title, &name, x, 38, 0);
    char subline[TEXT_SIZE];
    int pending = carafePendingCount(g_catalog);
    if (g_error_mode) {
        snprintf(subline, sizeof(subline), "%s", s->failed);
    } else if (pending > 0) {
        int form = g_language != 0 ? (pending == 1 ? 0 : 1)
                   : pending % 10 == 1 && pending % 100 != 11 ? 0
                   : pending % 10 >= 2 && pending % 10 <= 4 && (pending % 100 < 12 || pending % 100 > 14) ? 1 : 2;
        snprintf(subline, sizeof(subline), s->pending[form], pending);
    } else {
        snprintf(subline, sizeof(subline), "%s", s->running);
    }
    CarafeTextStyle sub = { 14, CarafeRegular, 20, pending > 0 && !g_error_mode ? CARAFE_COLOR_RESTART : CARAFE_COLOR_LABEL,
        false };
    carafeDrawLine(picture, subline, &sub, x, 64, 0);

    if (g_group >= 0 || g_picker) {
        int bx = 1208;
        const CarafeSection* section = tabSection(g_tab);
        const char* parent = section != NULL ? pick(section->name) : "";
        char title[TEXT_SIZE];
        if (g_picker) {
            snprintf(title, sizeof(title), s->button, carafe_source_names[g_picker_source]);
        } else {
            snprintf(title, sizeof(title), "%s", pick(g_catalog->groups[g_group].name));
        }
        int title_w = carafeTextWidth(title, 20, CarafeBold);
        bx -= title_w;
        carafeDrawText(picture, title, 20, CarafeBold, bx, 45, CARAFE_COLOR_INK);
        char crumb[TEXT_SIZE];
        snprintf(crumb, sizeof(crumb), "%s ›", parent);
        int crumb_w = carafeTextWidth(crumb, 17, CarafeRegular);
        bx -= crumb_w + 10;
        carafeDrawText(picture, crumb, 17, CarafeRegular, bx, 48, CARAFE_COLOR_HINT);
        int back_w = 24 + 8 + carafeTextWidth(s->back, 15, CarafeBold) + 18;
        bx -= back_w + 14;
        carafeDrawRound(picture, bx, 42, back_w, 36, 12, CARAFE_COLOR_WHITE, 18);
        drawGlyph(picture, bx + 6, 48, "B", CARAFE_COLOR_PRIMARY_INK, CARAFE_COLOR_INK);
        carafeDrawText(picture, s->back, 15, CarafeBold, bx + 38, 50, CARAFE_COLOR_INK);
        addHit(HitButton, ButtonBack, bx, 42, back_w, 36);
        return;
    }
    int count = tabCount();
    int widths[16];
    int total = 34 + 8 + 34;
    for (int i = 0; i < count && i < 16; i++) {
        const char* label = tabIsTry(i) ? s->what_to_try : pick(tabSection(i)->name);
        widths[i] = carafeTextWidth(label, 16, CarafeBold) + 28;
        total += widths[i] + 4;
    }
    int tx = 1208 - total;
    carafeDrawRoundStroke(picture, tx, 43, 34, 26, 8, 2, CARAFE_COLOR_DIM, 255);
    CarafeTextStyle shoulder = { 13, CarafeHeavy, 26, CARAFE_COLOR_HINT, true };
    carafeDrawLine(picture, "L", &shoulder, tx, 43, 34);
    tx += 34 + 4;
    for (int i = 0; i < count && i < 16; i++) {
        const char* label = tabIsTry(i) ? s->what_to_try : pick(tabSection(i)->name);
        bool on = i == g_tab;
        if (on) {
            carafeDrawRound(picture, tx, 36, widths[i], 40, 12, CARAFE_COLOR_FOCUS, 255);
        }
        CarafeTextStyle style = { 16, CarafeBold, 40, on ? 0xFF22121Au : CARAFE_COLOR_HINT, true };
        carafeDrawLine(picture, label, &style, tx, 36, widths[i]);
        addHit(HitTab, i, tx, 36, widths[i], 40);
        tx += widths[i] + 4;
    }
    carafeDrawRoundStroke(picture, tx, 43, 34, 26, 8, 2, CARAFE_COLOR_DIM, 255);
    carafeDrawLine(picture, "R", &shoulder, tx, 43, 34);
}

static const char* itemName(const Item* item) {
    switch (item->kind) {
    case ItemSetting:
        return pick(g_catalog->settings[item->index].name);
    case ItemGroup:
        return pick(g_catalog->groups[item->index].name);
    case ItemPreset:
        return text()->preset;
    case ItemReset:
        return text()->reset_all;
    case ItemSuggest:
        return pick(g_catalog->settings[g_catalog->suggests[item->index].setting].name);
    }
    return "";
}

static void itemValue(const Item* item, char* out, size_t size) {
    out[0] = '\0';
    switch (item->kind) {
    case ItemSetting:
        snprintf(out, size, "%s", carafeValueLabel(&g_catalog->settings[item->index], g_language));
        break;
    case ItemGroup:
        snprintf(out, size, "›");
        break;
    case ItemPreset: {
        const CarafeSection* section = tabSection(g_tab);
        int current = section != NULL ? carafePresetCurrent(g_catalog, section) : -1;
        snprintf(out, size, "%s", current >= 0 ? pick(section->presets[current].name) : text()->custom);
        break;
    }
    case ItemReset:
        snprintf(out, size, "›");
        break;
    case ItemSuggest:
        snprintf(out, size, "%s", carafeValueLabel(&g_catalog->settings[g_catalog->suggests[item->index].setting],
            g_language));
        break;
    }
}

static bool itemChanged(const Item* item) {
    if (item->kind == ItemSetting || item->kind == ItemSuggest) {
        int index = item->kind == ItemSetting ? item->index : g_catalog->suggests[item->index].setting;
        const CarafeSetting* setting = &g_catalog->settings[index];
        return setting->current != defaultIndex(setting);
    }
    return false;
}

static bool itemDimmed(const Item* item) {
    if (item->kind == ItemSetting) {
        return !settingActive(&g_catalog->settings[item->index]);
    }
    return false;
}

static void drawRow(CarafeImage* picture, int index, const Item* item, int y, bool focused) {
    char value[TEXT_SIZE];
    itemValue(item, value, sizeof(value));
    bool dim = itemDimmed(item);
    if (focused) {
        carafeDrawFocusRing(picture, LIST_X, y, LIST_W, ROW_H, 14);
    }
    uint32_t ink = focused ? CARAFE_COLOR_PRIMARY_INK : dim ? CARAFE_COLOR_DIM : CARAFE_COLOR_VALUE;
    if (itemChanged(item)) {
        carafeDrawCircle(picture, LIST_X + 18, y + ROW_H / 2, 4, CARAFE_COLOR_FOCUS, 255);
    }
    int value_w = carafeTextWidth(value, 16, CarafeBold);
    CarafeTextStyle name = { 16, CarafeRegular, ROW_H, ink, false };
    carafeDrawLine(picture, itemName(item), &name, LIST_X + 32, y, 0);
    CarafeTextStyle val = { 16, CarafeBold, ROW_H, focused ? CARAFE_COLOR_PRIMARY_INK : dim ? CARAFE_COLOR_DIM
                                                                                         : CARAFE_COLOR_INK,
        false };
    carafeDrawLine(picture, value, &val, LIST_X + LIST_W - 16 - value_w, y, 0);
    addHit(HitRow, index, LIST_X, y, LIST_W, ROW_H);
}

static void drawCardSetting(CarafeImage* picture, const CarafeSetting* setting, const char* why, int top, int bottom) {
    const Strings* s = text();
    int x = CARD_X + 30;
    int width = CARD_W - 60;
    CarafeTextStyle title = { 24, CarafeHeavy, 29, CARAFE_COLOR_INK, false };
    int y = carafeDrawWrapped(picture, pick(setting->name), &title, x, top + 26, width) + 8;
    CarafeTextStyle body = { 16, CarafeRegular, 24, CARAFE_COLOR_TEXT, false };
    if (why != NULL && why[0] != '\0') {
        CarafeTextStyle reason = { 16, CarafeBold, 24, CARAFE_COLOR_CHIP, false };
        y = carafeDrawWrapped(picture, why, &reason, x, y, width) + 4;
    }
    y = carafeDrawWrapped(picture, pick(setting->hint), &body, x, y, width);
    if (!settingActive(setting)) {
        CarafeTextStyle off = { 16, CarafeRegular, 24, CARAFE_COLOR_LABEL, false };
        y = carafeDrawWrapped(picture, s->inactive, &off, x, y, width);
    }
    y += 10;
    int def = defaultIndex(setting);
    int limit = bottom - 64;
    for (int i = 0; i < setting->value_count && y + 34 <= limit; i++) {
        bool current = i == setting->current;
        if (current) {
            carafeDrawRound(picture, x - 4, y, width + 8, 32, 10, CARAFE_COLOR_FOCUS, 41);
            carafeDrawRing(picture, x + 10, y + 16, 8, 5, CARAFE_COLOR_FOCUS, 255);
        } else {
            carafeDrawRing(picture, x + 10, y + 16, 8, 2, CARAFE_COLOR_DIM, 255);
        }
        CarafeTextStyle option = { 15, current ? CarafeBold : CarafeRegular, 32,
            current ? CARAFE_COLOR_INK : CARAFE_COLOR_TEXT, false };
        const char* label = pick(setting->values[i].label);
        carafeDrawLine(picture, label, &option, x + 28, y, 0);
        if (i == def) {
            int w = carafeTextWidth(s->default_mark, 12, CarafeRegular);
            carafeDrawText(picture, s->default_mark, 12, CarafeRegular, x + width - w, y + 8, CARAFE_COLOR_CHIP);
        }
        y += 34;
    }
    const char* mode = setting->now ? s->now : g_error_mode ? s->after_restart_error : s->after_restart;
    uint32_t color = setting->now ? CARAFE_COLOR_LIVE : CARAFE_COLOR_RESTART;
    int mode_h = carafeWrappedHeight(mode, &body, width - 28) + 16;
    carafeDrawRound(picture, x - 4, bottom - 26 - mode_h, width + 8, mode_h, 12, color, 30);
    CarafeTextStyle mode_style = { 14, CarafeRegular, 20, color, false };
    carafeDrawWrapped(picture, mode, &mode_style, x + 10, bottom - 26 - mode_h + 8, width - 28);
}

static void drawCard(CarafeImage* picture, const Item* item, int top) {
    int bottom = LIST_BOTTOM;
    carafeDrawRound(picture, CARD_X, top, CARD_W, bottom - top, 24, CARAFE_COLOR_PANEL, 128);
    carafeDrawRoundStroke(picture, CARD_X, top, CARD_W, bottom - top, 24, 1, CARAFE_COLOR_WHITE, 20);
    if (item == NULL) {
        return;
    }
    const Strings* s = text();
    int x = CARD_X + 30;
    int width = CARD_W - 60;
    if (item->kind == ItemSetting) {
        drawCardSetting(picture, &g_catalog->settings[item->index], NULL, top, bottom);
        return;
    }
    if (item->kind == ItemSuggest) {
        const CarafeSuggest* suggest = &g_catalog->suggests[item->index];
        drawCardSetting(picture, &g_catalog->settings[suggest->setting], pick(suggest->why), top, bottom);
        return;
    }
    CarafeTextStyle title = { 24, CarafeHeavy, 29, CARAFE_COLOR_INK, false };
    CarafeTextStyle body = { 16, CarafeRegular, 24, CARAFE_COLOR_TEXT, false };
    int y = carafeDrawWrapped(picture, itemName(item), &title, x, top + 26, width) + 8;
    if (item->kind == ItemPreset) {
        const CarafeSection* section = tabSection(g_tab);
        y = carafeDrawWrapped(picture, s->preset_hint, &body, x, y, width) + 10;
        int current = carafePresetCurrent(g_catalog, section);
        for (int i = 0; i < section->preset_count; i++) {
            bool on = i == current;
            if (on) {
                carafeDrawRound(picture, x - 4, y, width + 8, 32, 10, CARAFE_COLOR_FOCUS, 41);
            }
            carafeDrawRing(picture, x + 10, y + 16, 8, on ? 5 : 2, on ? CARAFE_COLOR_FOCUS : CARAFE_COLOR_DIM, 255);
            CarafeTextStyle option = { 15, on ? CarafeBold : CarafeRegular, 32, on ? CARAFE_COLOR_INK : CARAFE_COLOR_TEXT,
                false };
            carafeDrawLine(picture, pick(section->presets[i].name), &option, x + 28, y, 0);
            y += 34;
            y = carafeDrawWrapped(picture, pick(section->presets[i].hint),
                    &(CarafeTextStyle){ 13, CarafeRegular, 18, CARAFE_COLOR_LABEL, false }, x + 28, y, width - 28) + 6;
        }
    } else if (item->kind == ItemReset) {
        carafeDrawWrapped(picture, g_reset_armed ? s->reset_again : s->reset_all_hint, &body, x, y, width);
    } else if (item->kind == ItemGroup) {
        const CarafeGroup* group = &g_catalog->groups[item->index];
        for (int p = 0; p < group->page_count; p++) {
            char line[TEXT_SIZE];
            int visible = 0;
            for (int r = 0; r < group->pages[p].row_count; r++) {
                visible += carafeSettingVisible(g_catalog, &g_catalog->settings[group->pages[p].rows[r].index]);
            }
            snprintf(line, sizeof(line), "%s · %d", pick(group->pages[p].name), visible);
            y = carafeDrawWrapped(picture, line, &body, x, y, width);
        }
    }
}

static void drawList(CarafeImage* picture) {
    Item items[MAX_ITEMS];
    int count = buildItems(items);
    int top = LIST_TOP;
    if (g_group >= 0 && g_catalog->groups[g_group].page_count > 1) {
        const CarafeGroup* group = &g_catalog->groups[g_group];
        int x = LIST_X;
        for (int p = 0; p < group->page_count; p++) {
            const char* name = pick(group->pages[p].name);
            int w = carafeTextWidth(name, 14, CarafeBold) + 24;
            if (x + w > 1208) {
                break;
            }
            bool on = p == g_page;
            carafeDrawRound(picture, x, 104, w, 32, 16, on ? CARAFE_COLOR_FOCUS : CARAFE_COLOR_WHITE, on ? 255 : 18);
            CarafeTextStyle style = { 14, CarafeBold, 32, on ? 0xFF22121Au : CARAFE_COLOR_HINT, true };
            carafeDrawLine(picture, name, &style, x, 104, w);
            addHit(HitPage, p, x, 104, w, 32);
            x += w + 6;
        }
        top = LIST_TOP_PAGES;
    }
    int* focus = focusRef();
    int* scroll = scrollRef();
    if (*focus >= count) {
        *focus = count > 0 ? count - 1 : 0;
    }
    int visible = (LIST_BOTTOM - top) / (ROW_H + ROW_GAP);
    if (*focus < *scroll) {
        *scroll = *focus;
    }
    if (*focus >= *scroll + visible) {
        *scroll = *focus - visible + 1;
    }
    int y = top;
    for (int i = *scroll; i < count && i < *scroll + visible; i++) {
        drawRow(picture, i, &items[i], y + 3, i == *focus);
        y += ROW_H + ROW_GAP;
    }
    if (*scroll + visible < count) {
        carafeDrawText(picture, "…", 18, CarafeBold, LIST_X + LIST_W / 2, LIST_BOTTOM - 8, CARAFE_COLOR_LABEL);
    }
    drawCard(picture, count > 0 ? &items[*focus] : NULL, top);
}

static void drawDevicePart(CarafeImage* picture, int source, int x, int y, int w, int h, int radius) {
    bool on = false;
    int focused = g_pad_focus - STRIP_MAX;
    if (focused >= 0) {
        int left[16];
        int right[16];
        int left_count = columnSources(0, left);
        columnSources(1, right);
        int focus_source = focused < left_count ? left[focused] : right[focused - left_count];
        on = focus_source == source || ((source == S_STICKL)
                                           && (focus_source == S_LUP || focus_source == S_LDOWN
                                               || focus_source == S_LLEFT || focus_source == S_LRIGHT))
            || ((source == S_STICKR)
                && (focus_source == S_RUP || focus_source == S_RDOWN || focus_source == S_RLEFT
                    || focus_source == S_RRIGHT));
    }
    carafeDrawRound(picture, x, y, w, h, radius, on ? CARAFE_COLOR_FOCUS : 0xFF1C0B13u, 255);
    carafeDrawRoundStroke(picture, x, y, w, h, radius, 2, on ? CARAFE_COLOR_FOCUS : CARAFE_COLOR_TEXT, 255);
}

static void drawDevice(CarafeImage* picture) {
    const float k = 600.0f / 251.0f;
    const int ox = 340 + (int)(6 * k);
    const int oy = 210 + (int)(16 * k);
#define DX(v) (ox + (int)((v) * k))
#define DY(v) (oy + (int)((v) * k))
#define DS(v) ((int)((v) * k))
    carafeDrawRoundStroke(picture, DX(0), DY(0), DS(239), DS(102), DS(16), 2, CARAFE_COLOR_TEXT, 255);
    carafeDrawRound(picture, DX(35.9f), DY(0), 2, DS(102), 0, CARAFE_COLOR_TEXT, 255);
    carafeDrawRound(picture, DX(203.1f), DY(0), 2, DS(102), 0, CARAFE_COLOR_TEXT, 255);
    carafeDrawRoundStroke(picture, DX(43.5f), DY(8), DS(152), DS(86), 4, 1, CARAFE_COLOR_TEXT, 200);
    CarafeTextStyle mode = { 13, CarafeRegular, 20, CARAFE_COLOR_LABEL, true };
    carafeDrawLine(picture, keyboardMode() ? text()->keyboard : text()->gamepad, &mode, DX(43.5f), DY(47), DS(152));
    drawDevicePart(picture, S_ZL, DX(9), DY(-11.5f), DS(24.5f), DS(7.5f), DS(3.75f));
    drawDevicePart(picture, S_ZR, DX(205.5f), DY(-11.5f), DS(24.5f), DS(7.5f), DS(3.75f));
    drawDevicePart(picture, S_L, DX(6), DY(-2.5f), DS(28), DS(3), DS(1.5f));
    drawDevicePart(picture, S_R, DX(205), DY(-2.5f), DS(28), DS(3), DS(1.5f));
    drawDevicePart(picture, S_MINUS, DX(25.8f), DY(7.6f), DS(6.4f), DS(1.9f) + 2, 2);
    drawDevicePart(picture, S_PLUS, DX(207.5f), DY(6), DS(5), DS(5), 3);
    drawDevicePart(picture, S_STICKL, DX(17.5f - 8), DY(28 - 8), DS(16), DS(16), DS(8));
    drawDevicePart(picture, S_STICKR, DX(221.5f - 8), DY(60 - 8), DS(16), DS(16), DS(8));
    const int dpad[4][3] = { { S_UP, 0, -1 }, { S_DOWN, 0, 1 }, { S_LEFT, -1, 0 }, { S_RIGHT, 1, 0 } };
    for (int i = 0; i < 4; i++) {
        drawDevicePart(picture, dpad[i][0], DX(17.5f + dpad[i][1] * 9.2f - 4.2f), DY(60 + dpad[i][2] * 9.2f - 4.2f),
            DS(8.4f), DS(8.4f), DS(4.2f));
    }
    const int abxy[4][3] = { { S_X, 0, -1 }, { S_B, 0, 1 }, { S_Y, -1, 0 }, { S_A, 1, 0 } };
    const char* letters[4] = { "X", "B", "Y", "A" };
    for (int i = 0; i < 4; i++) {
        int bx = DX(221.5f + abxy[i][1] * 9.2f - 4.2f);
        int by = DY(28 + abxy[i][2] * 9.2f - 4.2f);
        drawDevicePart(picture, abxy[i][0], bx, by, DS(8.4f), DS(8.4f), DS(4.2f));
        CarafeTextStyle letter = { 12, CarafeHeavy, DS(8.4f), CARAFE_COLOR_INK, true };
        carafeDrawLine(picture, letters[i], &letter, bx, by, DS(8.4f));
    }
#undef DX
#undef DY
#undef DS
}

static void drawPad(CarafeImage* picture) {
    int strip[4];
    int strips = stripItems(strip);
    int width = strips > 0 ? (1136 - (strips - 1) * 10) / strips : 0;
    for (int i = 0; i < strips; i++) {
        const CarafeSetting* setting = &g_catalog->settings[strip[i]];
        int x = LIST_X + i * (width + 10);
        bool focus = g_pad_focus == i;
        if (focus) {
            carafeDrawFocusRing(picture, x, 104, width, 52, 14);
        } else {
            carafeDrawRound(picture, x, 104, width, 52, 14, CARAFE_COLOR_WHITE, 13);
            carafeDrawRoundStroke(picture, x, 104, width, 52, 14, 2, CARAFE_COLOR_WHITE, 26);
        }
        uint32_t ink = focus ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_VALUE;
        CarafeTextStyle name = { 15, CarafeRegular, 52, ink, false };
        carafeDrawLine(picture, pick(setting->name), &name, x + 14, 104, 0);
        const char* value = carafeValueLabel(setting, g_language);
        int vw = carafeTextWidth(value, 15, CarafeBold);
        CarafeTextStyle val = { 15, CarafeBold, 52, focus ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_INK, false };
        carafeDrawLine(picture, value, &val, x + width - 14 - vw, 104, 0);
        addHit(HitStrip, i, x, 104, width, 52);
    }
    bool pad = !keyboardMode();
    int left[16];
    int left_count = columnSources(0, left);
    for (int column = 0; column < 2; column++) {
        int x = column == 0 ? 72 : 958;
        int y = 176;
        int ordinal = column == 0 ? 0 : left_count;
        for (int r = 0; r < 5; r++) {
            const MapRow* row = &MAP[column][r];
            int h = row->cross ? 118 : 42;
            bool row_on = false;
            for (int c = 0; c < row->count; c++) {
                row_on |= g_pad_focus - STRIP_MAX == ordinal + c;
            }
            carafeDrawRound(picture, x, y, 250, h, 14, row_on ? CARAFE_COLOR_FOCUS : CARAFE_COLOR_PANEL,
                row_on ? 36 : 115);
            carafeDrawRoundStroke(picture, x, y, 250, h, 14, 1, row_on ? CARAFE_COLOR_FOCUS : CARAFE_COLOR_WHITE,
                row_on ? 255 : 20);
            carafeDrawText(picture, text()->rows[row->label][0], 13, CarafeBold, x + 10, y + (row->cross ? 4 : 5),
                CARAFE_COLOR_INK);
            carafeDrawText(picture, text()->rows[row->label][1], 11, CarafeRegular, x + 10, y + (row->cross ? 20 : 21),
                CARAFE_COLOR_LABEL);
            for (int c = 0; c < row->count; c++) {
                const Cell* cell = &row->cells[c];
                int cw = row->cross ? 76 : 120;
                int cx = row->cross ? x + 6 + cell->column * 80 : x + 124;
                int cy = row->cross ? y + 26 + cell->row * 30 : y + 7;
                bool focus = g_pad_focus - STRIP_MAX == ordinal + c;
                char label[TEXT_SIZE];
                carafeChordLabel(g_catalog, pad, chordOf(cell->source, pad), g_language, label, sizeof(label));
                if (focus) {
                    carafeDrawRound(picture, cx - 4, cy - 4, cw + 8, 36, 12, CARAFE_COLOR_FOCUS, 255);
                    carafeDrawRound(picture, cx - 2, cy - 2, cw + 4, 32, 11, CARAFE_COLOR_GAP, 255);
                    carafeDrawRound(picture, cx, cy, cw, 28, 9, CARAFE_COLOR_INK, 255);
                } else {
                    carafeDrawRound(picture, cx, cy, cw, 28, 9, 0xFF1C0B13u, 255);
                    carafeDrawRoundStroke(picture, cx, cy, cw, 28, 9, 1, CARAFE_COLOR_DIM, 255);
                }
                bool marked = cell->mark[0] != '\0' && strstr(label, cell->mark) == NULL;
                int mark_w = marked ? carafeTextWidth(cell->mark, 10, CarafeBold) + 8 : 0;
                if (marked) {
                    carafeDrawText(picture, cell->mark, 10, CarafeBold, cx + 6, cy + 7,
                        focus ? CARAFE_COLOR_DIM : CARAFE_COLOR_LABEL);
                }
                int size = 12;
                while (size > 9 && carafeTextWidth(label, size, CarafeBold) > cw - 8 - mark_w) {
                    size--;
                }
                CarafeTextStyle chip = { size, CarafeBold, 28, focus ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_VALUE,
                    true };
                carafeDrawLine(picture, label, &chip, cx + mark_w, cy, cw - mark_w);
                addHit(HitSource, STRIP_MAX + ordinal + c, cx, cy, cw, 28);
            }
            ordinal += row->count;
            y += h + 8;
        }
    }
    drawDevice(picture);
}

static void drawPicker(CarafeImage* picture) {
    const Strings* s = text();
    const int columns = g_picker_pad ? 4 : 8;
    const int cell_w = g_picker_pad ? 152 : 74;
    const int cell_h = 44;
    int count = pickerCount();
    int rows_visible = (LIST_BOTTOM - LIST_TOP) / (cell_h + 6);
    int focus_row = g_picker_focus / columns;
    int first_row = focus_row >= rows_visible ? focus_row - rows_visible + 1 : 0;
    for (int i = first_row * columns; i < count && i < (first_row + rows_visible) * columns; i++) {
        int column = i % columns;
        int row = i / columns - first_row;
        int x = LIST_X + column * (cell_w + 6);
        int y = LIST_TOP + row * (cell_h + 6);
        unsigned int code = pickerCode(i);
        bool chosen = false;
        for (int c = 0; c < g_picker_chord.count; c++) {
            chosen |= g_picker_chord.code[c] == code;
        }
        bool focus = i == g_picker_focus;
        if (focus) {
            carafeDrawFocusRing(picture, x, y, cell_w, cell_h, 10);
        } else {
            carafeDrawRound(picture, x, y, cell_w, cell_h, 10, chosen ? CARAFE_COLOR_ACCENT : CARAFE_COLOR_WHITE,
                chosen ? 255 : 20);
        }
        const CarafeKey* key = g_picker_pad ? &g_catalog->pads[i] : &g_catalog->keys[i];
        const char* label = pick(key->label);
        int size = 15;
        while (size > 9 && carafeTextWidth(label, size, CarafeBold) > cell_w - 6) {
            size--;
        }
        CarafeTextStyle style = { size, CarafeBold, cell_h, focus ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_VALUE, true };
        carafeDrawLine(picture, label, &style, x, y, cell_w);
        addHit(HitKey, i, x, y, cell_w, cell_h);
    }
    int top = LIST_TOP;
    carafeDrawRound(picture, CARD_X, top, CARD_W, LIST_BOTTOM - top, 24, CARAFE_COLOR_PANEL, 128);
    int x = CARD_X + 30;
    int width = CARD_W - 60;
    char title[TEXT_SIZE];
    snprintf(title, sizeof(title), s->button, carafe_source_names[g_picker_source]);
    CarafeTextStyle head = { 24, CarafeHeavy, 29, CARAFE_COLOR_INK, false };
    int y = carafeDrawWrapped(picture, title, &head, x, top + 26, width) + 8;
    CarafeTextStyle body = { 16, CarafeRegular, 24, CARAFE_COLOR_TEXT, false };
    y = carafeDrawWrapped(picture, g_picker_pad ? s->pick_pad : s->pick_keys, &body, x, y, width) + 12;
    char chord[TEXT_SIZE];
    carafeChordLabel(g_catalog, g_picker_pad, &g_picker_chord, g_language, chord, sizeof(chord));
    int cw = carafeTextWidth(chord, 16, CarafeBold) + 28;
    carafeDrawRound(picture, x, y, cw > width ? width : cw, 34, 17, CARAFE_COLOR_FOCUS, 41);
    CarafeTextStyle chip = { 16, CarafeBold, 34, CARAFE_COLOR_CHIP, true };
    carafeDrawLine(picture, chord, &chip, x, y, cw > width ? width : cw);
    int mode_h = 40;
    carafeDrawRound(picture, x - 4, LIST_BOTTOM - 26 - mode_h, width + 8, mode_h, 12, CARAFE_COLOR_LIVE, 30);
    CarafeTextStyle mode = { 14, CarafeRegular, mode_h, CARAFE_COLOR_LIVE, false };
    carafeDrawLine(picture, s->live_controls, &mode, x + 10, LIST_BOTTOM - 26 - mode_h, 0);
}

static void drawConfirm(CarafeImage* picture) {
    const Strings* s = text();
    carafeDrawRound(picture, 0, 0, picture->width, picture->height, 0, 0xFF0E0408u, 184);
    int pending[MAX_ITEMS];
    int count = 0;
    for (int i = 0; i < g_catalog->setting_count && count < 6; i++) {
        if (carafeSettingPending(&g_catalog->settings[i])) {
            pending[count++] = i;
        }
    }
    const int w = 700;
    const int x = (picture->width - w) / 2;
    const int h = 250 + count * 44 + 90;
    const int y = (picture->height - h) / 2;
    carafeDrawRound(picture, x, y, w, h, 24, 0xFF1C0B13u, 255);
    carafeDrawRoundStroke(picture, x, y, w, h, 24, 1, CARAFE_COLOR_WHITE, 26);
    CarafeTextStyle title = { 28, CarafeHeavy, 34, CARAFE_COLOR_INK, false };
    int ty = carafeDrawWrapped(picture, s->confirm_title, &title, x + 36, y + 30, w - 72) + 6;
    CarafeTextStyle body = { 17, CarafeRegular, 25, CARAFE_COLOR_TEXT, false };
    ty = carafeDrawWrapped(picture, s->confirm_body, &body, x + 36, ty, w - 72) + 10;
    for (int i = 0; i < count; i++) {
        const CarafeSetting* setting = &g_catalog->settings[pending[i]];
        const char* from = setting->start >= 0 ? pick(setting->values[setting->start].label) : setting->start_text;
        char line[TEXT_SIZE];
        snprintf(line, sizeof(line), "%s → %s", from, carafeValueLabel(setting, g_language));
        carafeDrawText(picture, pick(setting->name), 17, CarafeRegular, x + 36, ty + 10, CARAFE_COLOR_VALUE);
        int lw = carafeTextWidth(line, 17, CarafeBold);
        carafeDrawText(picture, line, 17, CarafeBold, x + w - 36 - lw, ty + 10, CARAFE_COLOR_INK);
        carafeDrawRound(picture, x + 36, ty + 43, w - 72, 1, 0, CARAFE_COLOR_WHITE, 26);
        ty += 44;
    }
    ty += 14;
    int box = ty;
    carafeDrawRound(picture, x + 36, box, w - 72, 64, 16, CARAFE_COLOR_WHITE, 13);
    if (g_trial) {
        carafeDrawRound(picture, x + 52, box + 18, 28, 28, 8, CARAFE_COLOR_FOCUS, 255);
        carafeDrawText(picture, "✓", 18, CarafeHeavy, x + 58, box + 18, 0xFF22121Au);
    } else {
        carafeDrawRoundStroke(picture, x + 52, box + 18, 28, 28, 8, 2, CARAFE_COLOR_DIM, 255);
    }
    carafeDrawText(picture, s->trial, 18, CarafeBold, x + 96, box + 8, CARAFE_COLOR_INK);
    carafeDrawText(picture, s->trial_hint, 14, CarafeRegular, x + 96, box + 34, CARAFE_COLOR_HINT);
    drawGlyph(picture, x + w - 36 - 24, box + 20, "X", CARAFE_COLOR_PRIMARY_INK, CARAFE_COLOR_INK);
    addHit(HitButton, ButtonTrial, x + 36, box, w - 72, 64);
    int by = box + 84;
    const char* labels[3] = { s->restart, s->later, s->undo };
    const ButtonId ids[3] = { ButtonConfirm, ButtonLater, ButtonUndo };
    int bx = x + 36;
    for (int i = 0; i < 3; i++) {
        bool focus = i == g_confirm_focus;
        int bw = (focus ? 14 + 46 : 30) + carafeTextWidth(labels[i], 20, CarafeBold) + 30;
        if (focus) {
            carafeDrawFocusRing(picture, bx, by, bw, 56, 16);
            carafeDrawCircle(picture, bx + 14 + 17, by + 28, 17, CARAFE_COLOR_PRIMARY_INK, 255);
            CarafeTextStyle g = { 19, CarafeHeavy, 34, CARAFE_COLOR_INK, true };
            carafeDrawLine(picture, "A", &g, bx + 14, by + 11, 34);
        } else {
            carafeDrawRound(picture, bx, by, bw, 56, 16, CARAFE_COLOR_WHITE, 18);
            carafeDrawRoundStroke(picture, bx, by, bw, 56, 16, 2, CARAFE_COLOR_WHITE, 36);
        }
        CarafeTextStyle label = { 20, CarafeBold, 56, focus ? CARAFE_COLOR_PRIMARY_INK : CARAFE_COLOR_INK, false };
        carafeDrawLine(picture, labels[i], &label, bx + (focus ? 14 + 46 : 30), by, 0);
        addHit(HitButton, ids[i], bx, by, bw, 56);
        bx += bw + 14;
    }
}

static void drawFooter(CarafeImage* picture) {
    const Strings* s = text();
    int x = LIST_X;
    int pending = carafePendingCount(g_catalog);
    if (g_picker) {
        x = drawButtonAt(picture, x, "+", s->done, 1, ButtonDone);
        drawButtonAt(picture, x, NULL, s->clear, 0, ButtonClear);
    } else if (g_error_mode) {
        char label[TEXT_SIZE];
        if (pending > 0) {
            snprintf(label, sizeof(label), s->restart_count, pending);
        } else {
            snprintf(label, sizeof(label), "%s", s->restart);
        }
        x = drawButtonAt(picture, x, "+", label, pending > 0 ? 2 : 1, ButtonRestart);
        drawButtonAt(picture, x, NULL, s->to_error, 0, ButtonBack);
    } else {
        if (pending > 0) {
            char label[TEXT_SIZE];
            snprintf(label, sizeof(label), s->restart_count, pending);
            x = drawButtonAt(picture, x, "+", label, 2, ButtonRestart);
        }
        drawButtonAt(picture, x, NULL, s->to_game, 0, ButtonGame);
    }
    int hx = CARD_X;
    int hy = BUTTONS_Y + 18;
    if (g_picker) {
        hx = drawHint(picture, hx, hy, "A", g_picker_pad ? s->choose : s->toggle);
        hx = drawHint(picture, hx, hy, "Y", s->clear);
        drawHint(picture, hx, hy, "B", s->cancel);
        return;
    }
    hx = drawHint(picture, hx, hy, "A", s->change);
    hx = drawHint(picture, hx, hy, "Y", s->reset);
    hx = drawHint(picture, hx, hy, "L R", g_group >= 0 ? s->pages : s->sections);
    drawHint(picture, hx, hy, "B", g_group >= 0 ? s->back : g_error_mode ? s->to_error : s->to_game);
}

static void renderLocked(CarafeImage* picture) {
    if (g_catalog == NULL) {
        return;
    }
    g_hit_count = 0;
    carafeDrawBackground(picture, g_error_mode ? 255 : MENU_OPACITY);
    drawHeader(picture);
    if (g_picker) {
        drawPicker(picture);
    } else if (g_group < 0 && tabIsControls(g_tab)) {
        drawPad(picture);
    } else {
        drawList(picture);
    }
    drawFooter(picture);
    if (g_confirm) {
        g_hit_count = 0;
        drawConfirm(picture);
    }
}

void carafeMenuRender(CarafeImage* picture) {
    pthread_mutex_lock(&g_lock);
    renderLocked(picture);
    pthread_mutex_unlock(&g_lock);
}

bool carafeMenuVisible(void) {
    return __atomic_load_n(&g_open, __ATOMIC_ACQUIRE) && !g_error_mode;
}

unsigned int carafeMenuGeneration(void) {
    return __atomic_load_n(&g_generation, __ATOMIC_RELAXED);
}

static bool ensurePicture(int width, int height) {
    if (g_picture.pixels != NULL && g_picture.width == width && g_picture.height == height) {
        return true;
    }
    free(g_picture.pixels);
    g_picture.pixels = malloc((size_t)width * height * 4);
    g_picture.width = width;
    g_picture.height = height;
    g_rendered = 0;
    return g_picture.pixels != NULL;
}

static bool prepareLocked(int width, int height, unsigned int* generation) {
    if (!g_open || width != CARAFE_SCREEN_WIDTH || height != CARAFE_SCREEN_HEIGHT) {
        return false;
    }
    pthread_mutex_lock(&g_picture_lock);
    bool ok = ensurePicture(width, height);
    if (ok) {
        unsigned int current = carafeMenuGeneration();
        if (g_rendered != current) {
            renderLocked(&g_picture);
            g_rendered = current;
        }
        *generation = g_rendered | 0x80000000u;
    }
    pthread_mutex_unlock(&g_picture_lock);
    return ok;
}

unsigned int carafeMenuCopy(int screen_width, int screen_height, void* pixels, int stride, int rgba) {
    unsigned int generation = 0;
    pthread_mutex_lock(&g_lock);
    bool ok = prepareLocked(screen_width, screen_height, &generation);
    pthread_mutex_unlock(&g_lock);
    if (!ok) {
        return 0;
    }
    pthread_mutex_lock(&g_picture_lock);
    if (g_picture.pixels == NULL) {
        pthread_mutex_unlock(&g_picture_lock);
        return 0;
    }
    for (int y = 0; y < g_picture.height; y++) {
        uint32_t* out = (uint32_t*)((uint8_t*)pixels + (size_t)y * stride);
        const uint32_t* in = g_picture.pixels + (size_t)y * g_picture.width;
        for (int x = 0; x < g_picture.width; x++) {
            uint32_t p = in[x];
            out[x] = rgba ? (p & 0xFF00FF00u) | (p >> 16 & 0xFF) | (p & 0xFF) << 16 : p;
        }
    }
    pthread_mutex_unlock(&g_picture_lock);
    return generation;
}

int wine_nx_osk_translucent(void) {
    return carafeMenuVisible();
}

void wine_nx_menu_poll(u64 held) {
    bool chord = (held & CHORD_BUTTONS) == CHORD_BUTTONS;
    bool was = g_chord_held != 0;
    g_chord_held = chord;
    if (!chord || was) {
        return;
    }
    pthread_mutex_lock(&g_lock);
    if (g_open) {
        closeLocked();
    } else if (!openLocked(false)) {
        note("[CARAFE] settings menu: nothing to show");
    }
    pthread_mutex_unlock(&g_lock);
}

int __real_wine_nx_osk_input(unsigned int held, int stick_x, int stick_y, int touching, int touch_x, int touch_y,
    uint64_t now_ns);

int __wrap_wine_nx_osk_input(unsigned int held, int stick_x, int stick_y, int touching, int touch_x, int touch_y,
    uint64_t now_ns) {
    if (!carafeMenuVisible()) {
        return __real_wine_nx_osk_input(held, stick_x, stick_y, touching, touch_x, touch_y, now_ns);
    }
    unsigned int stick = 0;
    if (stick_y > 16000) {
        stick |= OSK_UP;
    } else if (stick_y < -16000) {
        stick |= OSK_DOWN;
    }
    if (stick_x > 16000) {
        stick |= OSK_RIGHT;
    } else if (stick_x < -16000) {
        stick |= OSK_LEFT;
    }
    CarafeMenuAction action = carafeMenuInput(held | stick, touching, touch_x, touch_y, now_ns);
    if (action == CarafeMenuBack) {
        pthread_mutex_lock(&g_lock);
        closeLocked();
        pthread_mutex_unlock(&g_lock);
    } else if (action == CarafeMenuRestart) {
        pthread_mutex_lock(&g_lock);
        bool trial = g_trial;
        closeLocked();
        pthread_mutex_unlock(&g_lock);
        note("[CARAFE] settings menu: restarting the game with the new settings%s", trial ? " (trial run)" : "");
        carafeRequestRestart(trial);
    }
    return 1;
}

bool carafeMenuOpenError(CarafeFailure failure) {
    pthread_mutex_lock(&g_lock);
    g_failure = failure;
    bool ok = openLocked(true);
    pthread_mutex_unlock(&g_lock);
    return ok;
}

void carafeMenuClose(void) {
    pthread_mutex_lock(&g_lock);
    closeLocked();
    pthread_mutex_unlock(&g_lock);
}
