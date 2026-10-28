//! `carafe/menu.txt`: what the console's settings menu can change, rendered from the catalog in every language
//! the console menu speaks (`docs/settings-menu.md`).

use std::fmt::Write as _;

use thiserror::Error;

use crate::package::GeneratedFile;
use crate::settings_catalog::{self, Api, Catalog, Choice, Condition, Entry, Label, Setting};

/// Path of the menu file in RomFS from its root.
pub const MENU_FILE: &str = "carafe/menu.txt";

/// Version written on the first line.
pub const MENU_VERSION: u32 = 2;

/// A language of the console menu, in the order its texts are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum MenuLanguage {
    Ru,
    En,
    ZhHans,
    Ja,
    Ko,
    ZhHant,
    Fr,
    FrCa,
    De,
    It,
    Es,
    Es419,
    Nl,
    Pt,
    PtBr,
}

impl MenuLanguage {
    /// All languages in file order.
    pub const ALL: [Self; 15] = [
        Self::Ru,
        Self::En,
        Self::ZhHans,
        Self::Ja,
        Self::Ko,
        Self::ZhHant,
        Self::Fr,
        Self::FrCa,
        Self::De,
        Self::It,
        Self::Es,
        Self::Es419,
        Self::Nl,
        Self::Pt,
        Self::PtBr,
    ];

    /// Returns the language's code, as the `languages` line and the app's i18n files name it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
            Self::ZhHans => "zh-Hans",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::ZhHant => "zh-Hant",
            Self::Fr => "fr",
            Self::FrCa => "fr-CA",
            Self::De => "de",
            Self::It => "it",
            Self::Es => "es",
            Self::Es419 => "es-419",
            Self::Nl => "nl",
            Self::Pt => "pt",
            Self::PtBr => "pt-BR",
        }
    }

    /// Returns the language's position in [`MenuLanguage::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Returns the text of an i18n key in a language, or `None` when there is none.
pub type Texts<'a> = dyn Fn(MenuLanguage, &str) -> Option<String> + 'a;

/// Error rendering the menu file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MenuError {
    /// A key has no text in a language.
    #[error("no {language:?} text for {key}")]
    MissingText {
        /// The language.
        language: MenuLanguage,
        /// The key.
        key: String,
    },
    /// A text holds a tab or a line break, or is empty.
    #[error("the text of {0} cannot go into the menu file")]
    BadText(String),
}

/// Returns `carafe/menu.txt` for a game that imports `apis`; empty `apis` means the game's Direct3D is unknown.
///
/// # Errors
///
/// [`MenuError`] when a text is missing or cannot be written.
pub fn render(apis: &[Api], texts: &Texts<'_>) -> Result<String, MenuError> {
    let catalog = settings_catalog::catalog();
    let mut out = Writer {
        out: String::new(),
        texts,
    };
    out.line(&["carafe-menu", &MENU_VERSION.to_string()]);
    let mut languages = vec!["languages"];
    languages.extend(MenuLanguage::ALL.map(MenuLanguage::code));
    out.line(&languages);
    let mut sorted = apis.to_vec();
    sorted.sort();
    sorted.dedup();
    for api in sorted {
        out.line(&["api", api.name()]);
    }
    for section in &catalog.sections {
        let names = out.labels(&key_label(&section.name))?;
        out.record(&["section", section.id.name()], &names);
        for entry in &section.entries {
            match entry {
                Entry::Setting(setting) => out.setting(catalog, setting)?,
                Entry::Group(group) => {
                    let names = out.labels(&key_label(&group.name))?;
                    out.record(
                        &["group", &group.id, &condition(catalog, &group.condition)],
                        &names,
                    );
                    for page in &group.pages {
                        if let (Some(id), Some(name)) = (&page.id, &page.name) {
                            let names = out.labels(&key_label(name))?;
                            out.record(&["page", id], &names);
                        }
                        for setting in &page.settings {
                            out.setting(catalog, setting)?;
                        }
                    }
                    out.line(&["end"]);
                }
            }
        }
        for preset in &section.presets {
            let mut texts = out.labels(&key_label(&preset.name))?;
            texts.extend(out.labels(&key_label(&preset.hint))?);
            out.record(&["preset", &preset.id], &texts);
            for value in &preset.values {
                out.line(&["set", &value.setting, &value.value.to_string()]);
            }
        }
    }
    for suggestion in &catalog.suggestions {
        let why = out.labels(&key_label(&suggestion.why))?;
        out.record(
            &[
                "suggest",
                suggestion.kind.name(),
                &suggestion.setting,
                &suggestion.value.to_string(),
            ],
            &why,
        );
    }
    out.choices("key", &catalog.keys, |code| format!("0x{code:x}"))?;
    out.choices("pad", &catalog.pads, |code| code.to_string())?;
    Ok(out.out)
}

/// Returns the menu file as a RomFS file.
///
/// # Errors
///
/// [`MenuError`] as [`render`].
pub fn file(apis: &[Api], texts: &Texts<'_>) -> Result<GeneratedFile, MenuError> {
    Ok(GeneratedFile {
        path: MENU_FILE.to_owned(),
        contents: render(apis, texts)?,
    })
}

/// Returns every i18n key the menu file needs, each once.
#[must_use]
pub fn keys() -> Vec<String> {
    let catalog = settings_catalog::catalog();
    let mut keys = Vec::new();
    let mut add = |label: &Label| {
        if let Label::Key { key } | Label::Unit { key, .. } = label {
            keys.push(key.clone());
        }
    };
    for section in &catalog.sections {
        add(&key_label(&section.name));
        for preset in &section.presets {
            add(&key_label(&preset.name));
            add(&key_label(&preset.hint));
        }
    }
    for group in catalog.groups() {
        add(&key_label(&group.name));
        for page in &group.pages {
            if let Some(name) = &page.name {
                add(&key_label(name));
            }
        }
    }
    for setting in catalog.settings() {
        add(&key_label(&setting.name));
        add(&key_label(&setting.hint));
        for value in &setting.values {
            add(&value.label);
        }
    }
    for suggestion in &catalog.suggestions {
        add(&key_label(&suggestion.why));
    }
    for choice in catalog.keys.iter().chain(&catalog.pads) {
        add(&choice.label);
    }
    keys.sort();
    keys.dedup();
    keys
}

fn key_label(key: &str) -> Label {
    Label::Key {
        key: key.to_owned(),
    }
}

fn condition(catalog: &Catalog, condition: &Condition) -> String {
    let (kind, setting, values) = match condition {
        Condition::Always => return "-".to_owned(),
        Condition::Show { setting, values } => ("show", setting, values),
        Condition::On { setting, values } => ("on", setting, values),
    };
    debug_assert!(catalog.setting(setting).is_some());
    let values: Vec<String> = values.iter().map(ToString::to_string).collect();
    format!("{kind}:{setting}={}", values.join(","))
}

struct Writer<'a, 'b> {
    out: String,
    texts: &'a Texts<'b>,
}

impl Writer<'_, '_> {
    fn line(&mut self, fields: &[&str]) {
        let _ = writeln!(self.out, "{}", fields.join("\t"));
    }

    fn record(&mut self, fields: &[&str], texts: &[String]) {
        let mut all: Vec<&str> = fields.to_vec();
        all.extend(texts.iter().map(String::as_str));
        self.line(&all);
    }

    fn labels(&self, label: &Label) -> Result<Vec<String>, MenuError> {
        MenuLanguage::ALL
            .iter()
            .map(|&language| self.label(language, label))
            .collect()
    }

    fn label(&self, language: MenuLanguage, label: &Label) -> Result<String, MenuError> {
        let (key, text) = match label {
            Label::Text { text } => (text.as_str(), text.clone()),
            Label::Key { key } => (key.as_str(), self.text(language, key)?),
            Label::Unit { key, value } => (
                key.as_str(),
                self.text(language, key)?.replace("{{value}}", value),
            ),
        };
        if text.is_empty() || text.contains(['\t', '\n', '\r']) {
            return Err(MenuError::BadText(key.to_owned()));
        }
        Ok(text)
    }

    fn text(&self, language: MenuLanguage, key: &str) -> Result<String, MenuError> {
        (self.texts)(language, key).ok_or_else(|| MenuError::MissingText {
            language,
            key: key.to_owned(),
        })
    }

    fn setting(&mut self, catalog: &Catalog, setting: &Setting) -> Result<(), MenuError> {
        let apply = match setting.apply {
            settings_catalog::Apply::Now => "now",
            settings_catalog::Apply::Restart => "restart",
        };
        let apis = if setting.apis.is_empty() {
            "-".to_owned()
        } else {
            setting
                .apis
                .iter()
                .map(|api| api.name())
                .collect::<Vec<_>>()
                .join(",")
        };
        let mut texts = self.labels(&key_label(&setting.name))?;
        texts.extend(self.labels(&key_label(&setting.hint))?);
        self.record(
            &[
                "setting",
                &setting.id,
                setting.file.name(),
                &setting.key,
                apply,
                &setting.default.to_string(),
                &condition(catalog, &setting.condition),
                &apis,
            ],
            &texts,
        );
        for value in &setting.values {
            let labels = self.labels(&value.label)?;
            self.record(&["value", value.text.as_deref().unwrap_or("-")], &labels);
        }
        Ok(())
    }

    fn choices(
        &mut self,
        kind: &str,
        choices: &[Choice],
        code: impl Fn(u16) -> String,
    ) -> Result<(), MenuError> {
        for choice in choices {
            let labels = self.labels(&choice.label)?;
            self.record(&[kind, &code(choice.code)], &labels);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: MenuLanguage, key: &str) -> Option<String> {
        let tag = language.code();
        Some(if key.ends_with(".mib") || key.ends_with(".percent") {
            format!("{{{{value}}}} {tag}")
        } else {
            format!("{tag}:{key}")
        })
    }

    fn fields<'a>(menu: &'a str, kind: &str, id: &str) -> Vec<&'a str> {
        menu.lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .find(|fields| fields[0] == kind && fields.get(1) == Some(&id))
            .unwrap_or_else(|| panic!("no {kind} {id}"))
    }

    #[test]
    fn languages_have_unique_codes_in_file_order() {
        let codes: Vec<&str> = MenuLanguage::ALL.map(MenuLanguage::code).to_vec();
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len());
        for (position, language) in MenuLanguage::ALL.iter().enumerate() {
            assert_eq!(language.index(), position);
        }
        assert_eq!(&codes[..3], ["ru", "en", "zh-Hans"]);
    }

    #[test]
    fn starts_with_the_version_the_languages_and_the_game_apis() {
        let menu = render(&[Api::D3d9, Api::D3d8, Api::D3d9], &texts).unwrap();
        let mut lines = menu.lines();
        assert_eq!(lines.next(), Some("carafe-menu\t2"));
        assert_eq!(
            lines.next(),
            Some(
                "languages\tru\ten\tzh-Hans\tja\tko\tzh-Hant\tfr\tfr-CA\tde\tit\tes\tes-419\tnl\tpt\tpt-BR"
            )
        );
        assert_eq!(lines.next(), Some("api\td3d8"));
        assert_eq!(lines.next(), Some("api\td3d9"));
        assert_eq!(
            lines.next().map(|l| l.split('\t').next()),
            Some(Some("section"))
        );
        let unknown = render(&[], &texts).unwrap();
        assert!(!unknown.lines().any(|line| line.starts_with("api\t")));
    }

    #[test]
    fn sections_come_in_order() {
        let menu = render(&[], &texts).unwrap();
        let sections: Vec<&str> = menu
            .lines()
            .filter_map(|line| line.strip_prefix("section\t"))
            .map(|rest| rest.split('\t').next().unwrap())
            .collect();
        assert_eq!(sections, ["controls", "graphics", "system", "experimental"]);
    }

    #[test]
    fn a_setting_line_carries_everything_the_menu_needs() {
        let menu = render(&[], &texts).unwrap();
        let tso = fields(&menu, "setting", "fex-tso");
        assert_eq!(
            &tso[..8],
            [
                "setting",
                "fex-tso",
                "program",
                "FEX_TSOENABLED",
                "restart",
                "0",
                "-",
                "-"
            ]
        );
        let count = MenuLanguage::ALL.len();
        assert_eq!(tso[8], "ru:catalog.settings.fex-tso.name");
        assert_eq!(tso[8 + count - 1], "pt-BR:catalog.settings.fex-tso.name");
        assert_eq!(tso[8 + count + 2], "zh-Hans:catalog.settings.fex-tso.hint");
        assert_eq!(tso.len(), 8 + 2 * count);
        let budget = fields(&menu, "setting", "d3d9-maxAvailableMemory");
        assert_eq!(budget[7], "d3d8,d3d9");
        let fps = fields(&menu, "setting", "frame-limit");
        assert_eq!(fps[4], "now");
        let vector = fields(&menu, "setting", "fex-vector-tso");
        assert_eq!(vector[6], "on:fex-tso=1");
    }

    #[test]
    fn values_follow_their_setting_and_absent_is_a_dash() {
        let menu = render(&[], &texts).unwrap();
        let lines: Vec<&str> = menu.lines().collect();
        let at = lines
            .iter()
            .position(|line| line.starts_with("setting\tdxvk-maxMemoryBudget\t"))
            .unwrap();
        assert!(lines[at + 1].starts_with("value\t-\t"));
        assert!(lines[at + 2].starts_with("value\t0\t"));
        let labels: Vec<String> = MenuLanguage::ALL
            .iter()
            .map(|language| format!("128 {}", language.code()))
            .collect();
        assert_eq!(lines[at + 3], format!("value\t128\t{}", labels.join("\t")));
    }

    #[test]
    fn groups_end_and_pages_open_inside_them() {
        let menu = render(&[], &texts).unwrap();
        let group = fields(&menu, "group", "dxvk");
        assert_eq!(group[2], "-");
        let lines: Vec<&str> = menu.lines().collect();
        let start = lines
            .iter()
            .position(|line| line.starts_with("group\tdxvk\t"))
            .unwrap();
        let end = start
            + lines[start..]
                .iter()
                .position(|line| *line == "end")
                .unwrap();
        let pages = lines[start..end]
            .iter()
            .filter(|line| line.starts_with("page\t"))
            .count();
        assert_eq!(pages, 8);
        let settings = lines[start..end]
            .iter()
            .filter(|line| line.starts_with("setting\t"))
            .count();
        assert_eq!(settings, 51);
        assert_eq!(
            menu.lines()
                .filter(|line| line.starts_with("group\t"))
                .count(),
            2
        );
        assert_eq!(menu.lines().filter(|line| *line == "end").count(), 2);
    }

    #[test]
    fn presets_suggestions_and_choices_are_written() {
        let menu = render(&[], &texts).unwrap();
        let preset = fields(&menu, "preset", "fast");
        assert_eq!(preset.len(), 2 + 2 * MenuLanguage::ALL.len());
        assert!(menu.contains("set\tfex-maxinst\t4\n"));
        let suggest: Vec<&str> = menu
            .lines()
            .filter(|line| line.starts_with("suggest\tmemory\t"))
            .collect();
        assert_eq!(suggest.len(), 3);
        let space = vec!["Space"; MenuLanguage::ALL.len()].join("\t");
        assert!(menu.contains(&format!("key\t0x20\t{space}\n")));
        assert!(menu.contains("key\t0x100\tru:mouseShort.left\t"));
        assert!(menu.contains("pad\t24\tru:padShort.t24\t"));
    }

    #[test]
    fn missing_or_broken_texts_are_errors() {
        let none = |_: MenuLanguage, _: &str| None;
        assert!(matches!(
            render(&[], &none),
            Err(MenuError::MissingText { .. })
        ));
        let tabbed = |_: MenuLanguage, _: &str| Some("a\tb".to_owned());
        assert!(matches!(render(&[], &tabbed), Err(MenuError::BadText(_))));
    }

    #[test]
    fn every_needed_key_is_listed_once() {
        let keys = keys();
        assert!(keys.contains(&"catalog.settings.fex-tso.name".to_owned()));
        assert!(keys.contains(&"catalog.values.mib".to_owned()));
        assert!(keys.contains(&"padShort.t1".to_owned()));
        let mut sorted = keys.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
    }

    #[test]
    fn file_lands_at_the_menu_path() {
        assert_eq!(file(&[], &texts).unwrap().path, "carafe/menu.txt");
    }
}
