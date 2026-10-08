//! What Carafe puts into a game's NSP on top of the runtime.

use thiserror::Error;

use crate::autorun_files::{AutorunFiles, executable_stem};
use crate::nacp;
use crate::record::BuildRecord;

/// Games folder in RomFS; under it is the game folder with its files.
pub const GAMES_DIR: &str = "switch/wine/drive_c/Games";

/// Name of the build record inside the NSP.
pub const RECORD_FILE: &str = "carafe.json";

/// Name of the icon copy inside the NSP: the library reads it without decrypting the NCA.
pub const ICON_FILE: &str = "icon.jpg";

/// Path of the game icon in RomFS from its root, separated by `/`: the loading screen on the console shows it.
pub const ROMFS_ICON_FILE: &str = "carafe/icon.jpg";

const RUNTIME_NRO: &str = "sdmc:/switch/wine/wine-nx-runtime.nro";
const ARGV_FILE: &str = "carafe/argv";
const TITLE_FILE: &str = "carafe/title";

/// Text file that Carafe creates in RomFS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// Path from the RomFS root, separated by `/`.
    pub path: String,
    /// Contents.
    pub contents: String,
}

/// Everything a game's NSP gets beyond the runtime, except the game's own files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamePackage {
    /// Folder the game folder is copied into: a path from the RomFS root, separated by `/`.
    pub game_dir: String,
    /// RomFS files that are written after the game is copied and replace game files with the same names.
    pub romfs_files: Vec<GeneratedFile>,
    /// `control.nacp`.
    pub nacp: Vec<u8>,
    /// Icon file names in control, one for each supported language.
    pub icon_files: Vec<String>,
}

/// NSP composition error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackageError {
    /// No name can be taken from the game folder path.
    #[error("the game folder has no name: {0}")]
    NoFolderName(String),
    /// The folder name contains a quote or a control character.
    #[error("the game folder name is not usable on a command line: {0}")]
    BadFolderName(String),
    /// The path to the `.exe` is not relative or leaves the game folder.
    #[error("the executable must be inside the game folder: {0}")]
    ExecutableOutside(String),
    /// A launch argument contains a line break.
    #[error("a launch argument spans several lines: {0}")]
    MultilineArgument(String),
}

/// Composes the contents of a game's NSP from the build record.
///
/// The game folder in RomFS is named like the folder on the PC. The runtime gets the path to the `.exe` in
/// `carafe/argv`, the title for the loading screen in `carafe/title`; next to the `.exe` go `<name>.keys.txt`,
/// `<name>.wine-nx.txt` and, if there are arguments, `<name>.args.txt`.
///
/// # Errors
///
/// [`PackageError`] if the folder name, the path to the `.exe` or the arguments cannot be passed to the runtime.
pub fn package(record: &BuildRecord) -> Result<GamePackage, PackageError> {
    let source = &record.source;
    let folder_name = folder_name(&source.folder)?;
    let executable = executable_path(&source.executable)?;
    let game_dir = format!("{GAMES_DIR}/{folder_name}");
    let exe_dir = match executable.rsplit_once('/') {
        Some((dir, _)) => format!("{game_dir}/{dir}"),
        None => game_dir.clone(),
    };
    let target = format!("sdmc:/{game_dir}/{executable}");
    let settings = AutorunFiles::render(&executable, &record.settings);
    let mut romfs_files = vec![
        GeneratedFile {
            path: ARGV_FILE.to_owned(),
            contents: format!("{RUNTIME_NRO} \"{target}\""),
        },
        GeneratedFile {
            path: TITLE_FILE.to_owned(),
            contents: record.metadata.title.clone(),
        },
        GeneratedFile {
            path: format!("{exe_dir}/{}", settings.keys_name),
            contents: settings.keys,
        },
        GeneratedFile {
            path: format!("{exe_dir}/{}", settings.program_name),
            contents: settings.program,
        },
    ];
    if !source.arguments.is_empty() {
        let stem = executable_stem(&executable);
        romfs_files.push(GeneratedFile {
            path: format!("{exe_dir}/{stem}.args.txt"),
            contents: format!("{}\n", command_line(&source.arguments)?),
        });
    }
    let nacp = nacp::render(&record.metadata, &record.display_version(), record.title_id);
    let icon_files = nacp::supported_languages(&record.metadata)
        .into_iter()
        .map(|language| format!("icon_{}.dat", language.name()))
        .collect();
    Ok(GamePackage {
        game_dir,
        romfs_files,
        nacp,
        icon_files,
    })
}

fn folder_name(folder: &str) -> Result<&str, PackageError> {
    let name = folder
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .filter(|name| *name != "." && *name != ".." && !name.ends_with(':'))
        .ok_or_else(|| PackageError::NoFolderName(folder.to_owned()))?;
    if name.chars().any(|c| c == '"' || c.is_control()) {
        return Err(PackageError::BadFolderName(name.to_owned()));
    }
    Ok(name)
}

fn executable_path(executable: &str) -> Result<String, PackageError> {
    let outside = || PackageError::ExecutableOutside(executable.to_owned());
    let parts: Vec<&str> = executable.split(['/', '\\']).collect();
    let valid = parts.iter().all(|part| {
        !part.is_empty()
            && *part != "."
            && *part != ".."
            && !part.contains(':')
            && !part.chars().any(|c| c == '"' || c.is_control())
    });
    if !valid {
        return Err(outside());
    }
    Ok(parts.join("/"))
}

fn command_line(arguments: &[String]) -> Result<String, PackageError> {
    if let Some(bad) = arguments.iter().find(|a| a.contains(['\n', '\r'])) {
        return Err(PackageError::MultilineArgument(bad.clone()));
    }
    let quoted: Vec<String> = arguments.iter().map(|a| quote(a)).collect();
    Ok(quoted.join(" "))
}

fn quote(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '"']) {
        return argument.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in argument.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;
    use crate::record::{Arch, FORMAT_VERSION, GameSource};
    use crate::settings::AutorunSettings;
    use crate::title_id::TitleId;

    fn record(folder: &str, executable: &str, arguments: &[&str]) -> BuildRecord {
        BuildRecord {
            format_version: FORMAT_VERSION,
            title_id: TitleId::parse("056694dd13640000").unwrap(),
            build_number: 1,
            runtime_version: "0.1.0".to_owned(),
            source: GameSource {
                folder: folder.to_owned(),
                executable: executable.to_owned(),
                arch: Arch::X86,
                arguments: arguments.iter().map(|&a| a.to_owned()).collect(),
            },
            metadata: Metadata::new("OpenTTD x86"),
            settings: AutorunSettings::default(),
        }
    }

    fn file<'a>(package: &'a GamePackage, path: &str) -> Option<&'a str> {
        package
            .romfs_files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.contents.as_str())
    }

    #[test]
    fn argv_points_at_the_executable_like_the_scripts() {
        let record = record("D:\\Game\\openttd-15.3-windows-win32\\", "openttd.exe", &[]);
        let package = package(&record).unwrap();
        assert_eq!(
            package.game_dir,
            "switch/wine/drive_c/Games/openttd-15.3-windows-win32"
        );
        assert_eq!(
            file(&package, "carafe/argv"),
            Some(
                "sdmc:/switch/wine/wine-nx-runtime.nro \
                 \"sdmc:/switch/wine/drive_c/Games/openttd-15.3-windows-win32/openttd.exe\""
            )
        );
        assert_eq!(file(&package, "carafe/title"), Some("OpenTTD x86"));
        let games = "switch/wine/drive_c/Games/openttd-15.3-windows-win32";
        assert!(file(&package, &format!("{games}/openttd.keys.txt")).is_some());
        assert!(file(&package, &format!("{games}/openttd.wine-nx.txt")).is_some());
        assert!(file(&package, &format!("{games}/openttd.args.txt")).is_none());
        assert_eq!(package.icon_files, vec!["icon_AmericanEnglish.dat"]);
        assert_eq!(package.nacp.len(), nacp::NACP_SIZE);
    }

    #[test]
    fn settings_files_sit_next_to_a_nested_executable() {
        let record = record(
            "/home/me/Games/Fallout 2",
            "bin\\Fallout2.exe",
            &["-v", "win32"],
        );
        let package = package(&record).unwrap();
        let bin = "switch/wine/drive_c/Games/Fallout 2/bin";
        assert!(file(&package, &format!("{bin}/Fallout2.keys.txt")).is_some());
        assert_eq!(
            file(&package, &format!("{bin}/Fallout2.args.txt")),
            Some("-v win32\n")
        );
        assert!(
            file(&package, "carafe/argv")
                .unwrap()
                .ends_with("/Fallout 2/bin/Fallout2.exe\"")
        );
    }

    #[test]
    fn arguments_are_quoted_for_windows() {
        let arguments = [
            "plain".to_owned(),
            "two words".to_owned(),
            String::new(),
            "say \"hi\"".to_owned(),
            "C:\\dir with space\\".to_owned(),
        ];
        assert_eq!(
            command_line(&arguments).unwrap(),
            r#"plain "two words" "" "say \"hi\"" "C:\dir with space\\""#
        );
    }

    #[test]
    fn executable_must_stay_inside_the_folder() {
        for bad in ["..\\game.exe", "C:\\game.exe", "/game.exe", "bin//game.exe"] {
            assert_eq!(
                package(&record("D:\\Game\\x", bad, &[])),
                Err(PackageError::ExecutableOutside(bad.to_owned()))
            );
        }
    }

    #[test]
    fn folder_needs_a_usable_name() {
        assert_eq!(
            package(&record("D:\\", "game.exe", &[])),
            Err(PackageError::NoFolderName("D:\\".to_owned()))
        );
        assert_eq!(
            package(&record("/tmp/a\"b", "game.exe", &[])),
            Err(PackageError::BadFolderName("a\"b".to_owned()))
        );
    }

    #[test]
    fn multiline_arguments_are_rejected() {
        assert_eq!(
            package(&record("D:\\Game\\x", "game.exe", &["a\nb"])),
            Err(PackageError::MultilineArgument("a\nb".to_owned()))
        );
    }
}
