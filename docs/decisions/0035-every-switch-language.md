# 35. Every Switch system language

Date: 2026-10-24

Replaces the language rules of [0026](0026-simplified-chinese-interface.md). Research:
[research/languages/REPORT.md](../research/languages/REPORT.md).

## Context

The app, the installer and the console screens spoke Russian, English and Simplified Chinese; the loading and
error screens only Russian and English. The console offers 15 interface languages, regional variants included.
The overlay's font had no Hangul and lacked many Simplified Chinese characters, so the Chinese menu drew gaps.

## Decision

- Fifteen languages everywhere: English, Русский, 简体中文, 日本語, 한국어, 繁體中文, Français, Français (Canada),
  Deutsch, Italiano, Español, Español (Latinoamérica), Nederlands, Português, Português (Brasil). The codes are
  `en`, `ru`, `zh-Hans`, `ja`, `ko`, `zh-Hant`, `fr`, `fr-CA`, `de`, `it`, `es`, `es-419`, `nl`, `pt`, `pt-BR` in
  preferences, generated bindings, i18next resources and the console menu file. American and British English share
  one text. Carafe writes the translations itself; each is checked against English for keys, placeholders, markup
  and the language's plural categories.
- The app follows Windows on first launch: `ru*`; `zh-Hant`, `zh-TW`, `zh-HK`, `zh-MO` as Traditional Chinese, any
  other `zh` as Simplified; `ja`, `ko`, `de`, `it`, `nl`; `fr-CA` and other `fr`; `es` and `es-ES` as Spain, any
  other `es-*` as Latin America; `pt-PT` and other regional `pt-*` as Portugal, `pt` and `pt-BR` as Brazil (CLDR's
  `pt` is Brazilian); everything else English.
- The picker stays flags, as the user chose: the current language's flag in the header, a popup grid of the others,
  the native name as the tooltip. "Same as system" is its first entry, a globe, and stays the default. Flags:
  Britain for English, Mexico for Latin American Spanish, Canada for Canadian French, Brazil for Brazilian Portuguese,
  Taiwan for Traditional Chinese (the console's old code for it is `zh-TW`).
- The installer picks the Windows language itself, with English, Russian, SimpChinese, TradChinese, Japanese,
  Korean, French, German, Italian, Spanish, Dutch, Portuguese and PortugueseBR. NSIS has no Canadian French or Latin
  American Spanish; those Windows languages get French and Spanish.
- The console follows `setGetSystemLanguage`: en-US and en-GB as English, the old zh-CN and zh-TW as Simplified and
  Traditional. One lookup serves the loading screen, the error screen and the menu. Each screen keeps its strings
  in C tables indexed by language; the menu's plural line has a rule per language.
- `carafe/menu.txt` becomes version 2: a `languages` line names the 15 columns in order, and every text record holds
  15 texts. The overlay rejects a file whose list differs from its own, keeps only the console's language and English
  of each record and frees nothing else; the read limit grows to 1 MiB.
- Text on the console is drawn in runs: each code point goes to M PLUS 1p, or for Chinese and Korean to the console's
  shared font of that language (`ChineseSimplified` then `ExtChineseSimplified`, `ChineseTraditional`, `KO`). In
  those languages CJK code points prefer the language's font; any code point M PLUS 1p lacks falls back to the shared
  fonts. The loading screen keeps the console's `Standard` font as its first font with the same fallback.

## Rejected

- A dropdown of native names, or names with flags: offered in the mockup, the user kept flags.
- Bundling a CJK font (Noto, Source Han): 10–20 MB per NSP for what the console already holds.
- A Chinese font first for every glyph: Latin letters come out full width.
- Keeping all 15 texts of every record in memory: about 400 KB more for texts the console never shows.
- A separate menu file per language: the package cannot know the console's language when it is built.
