//! GOG's install scripts, `goggame-<id>.script` in the game folder: the registry values GOG's installer writes after
//! it copies the game, which a game folder copied without the installer lacks.

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::registry::{InstalledEntry, Qword, RegistryValue, split_root};

const SCRIPT_PREFIX: &str = "goggame-";
const SCRIPT_SUFFIX: &str = ".script";
const SET_REGISTRY: &str = "setregistry";
const USER_DIR: &str = "C:\\users\\steamuser";

/// The largest install script that is read; GOG's scripts take a few kilobytes.
pub const MAX_SCRIPT_BYTES: u64 = 1 << 20;

/// An install script as GOG writes it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallScript {
    /// The game's or the add-on's GOG product ID.
    #[serde(default)]
    product_id: Option<ScriptText>,
    /// The steps, in the order the installer runs them.
    #[serde(default)]
    actions: Vec<ScriptAction>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ScriptAction {
    #[serde(default)]
    install: Option<ScriptStep>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ScriptStep {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    arguments: ScriptArguments,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptArguments {
    #[serde(default)]
    root: Option<ScriptText>,
    #[serde(default)]
    subkey: Option<ScriptText>,
    #[serde(default)]
    value_name: Option<ScriptText>,
    #[serde(default)]
    value_data: Option<ScriptText>,
    #[serde(default)]
    value_type: Option<ScriptText>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ScriptText {
    Text(String),
    Number(u64),
    Other(IgnoredAny),
}

impl ScriptText {
    fn text(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::Number(number) => Some(number.to_string()),
            Self::Other(_) => None,
        }
    }
}

/// Returns whether a file in the game folder's root is a GOG install script.
#[must_use]
pub fn is_install_script(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();
    name.len() > SCRIPT_PREFIX.len() + SCRIPT_SUFFIX.len()
        && name.starts_with(SCRIPT_PREFIX)
        && name.ends_with(SCRIPT_SUFFIX)
}

/// Returns the registry values and keys the script's `setRegistry` steps write.
///
/// `{app}` becomes `game_dir`, the game folder as the game sees it; `{productId}` the script's product ID; `{userDocs}`,
/// `{userAppData}` and `{localAppData}` the console user's folders. Placeholders are matched in any case.
///
/// # Returns
///
/// The entries in the script's order. A step with an unknown placeholder, root or value type, or with data that does
/// not fit its type, is left out.
#[must_use]
pub fn installed_entries(script: &InstallScript, game_dir: &str) -> Vec<InstalledEntry> {
    let product_id = script
        .product_id
        .as_ref()
        .and_then(ScriptText::text)
        .unwrap_or_default();
    script
        .actions
        .iter()
        .filter_map(|action| action.install.as_ref())
        .filter(|step| {
            step.action
                .as_deref()
                .is_some_and(|action| action.eq_ignore_ascii_case(SET_REGISTRY))
        })
        .filter_map(|step| entry(&step.arguments, game_dir, &product_id))
        .collect()
}

fn entry(arguments: &ScriptArguments, game_dir: &str, product_id: &str) -> Option<InstalledEntry> {
    let fill = |text: &ScriptText| {
        text.text()
            .and_then(|text| expand(&text, game_dir, product_id))
    };
    let root = fill(arguments.root.as_ref()?)?;
    let subkey = fill(arguments.subkey.as_ref()?)?;
    let (root, path) = split_root(&format!("{root}\\{subkey}"))?;
    if path.is_empty() {
        return None;
    }
    let Some(name) = &arguments.value_name else {
        return Some(InstalledEntry::Key { root, path });
    };
    let name = fill(name)?;
    let data = match &arguments.value_data {
        Some(data) => fill(data)?,
        None => String::new(),
    };
    let kind = arguments
        .value_type
        .as_ref()
        .and_then(ScriptText::text)
        .unwrap_or_else(|| "string".to_owned());
    let value = value(&kind, &data)?;
    Some(InstalledEntry::Value {
        root,
        path,
        name,
        value,
    })
}

fn value(kind: &str, data: &str) -> Option<RegistryValue> {
    let data = data.trim();
    Some(match kind.to_ascii_lowercase().as_str() {
        "string" | "sz" | "reg_sz" => RegistryValue::String {
            text: data.to_owned(),
        },
        "expandstring" | "expand_sz" | "reg_expand_sz" => RegistryValue::ExpandString {
            text: data.to_owned(),
        },
        "dword" | "reg_dword" => RegistryValue::Dword {
            number: u32::try_from(number(data)?).ok()?,
        },
        "qword" | "reg_qword" => RegistryValue::Qword {
            number: Qword(number(data)?),
        },
        "binary" | "reg_binary" => RegistryValue::Binary {
            bytes: binary(data)?,
        },
        _ => return None,
    })
}

fn number(text: &str) -> Option<u64> {
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None if text.is_empty() => Some(0),
        None => text.parse().ok(),
    }
}

fn binary(text: &str) -> Option<Vec<u8>> {
    let digits: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&digits[at..at + 2], 16).ok())
        .collect()
}

fn expand(text: &str, game_dir: &str, product_id: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}')? + open;
        let name = rest[open + 1..close].to_ascii_lowercase();
        let user = |dir: &str| format!("{USER_DIR}\\{dir}");
        let value = match name.as_str() {
            "app" => game_dir.to_owned(),
            "productid" => product_id.to_owned(),
            "userdocs" => user("Documents"),
            "userappdata" => user("AppData\\Roaming"),
            "localappdata" => user("AppData\\Local"),
            _ => return None,
        };
        out.push_str(&value);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    Some(out.replace('/', "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryRoot;

    const GAME: &str = "C:\\Games\\SPORE";

    fn script(json: &str) -> InstallScript {
        serde_json::from_str(json).unwrap()
    }

    fn step(arguments: &str) -> String {
        format!(
            r#"{{"actions":[{{"install":{{"action":"setRegistry","arguments":{arguments}}},"name":"k"}}],
                "productId":"1948823323"}}"#
        )
    }

    fn only(json: &str) -> Vec<InstalledEntry> {
        installed_entries(&script(&step(json)), GAME)
    }

    #[test]
    fn install_scripts_are_found_by_name() {
        assert!(is_install_script("goggame-1948823323.script"));
        assert!(is_install_script("GOGGAME-1.SCRIPT"));
        assert!(!is_install_script("goggame-1948823323.info"));
        assert!(!is_install_script("goggame-.script"));
        assert!(!is_install_script("game.script"));
    }

    #[test]
    fn set_registry_steps_become_entries_with_the_game_folder() {
        let entries = installed_entries(
            &script(
                r#"{"actions":[
                    {"install":{"action":"savePath","arguments":{"savePath":"{userdocs}/My Spore Creations"}}},
                    {"install":{"action":"setRegistry","arguments":{"root":"HKEY_LOCAL_MACHINE",
                        "subkey":"SOFTWARE\\electronic arts\\spore","valueData":"{app}\\Data",
                        "valueName":"datadir","valueType":"string"}}},
                    {"install":{"action":"setRegistry","arguments":{"conditions":["onlyOnce"],
                        "root":"HKEY_LOCAL_MACHINE","subkey":"SOFTWARE\\electronic arts\\spore_EP1",
                        "valueData":"128448390","valueName":"PackID","valueType":"dword"}}},
                    {"install":{"action":"setRegistry","arguments":{"root":"HKEY_LOCAL_MACHINE",
                        "subkey":"SOFTWARE\\electronic arts\\ea games\\spore(tm)"}}}
                ],"productId":"1948823323"}"#,
            ),
            GAME,
        );
        assert_eq!(
            entries,
            vec![
                InstalledEntry::Value {
                    root: RegistryRoot::LocalMachine,
                    path: "SOFTWARE\\electronic arts\\spore".into(),
                    name: "datadir".into(),
                    value: RegistryValue::String {
                        text: "C:\\Games\\SPORE\\Data".into()
                    },
                },
                InstalledEntry::Value {
                    root: RegistryRoot::LocalMachine,
                    path: "SOFTWARE\\electronic arts\\spore_EP1".into(),
                    name: "PackID".into(),
                    value: RegistryValue::Dword {
                        number: 128_448_390
                    },
                },
                InstalledEntry::Key {
                    root: RegistryRoot::LocalMachine,
                    path: "SOFTWARE\\electronic arts\\ea games\\spore(tm)".into(),
                },
            ]
        );
    }

    #[test]
    fn placeholders_take_the_console_folders_and_the_product() {
        let entries = only(
            r#"{"root":"HKCU","subkey":"Software\\GOG.com\\Games\\{productId}",
                "valueName":"saves","valueData":"{userDocs}/Game","valueType":"string"}"#,
        );
        assert_eq!(
            entries,
            vec![InstalledEntry::Value {
                root: RegistryRoot::CurrentUser,
                path: "Software\\GOG.com\\Games\\1948823323".into(),
                name: "saves".into(),
                value: RegistryValue::String {
                    text: "C:\\users\\steamuser\\Documents\\Game".into()
                },
            }]
        );
    }

    #[test]
    fn wow64_keys_land_in_the_single_view() {
        let entries = only(
            r#"{"root":"HKEY_LOCAL_MACHINE","subkey":"SOFTWARE\\Wow6432Node\\Studio\\Game",
                "valueName":"Path","valueData":"{app}"}"#,
        );
        assert!(matches!(
            &entries[..],
            [InstalledEntry::Value { path, value: RegistryValue::String { text }, .. }]
                if path == "SOFTWARE\\Studio\\Game" && text == GAME
        ));
    }

    #[test]
    fn numbers_and_bytes_fit_their_types() {
        let numbers = only(
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"0x10","valueType":"dword"}"#,
        );
        assert!(matches!(
            &numbers[..],
            [InstalledEntry::Value {
                value: RegistryValue::Dword { number: 16 },
                ..
            }]
        ));
        let bytes = only(
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"b","valueData":"01 ff,02","valueType":"binary"}"#,
        );
        assert!(matches!(
            &bytes[..],
            [InstalledEntry::Value { value: RegistryValue::Binary { bytes }, .. }] if bytes == &[1, 0xff, 2]
        ));
    }

    #[test]
    fn steps_that_cannot_be_written_are_left_out() {
        for arguments in [
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"x","valueType":"dword"}"#,
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"4294967296","valueType":"dword"}"#,
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"1","valueType":"multiString"}"#,
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"{supportDir}\\x"}"#,
            r#"{"root":"HKEY_USERS","subkey":"Software\\G","valueName":"n","valueData":"1"}"#,
            r#"{"root":"HKLM","subkey":"","valueName":"n","valueData":"1"}"#,
            r#"{"subkey":"Software\\G","valueName":"n","valueData":"1"}"#,
            r#"{"root":"HKLM","subkey":"Software\\G","valueName":"n","valueData":"{app"}"#,
        ] {
            assert_eq!(only(arguments), vec![], "{arguments}");
        }
    }

    #[test]
    fn unexpected_shapes_do_not_spoil_the_script() {
        let entries = installed_entries(
            &script(
                r#"{"actions":[{"install":{"action":"setRegistry","arguments":{"root":"HKLM",
                    "subkey":"Software\\G","valueName":"n","valueData":[1,2]}}},
                    {"name":"no install"},
                    {"install":{"action":"setRegistry","arguments":{"root":"HKLM",
                    "subkey":"Software\\G","valueName":"m","valueData":7,"valueType":"dword"}}}]}"#,
            ),
            GAME,
        );
        assert!(matches!(
            &entries[..],
            [InstalledEntry::Value { name, value: RegistryValue::Dword { number: 7 }, .. }] if name == "m"
        ));
    }
}
