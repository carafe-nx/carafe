# 26. Simplified Chinese interface

Date: 2026-10-15

Replaced by [0035](0035-every-switch-language.md).

Replaces the languages of [0013](0013-desktop-app.md) and the installer languages of [0017](0017-updates.md).

## Context

The interface and installer support English and Russian. Simplified Chinese needs to be selectable,
persisted in preferences and detected on first launch without treating Traditional Chinese as Simplified.

## Decision

Use `zh-Hans` in preferences, generated bindings and i18next resources. Detect `zh`, `zh-CN`, `zh-SG`
and explicit `zh-Hans` locales as Simplified Chinese. Keep English as the fallback for unsupported locales,
including Traditional Chinese. Add the native language name and Chinese flag to the existing language
switcher and `SimpChinese` to the NSIS installer languages.

Check translation keys, interpolation, markup, locale detection and i18next rendering with Node's built-in
test runner as part of the UI check. Verify the preference value with a Rust serialization round trip.

## Rejected options

- Mapping every `zh` locale to Simplified Chinese would also affect users of Traditional Chinese.
- Keeping the language only in the UI would prevent the backend from restoring saved preferences.
