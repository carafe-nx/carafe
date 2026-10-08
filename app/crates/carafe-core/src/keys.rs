//! Checking `prod.keys`: whether it contains the keys hacBrewPack needs.

use std::collections::HashSet;

use crate::ports::KeysReport;

const KEK_SOURCES: [&str; 3] = [
    "master_key_00",
    "aes_kek_generation_source",
    "aes_key_generation_source",
];
const HEADER_KEY_SOURCES: [&str; 2] = ["header_kek_source", "header_key_source"];
const KEY_AREA_KEY_SOURCES: [&str; 1] = ["key_area_key_application_source"];

/// Returns which of the required keys are present in the `prod.keys` text or derivable from source keys.
///
/// File lines are `name = value` or `name, value`; names are compared case-insensitively,
/// other lines are ignored. `header_key` is derived from `master_key_00`, the KEK sources and
/// `header_kek_source` with `header_key_source`; `key_area_key_application_00` from `master_key_00`,
/// the KEK sources and `key_area_key_application_source`. Key values are not read.
#[must_use]
pub fn inspect_keys(text: &str) -> KeysReport {
    let names: HashSet<String> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter_map(key_name)
        .collect();
    let has_all = |required: &[&str]| required.iter().all(|name| names.contains(*name));

    KeysReport {
        header_key: names.contains("header_key")
            || (has_all(&KEK_SOURCES) && has_all(&HEADER_KEY_SOURCES)),
        key_area_key: names.contains("key_area_key_application_00")
            || (has_all(&KEK_SOURCES) && has_all(&KEY_AREA_KEY_SOURCES)),
    }
}

fn key_name(line: &str) -> Option<String> {
    let (name, _) = line.split_once(['=', ','])?;
    let name = name.trim().to_ascii_lowercase();
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    valid.then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_both_keys() {
        let report = inspect_keys("header_key = 00\nkey_area_key_application_00 = 11\n");
        assert!(report.header_key && report.key_area_key);
    }

    #[test]
    fn missing_keys_are_reported() {
        let report = inspect_keys("titlekek_00 = 00\n");
        assert!(!report.header_key && !report.key_area_key);
    }

    #[test]
    fn keys_can_be_derived_from_sources() {
        let text = "master_key_00 = 0\naes_kek_generation_source = 0\naes_key_generation_source = 0\n\
                    header_kek_source = 0\nheader_key_source = 0\nkey_area_key_application_source = 0\n";
        let report = inspect_keys(text);
        assert!(report.header_key && report.key_area_key);
    }

    #[test]
    fn a_missing_source_prevents_derivation() {
        let text = "master_key_00 = 0\naes_kek_generation_source = 0\nheader_kek_source = 0\nheader_key_source = 0\n";
        assert!(!inspect_keys(text).header_key);
    }

    #[test]
    fn names_ignore_case_bom_and_line_endings() {
        let report =
            inspect_keys("\u{feff}HEADER_KEY = 00\r\nKey_Area_Key_Application_00 = 11\r\n");
        assert!(report.header_key && report.key_area_key);
    }

    #[test]
    fn comma_separated_lines_count() {
        let report = inspect_keys("header_key,00\nkey_area_key_application_00,11\n");
        assert!(report.header_key && report.key_area_key);
    }

    #[test]
    fn text_without_key_lines_has_no_keys() {
        assert_eq!(
            inspect_keys("not a keys file\n\0\u{1}"),
            KeysReport {
                header_key: false,
                key_area_key: false
            }
        );
    }
}
