//! Texts of the console settings menu, taken from the app's i18n files so that the window and the console say the
//! same.

use std::sync::OnceLock;

use carafe_core::settings_menu::MenuLanguage;
use serde_json::Value;

const LOCALES: [&str; 15] = [
    include_str!("../../../ui/src/shared/i18n/locales/ru.json"),
    include_str!("../../../ui/src/shared/i18n/locales/en.json"),
    include_str!("../../../ui/src/shared/i18n/locales/zh-Hans.json"),
    include_str!("../../../ui/src/shared/i18n/locales/ja.json"),
    include_str!("../../../ui/src/shared/i18n/locales/ko.json"),
    include_str!("../../../ui/src/shared/i18n/locales/zh-Hant.json"),
    include_str!("../../../ui/src/shared/i18n/locales/fr.json"),
    include_str!("../../../ui/src/shared/i18n/locales/fr-CA.json"),
    include_str!("../../../ui/src/shared/i18n/locales/de.json"),
    include_str!("../../../ui/src/shared/i18n/locales/it.json"),
    include_str!("../../../ui/src/shared/i18n/locales/es.json"),
    include_str!("../../../ui/src/shared/i18n/locales/es-419.json"),
    include_str!("../../../ui/src/shared/i18n/locales/nl.json"),
    include_str!("../../../ui/src/shared/i18n/locales/pt.json"),
    include_str!("../../../ui/src/shared/i18n/locales/pt-BR.json"),
];

fn locale(language: MenuLanguage) -> &'static Value {
    static PARSED: OnceLock<[Value; 15]> = OnceLock::new();
    let parsed = PARSED
        .get_or_init(|| LOCALES.map(|text| serde_json::from_str(text).unwrap_or(Value::Null)));
    &parsed[language.index()]
}

/// Returns the text of a dotted i18n key in a language, or `None` when the key has no text.
#[must_use]
pub fn lookup(language: MenuLanguage, key: &str) -> Option<String> {
    key.split('.')
        .try_fold(locale(language), |node, part| node.get(part))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use carafe_core::settings_menu::{self, MenuLanguage};

    use super::*;

    #[test]
    fn every_locale_parses() {
        for language in MenuLanguage::ALL {
            assert!(locale(language).is_object(), "{}", language.code());
        }
    }

    #[test]
    fn every_menu_key_has_a_text_in_every_language() {
        let mut missing = Vec::new();
        for key in settings_menu::keys() {
            for language in MenuLanguage::ALL {
                if lookup(language, &key).is_none() {
                    missing.push(format!("{} {key}", language.code()));
                }
            }
        }
        assert!(missing.is_empty(), "missing texts:\n{}", missing.join("\n"));
    }

    #[test]
    fn the_menu_renders_from_the_app_texts() {
        let menu = settings_menu::render(&[], &lookup).unwrap();
        assert!(menu.starts_with("carafe-menu\t2\nlanguages\tru\ten\t"));
    }

    #[test]
    fn the_menu_file_stays_under_the_console_limit() {
        let menu = settings_menu::render(&[], &lookup).unwrap();
        assert!(menu.len() < 1024 * 1024, "{} bytes", menu.len());
    }

    #[test]
    fn keys_are_looked_up_by_their_path() {
        assert_eq!(
            lookup(MenuLanguage::Ru, "padShort.t1").as_deref(),
            Some("B")
        );
        assert_eq!(lookup(MenuLanguage::Ru, "padShort"), None);
        assert_eq!(lookup(MenuLanguage::En, "no.such.key"), None);
    }
}
