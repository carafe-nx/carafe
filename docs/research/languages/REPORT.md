# Every Switch system language

Researched 2026-10-24 for the ROADMAP item "The app and the console screens in every Switch system language".

## The console's languages

`SetLanguage` in libnx ([set.h](https://github.com/switchbrew/libnx/blob/master/nx/include/switch/services/set.h),
[switchbrew: Settings services](https://switchbrew.org/wiki/Settings_services)) has 18 values:

| Value | Name | Code | Note |
|---|---|---|---|
| 0 | `SetLanguage_JA` | ja | |
| 1 | `SetLanguage_ENUS` | en-US | |
| 2 | `SetLanguage_FR` | fr | |
| 3 | `SetLanguage_DE` | de | |
| 4 | `SetLanguage_IT` | it | |
| 5 | `SetLanguage_ES` | es | |
| 6 | `SetLanguage_ZHCN` | zh-CN | old code for Simplified Chinese |
| 7 | `SetLanguage_KO` | ko | |
| 8 | `SetLanguage_NL` | nl | |
| 9 | `SetLanguage_PT` | pt | |
| 10 | `SetLanguage_RU` | ru | |
| 11 | `SetLanguage_ZHTW` | zh-TW | old code for Traditional Chinese |
| 12 | `SetLanguage_ENGB` | en-GB | |
| 13 | `SetLanguage_FRCA` | fr-CA | |
| 14 | `SetLanguage_ES419` | es-419 | |
| 15 | `SetLanguage_ZHHANS` | zh-Hans | 4.0.0+ |
| 16 | `SetLanguage_ZHHANT` | zh-Hant | 4.0.0+ |
| 17 | `SetLanguage_PTBR` | pt-BR | 10.1.0+ |

These are 15 interface languages once en-US and en-GB share one text and the old Chinese codes are folded into
the new ones, as nx-hbmenu ([language.c](https://github.com/switchbrew/nx-hbmenu/blob/master/common/language.c)) and
libtesla do. Polish exists only on Switch 2 (21.0.0+) and has no libnx value.

On the original Switch the language list has 12 entries; the region picks the regional variant. Nintendo's
[11.0.0 notes](https://en-americas-support.nintendo.com/app/answers/detail/a_id/43314/): with the region set to
the Americas and the language to Português, the HOME Menu and some software show Brazilian Portuguese. Software
sees the variant through `setGetSystemLanguage`. Switch 2 lists the variants as separate entries
([switch2.jp](https://switch2.jp/change-language/)).

## Fonts on the console

- The bundled M PLUS 1p (`runtime/overlay/fonts/MPLUS1p-Regular.ttf`, its cmap read 2026-10-24) covers Latin-1,
  Latin Extended-A, 240 of 256 Cyrillic code points and 5082 CJK ideographs, the Japanese set. It has no Hangul at
  all. Of the Chinese already in `carafe_menu.c` 79 characters are missing (启 动 戏 开 页 设 选 项 键 钮 错 误…),
  about 300 of `zh-Hans.json`; Traditional misses a few (啟 圖) and draws the rest in Japanese shapes.
- The console's shared fonts ([pl.h](https://github.com/switchbrew/libnx/blob/master/nx/include/switch/services/pl.h)):
  `Standard` (Latin, kana, Japanese kanji), `ChineseSimplified`, `ExtChineseSimplified`, `ChineseTraditional`,
  `KO`, `NintendoExt` (button glyphs). Each is a TTF in shared memory, `plGetSharedFontByType` after
  `plInitialize(PlServiceType_User)`.
- SDL2_ttf has no glyph fallback (only SDL3_ttf has `TTF_AddFallbackFont`). The usual way is runs: pick a font per
  code point with `TTF_GlyphIsProvided32`, measure and render each run, align baselines by ascent.
- How homebrew picks: nx-hbmenu walks the fonts `plGetSharedFont` returns for the language, per glyph. libtesla puts
  the language's CJK font after `Standard`, so Chinese gets Japanese shapes where `Standard` has the character;
  Ultrahand puts the language's font first. With a Chinese font first, Latin letters come out full width
  ([PlayGuard #36](https://github.com/JigSawFr/PlayGuard/pull/36)), so the language's font should win only for
  CJK code points.

## The overlay today

- `CARAFE_LANGUAGES` 3 (ru 0, en 1, zh 2) in `carafe_settings.h`; English index 1 is the fallback in `pick()`,
  `carafeValueLabel()` and `carafeChordLabel()`.
- `carafeLanguage()` in `carafe_settings.c` knows RU and ZHCN/ZHHANS. The loading screen has its own
  `chooseLanguage()` with RU only; the error screen tests `== 0`. Both screens have English and Russian only.
- `menu.txt` texts are read by position: `setting` and `preset` lines hold N names then N hints, so the writer and
  the reader must agree on N. The file is capped at 512 KiB; at 15 languages it would be about 350–400 KB.
- Few labels are truncated: buttons, tabs and footer hints overflow when too long. The error screen's three buttons
  share about 400 px at 22 px bold.

## The app's picker

Flags for languages are discouraged: [W3C](https://www.w3.org/International/questions/qa-site-conneg),
[NN/g](https://www.nngroup.com/articles/flag-problems/), [GOV.UK](https://design-system.service.gov.uk/components/language-navigation/).
Steam, Heroic and Playnite use a dropdown of native names; Discord shows flags with names. Latin American Spanish has
no flag of its own and Traditional Chinese needs Taiwan's or Hong Kong's.

## The installer

Tauri 2 takes NSIS language file names in `bundle.windows.nsis.languages`
([config](https://v2.tauri.app/reference/config/#nsisconfig)) and ships its own strings for English, Russian,
SimpChinese, TradChinese, Japanese, Korean, French, German, Italian, Spanish, SpanishInternational, Dutch,
Portuguese and PortugueseBR ([languages](https://github.com/tauri-apps/tauri/tree/dev/crates/tauri-bundler/src/bundle/windows/nsis/languages)).
NSIS has no Canadian French or Latin American Spanish; it matches the Windows language exactly, then by primary
language, then takes the first entry ([forum](https://nsis-dev.github.io/NSIS-Forums/html/t-244530.html)), so those
users get French and Spanish.
