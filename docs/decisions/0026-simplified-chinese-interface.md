# Simplified Chinese interface

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
