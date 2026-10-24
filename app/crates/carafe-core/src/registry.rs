//! The game's Windows registry: entries the package lays over it, rows of the console menu that change it,
//! `.reg` files imported and exported in the app, and the `carafe/registry.txt` file the runtime reads.

use std::fmt::Write as _;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use ts_rs::TS;

/// Path of the registry file inside the package's RomFS.
pub const REGISTRY_FILE: &str = "carafe/registry.txt";

const REGISTRY_HEADER: &str = "carafe-registry\t1";
const MAX_ROWS: usize = 64;
const MAX_FIELDS: usize = 8;
const MAX_OPTIONS: usize = 32;
const MAX_OPTION_BYTES: usize = 1024;
const MAX_NAME_CHARS: usize = 255;

const REG_NONE: u32 = 0;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_BINARY: u32 = 3;
const REG_DWORD: u32 = 4;
const REG_MULTI_SZ: u32 = 7;
const REG_QWORD: u32 = 11;

/// A root key of the game's registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RegistryRoot {
    /// `HKEY_CURRENT_USER`: the user's settings, where games keep most of theirs.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE`: settings of the whole system.
    LocalMachine,
}

impl RegistryRoot {
    /// Returns the short name the runtime reads: `HKCU` or `HKLM`.
    #[must_use]
    pub fn short_name(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU",
            Self::LocalMachine => "HKLM",
        }
    }

    /// Returns the full name used in `.reg` files.
    #[must_use]
    pub fn long_name(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKEY_CURRENT_USER",
            Self::LocalMachine => "HKEY_LOCAL_MACHINE",
        }
    }
}

/// The data of a registry value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum RegistryValue {
    /// `REG_SZ`.
    String {
        /// The text.
        text: String,
    },
    /// `REG_EXPAND_SZ`: text with `%VARIABLES%`.
    ExpandString {
        /// The text.
        text: String,
    },
    /// `REG_MULTI_SZ`.
    MultiString {
        /// The strings in order.
        items: Vec<String>,
    },
    /// `REG_DWORD`.
    Dword {
        /// The number.
        number: u32,
    },
    /// `REG_QWORD`.
    Qword {
        /// The number.
        number: Qword,
    },
    /// `REG_BINARY`.
    Binary {
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// Any other type, kept byte for byte.
    Other {
        /// The type number.
        kind: u32,
        /// The bytes.
        bytes: Vec<u8>,
    },
}

/// A 64-bit registry number, written as decimal text so that it survives JavaScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, type = "string")]
pub struct Qword(pub u64);

impl Serialize for Qword {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Qword {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Text {
            Text(String),
            Number(u64),
        }
        match Text::deserialize(deserializer)? {
            Text::Text(text) => text
                .trim()
                .parse()
                .map(Self)
                .map_err(serde::de::Error::custom),
            Text::Number(number) => Ok(Self(number)),
        }
    }
}

impl RegistryValue {
    /// Returns the Windows type number of the value.
    #[must_use]
    pub fn type_number(&self) -> u32 {
        match self {
            Self::String { .. } => REG_SZ,
            Self::ExpandString { .. } => REG_EXPAND_SZ,
            Self::MultiString { .. } => REG_MULTI_SZ,
            Self::Dword { .. } => REG_DWORD,
            Self::Qword { .. } => REG_QWORD,
            Self::Binary { .. } => REG_BINARY,
            Self::Other { kind, .. } => *kind,
        }
    }

    /// Returns the bytes the registry stores: strings in UTF-16LE with their terminating zeros, numbers in
    /// little-endian order.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        match self {
            Self::String { text } | Self::ExpandString { text } => utf16_bytes(text, true),
            Self::MultiString { items } => {
                let mut bytes = Vec::new();
                for item in items {
                    bytes.extend(utf16_bytes(item, true));
                }
                bytes.extend([0, 0]);
                bytes
            }
            Self::Dword { number } => number.to_le_bytes().to_vec(),
            Self::Qword { number } => number.0.to_le_bytes().to_vec(),
            Self::Binary { bytes } | Self::Other { bytes, .. } => bytes.clone(),
        }
    }

    fn from_bytes(kind: u32, bytes: Vec<u8>, wide: bool) -> Self {
        match kind {
            REG_SZ => Self::String {
                text: text_from(&bytes, wide),
            },
            REG_EXPAND_SZ => Self::ExpandString {
                text: text_from(&bytes, wide),
            },
            REG_MULTI_SZ => Self::MultiString {
                items: items_from(&bytes, wide),
            },
            REG_DWORD if bytes.len() == 4 => Self::Dword {
                number: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            },
            REG_QWORD if bytes.len() == 8 => {
                let mut array = [0; 8];
                array.copy_from_slice(&bytes);
                Self::Qword {
                    number: Qword(u64::from_le_bytes(array)),
                }
            }
            REG_BINARY => Self::Binary { bytes },
            _ => Self::Other { kind, bytes },
        }
    }
}

/// What an entry does to the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RegistryChange {
    /// Writes the value, creating its key when it is missing.
    Set {
        /// The data written.
        value: RegistryValue,
    },
    /// Removes the value.
    DeleteValue,
    /// Removes the key with everything under it.
    DeleteKey,
}

/// One change laid over the game's registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistryEntry {
    /// The root key.
    pub root: RegistryRoot,
    /// The key under the root, its parts separated by `\`.
    pub path: String,
    /// The value's name; empty for the key's default value and for [`RegistryChange::DeleteKey`].
    pub name: String,
    /// What is done.
    pub change: RegistryChange,
}

/// The type of a value a console menu row writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RegistryKind {
    /// `REG_SZ`.
    String,
    /// `REG_EXPAND_SZ`.
    ExpandString,
    /// `REG_DWORD`.
    Dword,
    /// `REG_QWORD`.
    Qword,
}

impl RegistryKind {
    fn type_number(self) -> u32 {
        match self {
            Self::String => REG_SZ,
            Self::ExpandString => REG_EXPAND_SZ,
            Self::Dword => REG_DWORD,
            Self::Qword => REG_QWORD,
        }
    }

    /// Turns the text of a variant into the value it writes.
    ///
    /// # Errors
    ///
    /// [`RegistryError::OptionValue`] if a number is not a decimal or `0x` hexadecimal number of the kind's size.
    pub fn value(self, text: &str) -> Result<RegistryValue, RegistryError> {
        let bad = || RegistryError::OptionValue(text.to_owned());
        Ok(match self {
            Self::String => RegistryValue::String {
                text: text.to_owned(),
            },
            Self::ExpandString => RegistryValue::ExpandString {
                text: text.to_owned(),
            },
            Self::Dword => RegistryValue::Dword {
                number: u32::try_from(parse_number(text).ok_or_else(bad)?).map_err(|_| bad())?,
            },
            Self::Qword => RegistryValue::Qword {
                number: Qword(parse_number(text).ok_or_else(bad)?),
            },
        })
    }
}

fn parse_number(text: &str) -> Option<u64> {
    let text = text.trim();
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

/// A value a console menu row writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistryField {
    /// The root key.
    pub root: RegistryRoot,
    /// The key under the root.
    pub path: String,
    /// The value's name; empty for the default value.
    pub name: String,
    /// The value's type.
    pub kind: RegistryKind,
}

/// A choice of a console menu row: one text per field, in the order of the fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistryOption {
    /// What the menu shows.
    pub label: String,
    /// The value written into each field, as text: a number for numbers.
    pub values: Vec<String>,
}

/// A row of the console menu's "Game registry" screen, shown like any other setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistryMenuRow {
    /// The row's name.
    pub name: String,
    /// What the row changes, for the card beside the list.
    #[serde(default)]
    pub hint: String,
    /// The values the row writes together.
    pub fields: Vec<RegistryField>,
    /// The choices.
    pub options: Vec<RegistryOption>,
}

/// When the package's entries are laid over the game's registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RegistryWriteMode {
    /// At the first start and after the game is built with other entries: what the game or the player changes later
    /// stays.
    #[default]
    Once,
    /// At every start, over whatever the game wrote.
    EveryStart,
}

/// The game's registry settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegistrySettings {
    /// When the entries are written.
    #[serde(default)]
    pub mode: RegistryWriteMode,
    /// The entries, in the order they are written.
    #[serde(default)]
    pub entries: Vec<RegistryEntry>,
    /// The rows of the console menu.
    #[serde(default)]
    pub menu: Vec<RegistryMenuRow>,
}

/// A registry setting that cannot go into the package.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    /// A key path has an empty part, a tab or a line break.
    #[error("registry key {0:?} is not a valid path")]
    Path(String),
    /// A value name is too long or has a tab or a line break.
    #[error("registry value name {0:?} is not valid")]
    Name(String),
    /// A key cannot be deleted without a path.
    #[error("a whole root key cannot be deleted")]
    DeleteRoot,
    /// A menu row has no name, no fields, no choices or too many of them.
    #[error("menu row {0:?} is incomplete")]
    Row(String),
    /// A choice has a different number of values than its row has fields.
    #[error("choice {0:?} does not have one value per field")]
    OptionCount(String),
    /// A choice's value does not fit its field.
    #[error("value {0:?} does not fit its field")]
    OptionValue(String),
    /// A text has a tab or a line break, which the menu file cannot carry.
    #[error("text {0:?} has a tab or a line break")]
    Text(String),
}

fn plain(text: &str) -> bool {
    !text.contains(['\t', '\n', '\r'])
}

fn check_path(path: &str) -> Result<(), RegistryError> {
    let trimmed = path.trim_matches('\\');
    if !plain(path) || (!trimmed.is_empty() && trimmed.split('\\').any(str::is_empty)) {
        return Err(RegistryError::Path(path.to_owned()));
    }
    Ok(())
}

fn check_name(name: &str) -> Result<(), RegistryError> {
    if !plain(name) || name.chars().count() > MAX_NAME_CHARS {
        return Err(RegistryError::Name(name.to_owned()));
    }
    Ok(())
}

impl RegistrySettings {
    /// Returns whether there is nothing to put into the package.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.menu.is_empty()
    }

    /// Checks that every entry and menu row can go into the package.
    ///
    /// # Errors
    ///
    /// The first [`RegistryError`] found.
    pub fn validate(&self) -> Result<(), RegistryError> {
        for entry in &self.entries {
            check_path(&entry.path)?;
            check_name(&entry.name)?;
            if entry.change == RegistryChange::DeleteKey && entry.path.trim_matches('\\').is_empty()
            {
                return Err(RegistryError::DeleteRoot);
            }
        }
        if self.menu.len() > MAX_ROWS {
            return Err(RegistryError::Row(String::new()));
        }
        for row in &self.menu {
            let incomplete = row.name.trim().is_empty()
                || row.fields.is_empty()
                || row.fields.len() > MAX_FIELDS
                || row.options.is_empty()
                || row.options.len() > MAX_OPTIONS;
            if incomplete {
                return Err(RegistryError::Row(row.name.clone()));
            }
            if !plain(&row.name) || !plain(&row.hint) {
                return Err(RegistryError::Text(row.name.clone()));
            }
            for field in &row.fields {
                check_path(&field.path)?;
                check_name(&field.name)?;
            }
            for option in &row.options {
                if !plain(&option.label) {
                    return Err(RegistryError::Text(option.label.clone()));
                }
                if option.values.len() != row.fields.len() {
                    return Err(RegistryError::OptionCount(option.label.clone()));
                }
                for (field, text) in row.fields.iter().zip(&option.values) {
                    let value = field.kind.value(text)?;
                    if value.bytes().len() > MAX_OPTION_BYTES {
                        return Err(RegistryError::OptionValue(text.clone()));
                    }
                }
            }
        }
        Ok(())
    }
}

fn utf16_bytes(text: &str, terminated: bool) -> Vec<u8> {
    let mut bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    if terminated {
        bytes.extend([0, 0]);
    }
    bytes
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn trimmed_path(path: &str) -> &str {
    path.trim_matches('\\')
}

/// Renders `carafe/registry.txt`, which the runtime reads when the game's registry is loaded.
///
/// # Returns
///
/// `None` when there are no entries and no menu rows.
#[must_use]
pub fn render_registry_file(settings: &RegistrySettings) -> Option<String> {
    if settings.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str(REGISTRY_HEADER);
    out.push('\n');
    let mode = match settings.mode {
        RegistryWriteMode::Once => "once",
        RegistryWriteMode::EveryStart => "always",
    };
    let _ = writeln!(out, "mode\t{mode}");
    for entry in &settings.entries {
        let root = entry.root.short_name();
        let path = trimmed_path(&entry.path);
        match &entry.change {
            RegistryChange::Set { value } => {
                let _ = writeln!(
                    out,
                    "set\t{root}\t{path}\t{}\t{}\t{}",
                    entry.name,
                    value.type_number(),
                    hex(&value.bytes())
                );
            }
            RegistryChange::DeleteValue => {
                let _ = writeln!(out, "delete\t{root}\t{path}\t{}", entry.name);
            }
            RegistryChange::DeleteKey => {
                let _ = writeln!(out, "deletekey\t{root}\t{path}");
            }
        }
    }
    for row in &settings.menu {
        let _ = writeln!(out, "row\t{}\t{}", row.name, row.hint);
        for field in &row.fields {
            let _ = writeln!(
                out,
                "field\t{}\t{}\t{}\t{}",
                field.root.short_name(),
                trimmed_path(&field.path),
                field.name,
                field.kind.type_number()
            );
        }
        for option in &row.options {
            out.push_str("option\t");
            out.push_str(&option.label);
            for (field, text) in row.fields.iter().zip(&option.values) {
                let bytes = field
                    .kind
                    .value(text)
                    .map(|value| value.bytes())
                    .unwrap_or_default();
                out.push('\t');
                out.push_str(&hex(&bytes));
            }
            out.push('\n');
        }
    }
    Some(out)
}

/// What could not be taken from a `.reg` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RegSkipReason {
    /// A key under `HKEY_USERS`, `HKEY_CURRENT_CONFIG` or a root the game's registry does not have.
    UnknownRoot,
    /// A value before any key line.
    NoKey,
    /// A value line that could not be read.
    BadValue,
}

/// A line of a `.reg` file that was not imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegSkip {
    /// The line number, from 1.
    pub line: u32,
    /// The line as it is in the file.
    pub text: String,
    /// Why it was not imported.
    pub reason: RegSkipReason,
}

/// What a `.reg` file holds.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegImport {
    /// The entries, in the file's order.
    pub entries: Vec<RegistryEntry>,
    /// The lines left out.
    pub skipped: Vec<RegSkip>,
}

/// A file that is not a `.reg` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RegParseError {
    /// The first line is not `Windows Registry Editor Version 5.00` or `REGEDIT4`.
    #[error("the file is not a Windows registry file")]
    NotRegFile,
}

const CP1251: [char; 128] = [
    'Ђ', 'Ѓ', '‚', 'ѓ', '„', '…', '†', '‡', '€', '‰', 'Љ', '‹', 'Њ', 'Ќ', 'Ћ', 'Џ', 'ђ', '‘', '’',
    '“', '”', '•', '–', '—', '\u{98}', '™', 'љ', '›', 'њ', 'ќ', 'ћ', 'џ', '\u{a0}', 'Ў', 'ў', 'Ј',
    '¤', 'Ґ', '¦', '§', 'Ё', '©', 'Є', '«', '¬', '\u{ad}', '®', 'Ї', '°', '±', 'І', 'і', 'ґ', 'µ',
    '¶', '·', 'ё', '№', 'є', '»', 'ј', 'Ѕ', 'ѕ', 'ї', 'А', 'Б', 'В', 'Г', 'Д', 'Е', 'Ж', 'З', 'И',
    'Й', 'К', 'Л', 'М', 'Н', 'О', 'П', 'Р', 'С', 'Т', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Ш', 'Щ', 'Ъ', 'Ы',
    'Ь', 'Э', 'Ю', 'Я', 'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о',
    'п', 'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ы', 'ь', 'э', 'ю', 'я',
];

fn narrow_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes
            .iter()
            .map(|&byte| {
                if byte < 0x80 {
                    char::from(byte)
                } else {
                    CP1251[usize::from(byte - 0x80)]
                }
            })
            .collect(),
    }
}

fn wide_units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, _) = bytes.as_chunks::<2>();
    pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect()
}

fn text_from(bytes: &[u8], wide: bool) -> String {
    if wide {
        let mut units = wide_units(bytes);
        while units.last() == Some(&0) {
            units.pop();
        }
        String::from_utf16_lossy(&units)
    } else {
        let mut end = bytes.len();
        while end > 0 && bytes[end - 1] == 0 {
            end -= 1;
        }
        narrow_text(&bytes[..end])
    }
}

fn items_from(bytes: &[u8], wide: bool) -> Vec<String> {
    let text = if wide {
        String::from_utf16_lossy(&wide_units(bytes))
    } else {
        narrow_text(bytes)
    };
    let mut items: Vec<String> = text.split('\0').map(str::to_owned).collect();
    while items.last().is_some_and(String::is_empty) {
        items.pop();
    }
    items
}

fn decode(bytes: &[u8]) -> (String, bool) {
    if let Some(rest) = bytes.strip_prefix([0xFF, 0xFE].as_slice()) {
        return (String::from_utf16_lossy(&wide_units(rest)), true);
    }
    let rest = bytes
        .strip_prefix([0xEF, 0xBB, 0xBF].as_slice())
        .unwrap_or(bytes);
    (narrow_text(rest), false)
}

fn split_root(key: &str) -> Option<(RegistryRoot, String)> {
    let key = key.trim().trim_matches('\\');
    let (root, rest) = key.split_once('\\').unwrap_or((key, ""));
    let rest = rest.trim_matches('\\');
    let upper = root.to_ascii_uppercase();
    match upper.as_str() {
        "HKEY_CURRENT_USER" | "HKCU" => Some((RegistryRoot::CurrentUser, rest.to_owned())),
        "HKEY_LOCAL_MACHINE" | "HKLM" => Some((RegistryRoot::LocalMachine, without_wow64(rest))),
        "HKEY_CLASSES_ROOT" | "HKCR" => {
            let path = if rest.is_empty() {
                "Software\\Classes".to_owned()
            } else {
                format!("Software\\Classes\\{rest}")
            };
            Some((RegistryRoot::LocalMachine, path))
        }
        _ => None,
    }
}

fn without_wow64(path: &str) -> String {
    let mut parts: Vec<&str> = path.split('\\').collect();
    if parts.len() >= 2
        && parts[0].eq_ignore_ascii_case("Software")
        && parts[1].eq_ignore_ascii_case("Wow6432Node")
    {
        parts.remove(1);
    }
    parts.join("\\")
}

fn parse_quoted(text: &str) -> Option<(String, &str)> {
    let mut chars = text.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return Some((out, &text[index + 1..])),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                '0' => return None,
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

fn only_comment(rest: &str) -> bool {
    let rest = rest.trim();
    rest.is_empty() || rest.starts_with(';')
}

fn parse_hex_bytes(text: &str) -> Option<Vec<u8>> {
    let text = text.split(';').next().unwrap_or("");
    let mut bytes = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        bytes.push(u8::from_str_radix(part, 16).ok()?);
    }
    Some(bytes)
}

fn parse_data(text: &str, wide: bool) -> Option<Option<RegistryValue>> {
    let text = text.trim_start();
    if let Some(rest) = text.strip_prefix('-') {
        return only_comment(rest).then_some(None);
    }
    if text.starts_with('"') {
        let (value, rest) = parse_quoted(text)?;
        return only_comment(rest).then_some(Some(RegistryValue::String { text: value }));
    }
    if let Some(rest) = text.strip_prefix("dword:") {
        let digits = rest.split(';').next().unwrap_or("").trim();
        if digits.is_empty() || digits.len() > 8 {
            return None;
        }
        return Some(Some(RegistryValue::Dword {
            number: u32::from_str_radix(digits, 16).ok()?,
        }));
    }
    if let Some(rest) = text.strip_prefix("hex:") {
        return Some(Some(RegistryValue::Binary {
            bytes: parse_hex_bytes(rest)?,
        }));
    }
    let rest = text.strip_prefix("hex(")?;
    let (kind, rest) = rest.split_once("):")?;
    if kind.is_empty() || kind.starts_with("0x") {
        return None;
    }
    let kind = u32::from_str_radix(kind, 16).ok()?;
    let bytes = parse_hex_bytes(rest)?;
    Some(Some(if kind == REG_NONE {
        RegistryValue::Other { kind, bytes }
    } else {
        RegistryValue::from_bytes(kind, bytes, wide)
    }))
}

fn logical_lines(text: &str) -> Vec<(u32, String)> {
    let mut lines = Vec::new();
    let mut raw = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    let mut number = 0u32;
    while let Some(line) = raw.next() {
        number += 1;
        let start = number;
        let mut joined = line.to_owned();
        while let Some(cut) = continuation_cut(&joined) {
            let Some(next) = raw.next() else {
                break;
            };
            number += 1;
            let next = next.trim_start();
            if next.is_empty() || next.starts_with(';') {
                continue;
            }
            joined.truncate(cut);
            joined.push_str(next);
        }
        lines.push((start, joined));
    }
    lines
}

fn continuation_cut(line: &str) -> Option<usize> {
    let at = line.find("=hex").or_else(|| line.find("= hex"))?;
    let end = at + line[at..].find(';').unwrap_or(line.len() - at);
    line[..end].trim_end().strip_suffix('\\').map(str::len)
}

/// Reads a `.reg` file as Windows' and Wine's `regedit` read it.
///
/// UTF-16LE files start with `FF FE`; others are read as UTF-8 and, when that fails, as Windows-1251.
/// `HKEY_CLASSES_ROOT` becomes `HKEY_LOCAL_MACHINE\Software\Classes`, and `Software\Wow6432Node` under
/// `HKEY_LOCAL_MACHINE` becomes `Software`: the game's registry has a single view for 32- and 64-bit programs.
///
/// # Errors
///
/// [`RegParseError::NotRegFile`] if the first line is not a registry editor header.
pub fn parse_reg(bytes: &[u8]) -> Result<RegImport, RegParseError> {
    let (text, wide) = decode(bytes);
    let lines = logical_lines(&text);
    let mut rest = lines.iter().skip_while(|(_, line)| line.trim().is_empty());
    let header = rest.next().map(|(_, line)| line.trim()).unwrap_or("");
    if header != "Windows Registry Editor Version 5.00" && header != "REGEDIT4" {
        return Err(RegParseError::NotRegFile);
    }
    let mut import = RegImport::default();
    let mut current: Option<(RegistryRoot, String)> = None;
    let mut key_known = false;
    for (number, line) in rest {
        let trimmed = line.trim_start();
        let skip = |reason| RegSkip {
            line: *number,
            text: line.clone(),
            reason,
        };
        if let Some(body) = trimmed.strip_prefix('[') {
            if body.starts_with(' ') {
                continue;
            }
            let Some(end) = body.rfind(']') else {
                continue;
            };
            let key = &body[..end];
            key_known = true;
            if let Some(deleted) = key.strip_prefix('-') {
                current = None;
                match split_root(deleted) {
                    Some((root, path)) if !path.is_empty() => import.entries.push(RegistryEntry {
                        root,
                        path,
                        name: String::new(),
                        change: RegistryChange::DeleteKey,
                    }),
                    Some(_) => {}
                    None => import.skipped.push(skip(RegSkipReason::UnknownRoot)),
                }
                continue;
            }
            current = split_root(key);
            if current.is_none() {
                import.skipped.push(skip(RegSkipReason::UnknownRoot));
            }
            continue;
        }
        let (name, data) = if let Some(after) = trimmed.strip_prefix('@') {
            (Some(String::new()), after)
        } else if trimmed.starts_with('"') {
            match parse_quoted(trimmed) {
                Some((name, after)) => (Some(name), after),
                None => (None, ""),
            }
        } else {
            continue;
        };
        let Some((root, path)) = current.clone() else {
            if key_known {
                continue;
            }
            import.skipped.push(skip(RegSkipReason::NoKey));
            continue;
        };
        let parsed = name.and_then(|name| {
            let data = data.trim_start().strip_prefix('=')?;
            Some((name, parse_data(data, wide)?))
        });
        match parsed {
            Some((name, Some(value))) => import.entries.push(RegistryEntry {
                root,
                path,
                name,
                change: RegistryChange::Set { value },
            }),
            Some((name, None)) => import.entries.push(RegistryEntry {
                root,
                path,
                name,
                change: RegistryChange::DeleteValue,
            }),
            None => import.skipped.push(skip(RegSkipReason::BadValue)),
        }
    }
    Ok(import)
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn hex_list(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn reg_data(value: &RegistryValue) -> String {
    match value {
        RegistryValue::String { text } if !text.contains('\0') => quote(text),
        RegistryValue::Dword { number } => format!("dword:{number:08x}"),
        RegistryValue::Binary { bytes } => format!("hex:{}", hex_list(bytes)),
        other => format!(
            "hex({:x}):{}",
            other.type_number(),
            hex_list(&other.bytes())
        ),
    }
}

/// Writes the entries as a `.reg` file in the shape Windows' `regedit` exports: UTF-16LE with a byte order mark and
/// `Windows Registry Editor Version 5.00`.
#[must_use]
pub fn export_reg(entries: &[RegistryEntry]) -> Vec<u8> {
    let mut text = String::from("Windows Registry Editor Version 5.00\r\n");
    let mut open: Option<(RegistryRoot, String)> = None;
    for entry in entries {
        let path = trimmed_path(&entry.path);
        let key = if path.is_empty() {
            entry.root.long_name().to_owned()
        } else {
            format!("{}\\{path}", entry.root.long_name())
        };
        if entry.change == RegistryChange::DeleteKey {
            let _ = write!(text, "\r\n[-{key}]\r\n");
            open = None;
            continue;
        }
        let here = (entry.root, path.to_ascii_lowercase());
        if open.as_ref() != Some(&here) {
            let _ = write!(text, "\r\n[{key}]\r\n");
            open = Some(here);
        }
        let name = if entry.name.is_empty() {
            "@".to_owned()
        } else {
            quote(&entry.name)
        };
        match &entry.change {
            RegistryChange::Set { value } => {
                let _ = write!(text, "{name}={}\r\n", reg_data(value));
            }
            RegistryChange::DeleteValue => {
                let _ = write!(text, "{name}=-\r\n");
            }
            RegistryChange::DeleteKey => {}
        }
    }
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(utf16_bytes(&text, false));
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide_file(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(utf16_bytes(text, false));
        bytes
    }

    fn set(root: RegistryRoot, path: &str, name: &str, value: RegistryValue) -> RegistryEntry {
        RegistryEntry {
            root,
            path: path.to_owned(),
            name: name.to_owned(),
            change: RegistryChange::Set { value },
        }
    }

    #[test]
    fn version_5_files_in_utf16_are_read() {
        let file = wide_file(
            "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\Studio\\Game]\r\n\
             \"Width\"=dword:00000500\r\n\"Language\"=\"ru\"\r\n@=\"default\"\r\n",
        );
        let import = parse_reg(&file).unwrap();
        assert!(import.skipped.is_empty());
        assert_eq!(
            import.entries,
            vec![
                set(
                    RegistryRoot::CurrentUser,
                    "Software\\Studio\\Game",
                    "Width",
                    RegistryValue::Dword { number: 1280 }
                ),
                set(
                    RegistryRoot::CurrentUser,
                    "Software\\Studio\\Game",
                    "Language",
                    RegistryValue::String { text: "ru".into() }
                ),
                set(
                    RegistryRoot::CurrentUser,
                    "Software\\Studio\\Game",
                    "",
                    RegistryValue::String {
                        text: "default".into()
                    }
                ),
            ]
        );
    }

    #[test]
    fn regedit4_files_in_one_byte_text_are_read() {
        let file =
            b"REGEDIT4\n\n[HKEY_LOCAL_MACHINE\\SOFTWARE\\Game]\n\"Path\"=\"C:\\\\Games\\\\Game\"\n";
        let import = parse_reg(file).unwrap();
        assert_eq!(
            import.entries,
            vec![set(
                RegistryRoot::LocalMachine,
                "SOFTWARE\\Game",
                "Path",
                RegistryValue::String {
                    text: "C:\\Games\\Game".into()
                }
            )]
        );
    }

    #[test]
    fn windows_1251_text_is_read_when_it_is_not_utf8() {
        let mut file = b"REGEDIT4\r\n[HKEY_CURRENT_USER\\Software\\Game]\r\n\"Name\"=\"".to_vec();
        file.extend([0xC8, 0xE3, 0xF0, 0xEE, 0xEA]);
        file.extend(b"\"\r\n");
        let import = parse_reg(&file).unwrap();
        assert_eq!(
            import.entries[0].change,
            RegistryChange::Set {
                value: RegistryValue::String {
                    text: "Игрок".into()
                }
            }
        );
    }

    #[test]
    fn utf8_with_a_byte_order_mark_is_read() {
        let mut file = vec![0xEF, 0xBB, 0xBF];
        file.extend(
            "Windows Registry Editor Version 5.00\n[HKCU\\Software\\Игра]\n\"Имя\"=\"Да\"\n"
                .as_bytes(),
        );
        let import = parse_reg(&file).unwrap();
        assert_eq!(import.entries[0].path, "Software\\Игра");
        assert_eq!(import.entries[0].name, "Имя");
    }

    #[test]
    fn a_file_without_the_header_is_not_a_reg_file() {
        assert_eq!(
            parse_reg(b"[HKEY_CURRENT_USER\\x]\n"),
            Err(RegParseError::NotRegFile)
        );
        assert_eq!(parse_reg(b""), Err(RegParseError::NotRegFile));
    }

    #[test]
    fn hex_types_follow_the_file_encoding() {
        let file = wide_file(
            "Windows Registry Editor Version 5.00\r\n[HKEY_CURRENT_USER\\K]\r\n\
             \"E\"=hex(2):25,00,41,00,25,00,00,00\r\n\
             \"M\"=hex(7):61,00,00,00,62,00,00,00,00,00\r\n\
             \"Q\"=hex(b):01,00,00,00,00,00,00,00\r\n\
             \"B\"=hex:de,ad\r\n\
             \"N\"=hex(0):\r\n",
        );
        let values: Vec<RegistryValue> = parse_reg(&file)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| match entry.change {
                RegistryChange::Set { value } => value,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            values,
            vec![
                RegistryValue::ExpandString { text: "%A%".into() },
                RegistryValue::MultiString {
                    items: vec!["a".into(), "b".into()]
                },
                RegistryValue::Qword { number: Qword(1) },
                RegistryValue::Binary {
                    bytes: vec![0xDE, 0xAD]
                },
                RegistryValue::Other {
                    kind: 0,
                    bytes: vec![]
                },
            ]
        );
        let narrow = b"REGEDIT4\n[HKEY_CURRENT_USER\\K]\n\"E\"=hex(2):25,41,25,00\n";
        assert_eq!(
            parse_reg(narrow).unwrap().entries[0].change,
            RegistryChange::Set {
                value: RegistryValue::ExpandString { text: "%A%".into() }
            }
        );
    }

    #[test]
    fn hex_data_continues_on_the_next_lines() {
        let file = b"REGEDIT4\n[HKEY_CURRENT_USER\\K]\n\"B\"=hex:01,02,\\\n  03,\\ ; comment\n\n  04\n\"C\"=dword:1\n";
        let entries = parse_reg(file).unwrap().entries;
        assert_eq!(
            entries[0].change,
            RegistryChange::Set {
                value: RegistryValue::Binary {
                    bytes: vec![1, 2, 3, 4]
                }
            }
        );
        assert_eq!(entries[1].name, "C");
    }

    #[test]
    fn deletions_of_values_and_keys_are_kept() {
        let file = b"REGEDIT4\n[-HKEY_CURRENT_USER\\Software\\Old]\n[HKEY_CURRENT_USER\\Software\\Game]\n\"Gone\"=-\n@=-\n[-HKEY_CURRENT_USER]\n";
        let entries = parse_reg(file).unwrap().entries;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].change, RegistryChange::DeleteKey);
        assert_eq!(entries[0].path, "Software\\Old");
        assert_eq!(entries[1].change, RegistryChange::DeleteValue);
        assert_eq!(entries[2].name, "");
    }

    #[test]
    fn classes_root_and_wow6432node_map_to_the_single_view() {
        let file = b"REGEDIT4\n[HKEY_CLASSES_ROOT\\.foo]\n@=\"Foo\"\n[HKEY_LOCAL_MACHINE\\SOFTWARE\\Wow6432Node\\Studio\\Game]\n\"V\"=dword:1\n";
        let entries = parse_reg(file).unwrap().entries;
        assert_eq!(entries[0].root, RegistryRoot::LocalMachine);
        assert_eq!(entries[0].path, "Software\\Classes\\.foo");
        assert_eq!(entries[1].path, "SOFTWARE\\Studio\\Game");
    }

    #[test]
    fn unknown_roots_and_bad_lines_are_reported_not_imported() {
        let file = b"REGEDIT4\n\"Early\"=\"x\"\n[HKEY_USERS\\.DEFAULT\\K]\n\"A\"=\"1\"\n[HKEY_CURRENT_USER\\K]\n\"B\"=dword:zz\n\"C\"=\"open\n; a comment\n\"D\"=dword:2 ; two\n";
        let import = parse_reg(file).unwrap();
        let reasons: Vec<(u32, RegSkipReason)> =
            import.skipped.iter().map(|s| (s.line, s.reason)).collect();
        assert_eq!(
            reasons,
            vec![
                (2, RegSkipReason::NoKey),
                (3, RegSkipReason::UnknownRoot),
                (6, RegSkipReason::BadValue),
                (7, RegSkipReason::BadValue),
            ]
        );
        assert_eq!(import.entries.len(), 1);
        assert_eq!(import.entries[0].name, "D");
    }

    #[test]
    fn escapes_in_names_and_strings_are_read() {
        let file = b"REGEDIT4\n[HKEY_CURRENT_USER\\K]\n\"a \\\"b\\\"\"=\"line\\nnext \\\\ end\"\n";
        let entry = &parse_reg(file).unwrap().entries[0];
        assert_eq!(entry.name, "a \"b\"");
        assert_eq!(
            entry.change,
            RegistryChange::Set {
                value: RegistryValue::String {
                    text: "line\nnext \\ end".into()
                }
            }
        );
    }

    #[test]
    fn exported_files_read_back_the_same() {
        let entries = vec![
            set(
                RegistryRoot::CurrentUser,
                "Software\\Game",
                "Text",
                RegistryValue::String {
                    text: "a \"q\" \\ б".into(),
                },
            ),
            set(
                RegistryRoot::CurrentUser,
                "Software\\Game",
                "",
                RegistryValue::Dword { number: 7 },
            ),
            set(
                RegistryRoot::CurrentUser,
                "Software\\Game",
                "Big",
                RegistryValue::Qword {
                    number: Qword(u64::MAX),
                },
            ),
            set(
                RegistryRoot::LocalMachine,
                "Software\\Game",
                "List",
                RegistryValue::MultiString {
                    items: vec!["x".into(), "y".into()],
                },
            ),
            set(
                RegistryRoot::LocalMachine,
                "Software\\Game",
                "Expand",
                RegistryValue::ExpandString {
                    text: "%TEMP%".into(),
                },
            ),
            RegistryEntry {
                root: RegistryRoot::LocalMachine,
                path: "Software\\Game".into(),
                name: "Old".into(),
                change: RegistryChange::DeleteValue,
            },
            RegistryEntry {
                root: RegistryRoot::CurrentUser,
                path: "Software\\Gone".into(),
                name: String::new(),
                change: RegistryChange::DeleteKey,
            },
        ];
        let bytes = export_reg(&entries);
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        assert_eq!(parse_reg(&bytes).unwrap().entries, entries);
    }

    fn row() -> RegistryMenuRow {
        RegistryMenuRow {
            name: "Resolution".into(),
            hint: String::new(),
            fields: vec![
                RegistryField {
                    root: RegistryRoot::CurrentUser,
                    path: "Software\\Game".into(),
                    name: "Width".into(),
                    kind: RegistryKind::Dword,
                },
                RegistryField {
                    root: RegistryRoot::CurrentUser,
                    path: "Software\\Game".into(),
                    name: "Height".into(),
                    kind: RegistryKind::Dword,
                },
            ],
            options: vec![
                RegistryOption {
                    label: "1280×720".into(),
                    values: vec!["1280".into(), "0x2d0".into()],
                },
                RegistryOption {
                    label: "800×600".into(),
                    values: vec!["800".into(), "600".into()],
                },
            ],
        }
    }

    #[test]
    fn the_registry_file_carries_entries_and_menu_rows() {
        let settings = RegistrySettings {
            mode: RegistryWriteMode::EveryStart,
            entries: vec![
                set(
                    RegistryRoot::CurrentUser,
                    "\\Software\\Game\\",
                    "Language",
                    RegistryValue::String { text: "ru".into() },
                ),
                RegistryEntry {
                    root: RegistryRoot::LocalMachine,
                    path: "Software\\Game".into(),
                    name: "Old".into(),
                    change: RegistryChange::DeleteValue,
                },
                RegistryEntry {
                    root: RegistryRoot::LocalMachine,
                    path: "Software\\Gone".into(),
                    name: String::new(),
                    change: RegistryChange::DeleteKey,
                },
            ],
            menu: vec![row()],
        };
        assert_eq!(settings.validate(), Ok(()));
        let text = render_registry_file(&settings).unwrap();
        assert_eq!(
            text,
            "carafe-registry\t1\nmode\talways\n\
             set\tHKCU\tSoftware\\Game\tLanguage\t1\t720075000000\n\
             delete\tHKLM\tSoftware\\Game\tOld\n\
             deletekey\tHKLM\tSoftware\\Gone\n\
             row\tResolution\t\n\
             field\tHKCU\tSoftware\\Game\tWidth\t4\n\
             field\tHKCU\tSoftware\\Game\tHeight\t4\n\
             option\t1280×720\t00050000\td0020000\n\
             option\t800×600\t20030000\t58020000\n"
        );
    }

    #[test]
    fn nothing_to_write_gives_no_registry_file() {
        assert_eq!(render_registry_file(&RegistrySettings::default()), None);
    }

    #[test]
    fn menu_rows_need_names_fields_and_fitting_choices() {
        let mut settings = RegistrySettings {
            menu: vec![row()],
            ..RegistrySettings::default()
        };
        settings.menu[0].options[1].values.pop();
        assert!(matches!(
            settings.validate(),
            Err(RegistryError::OptionCount(_))
        ));
        settings.menu[0] = row();
        settings.menu[0].options[0].values[0] = "4294967296".into();
        assert!(matches!(
            settings.validate(),
            Err(RegistryError::OptionValue(_))
        ));
        settings.menu[0] = row();
        settings.menu[0].name = " ".into();
        assert!(matches!(settings.validate(), Err(RegistryError::Row(_))));
        settings.menu[0] = row();
        settings.menu[0].options.clear();
        assert!(matches!(settings.validate(), Err(RegistryError::Row(_))));
        settings.menu[0] = row();
        settings.menu[0].options[0].label = "a\tb".into();
        assert!(matches!(settings.validate(), Err(RegistryError::Text(_))));
    }

    #[test]
    fn entries_need_clean_paths_and_names() {
        let mut settings = RegistrySettings {
            entries: vec![set(
                RegistryRoot::CurrentUser,
                "Software\\\\Game",
                "x",
                RegistryValue::Dword { number: 1 },
            )],
            ..RegistrySettings::default()
        };
        assert!(matches!(settings.validate(), Err(RegistryError::Path(_))));
        settings.entries[0].path = "Software\\Game".into();
        settings.entries[0].name = "a\nb".into();
        assert!(matches!(settings.validate(), Err(RegistryError::Name(_))));
        settings.entries[0] = RegistryEntry {
            root: RegistryRoot::CurrentUser,
            path: "\\".into(),
            name: String::new(),
            change: RegistryChange::DeleteKey,
        };
        assert_eq!(settings.validate(), Err(RegistryError::DeleteRoot));
    }

    #[test]
    fn qwords_travel_as_text() {
        let value = RegistryValue::Qword {
            number: Qword(u64::MAX),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"type":"qword","number":"18446744073709551615"}"#);
        assert_eq!(serde_json::from_str::<RegistryValue>(&json).unwrap(), value);
        assert_eq!(
            serde_json::from_str::<RegistryValue>(r#"{"type":"qword","number":5}"#).unwrap(),
            RegistryValue::Qword { number: Qword(5) }
        );
    }

    #[test]
    fn settings_written_without_a_registry_read_as_empty() {
        let settings: RegistrySettings = serde_json::from_str("{}").unwrap();
        assert!(settings.is_empty());
        assert_eq!(settings.mode, RegistryWriteMode::Once);
    }
}
