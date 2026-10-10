//! NSP building with hacBrewPack: the adapter of the [`Packer`] port.
//!
//! The runtime is unpacked from the archive into the library's temporary folder `.carafe/build/<Title ID>`,
//! each file is checked against its SHA-256; on top of it go the game and the files from [`carafe_core::package`].
//! hacBrewPack encrypts the NCAs, and the NSP with `carafe.json` next to the NCAs is assembled right here and moved
//! into the library only as a whole.

pub mod archive;
mod copy;
pub mod service;
mod tool;
pub mod verify;

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use carafe_core::BuildRecord;
use carafe_core::icon::IconJpeg;
use carafe_core::npdm::AddressSpace;
use carafe_core::package::{GamePackage, ICON_FILE, RECORD_FILE, ROMFS_ICON_FILE, package};
use carafe_core::pfs0::{self, Pfs0Entry};
use carafe_core::ports::{AdapterError, BuildProgress, BuildStage, PackTarget, Packer, StoredGame};
use carafe_core::{disk_space, npdm, path_limit, pe};

use crate::archive::RuntimeArchive;
use crate::copy::Tree;

const RUNTIME_MAIN: &str = "exefs/main";
const RUNTIME_NPDM: &str = "exefs/main.npdm";
const RUNTIME_ICON: &str = "control/icon_AmericanEnglish.dat";
const RUNTIME_LOGO: &str = "logo";
const RUNTIME_ROMFS: &str = "romfs";
const LOGO_FILES: [&str; 2] = ["NintendoLogo.png", "StartupMovie.gif"];
const ROMFS_WRITE_PREFIX: &str = "Writing ";
const OPEN_FAILED_PREFIX: &str = "Failed to open ";

/// The longest path, in UTF-8 bytes, that hacBrewPack on this system can read.
#[cfg(target_os = "windows")]
const PATH_LIMIT: usize = 259;
/// The longest path, in UTF-8 bytes, that hacBrewPack on this system can read.
#[cfg(target_os = "macos")]
const PATH_LIMIT: usize = 1023;
/// The longest path, in UTF-8 bytes, that hacBrewPack on this system can read.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PATH_LIMIT: usize = 4094;

/// Builds NSPs from the runtime archive and hacBrewPack.
pub struct HacBrewPacker {
    runtime: PathBuf,
    tool: PathBuf,
}

impl HacBrewPacker {
    /// Creates the packer.
    ///
    /// `runtime` is the runtime archive ([`archive::write_archive`]): the loader `exefs/main`, the NPDM template
    /// `exefs/main.npdm`, the icon `control/icon_AmericanEnglish.dat`, the splash screen `logo/` and the runtime
    /// RomFS `romfs/`. `tool` is the hacBrewPack executable.
    pub fn new(runtime: impl Into<PathBuf>, tool: impl Into<PathBuf>) -> Self {
        Self {
            runtime: runtime.into(),
            tool: tool.into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        record: &BuildRecord,
        icon: Option<&IconJpeg>,
        package: &GamePackage,
        target: &PackTarget,
        work: &Path,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<StoredGame, AdapterError> {
        let input = work.join("in");
        let exefs = input.join("exefs");
        let control = input.join("control");
        let romfs = input.join("romfs");

        report(
            progress,
            BuildStage::Runtime,
            0,
            Some(format!("runtime: {}", self.runtime.display())),
        );
        let runtime = RuntimeArchive::open(&self.runtime)?;
        let game_folder = Path::new(&record.source.folder);
        let game = Tree::scan(game_folder)?;
        check_paths(&romfs, &runtime, &game, package)?;
        check_space(work, target, runtime.unpacked_size() + game.total_bytes())?;
        create_dir(&exefs)?;
        write(&exefs.join("main"), &runtime.read(RUNTIME_MAIN)?)?;
        let mut npdm = runtime.read(RUNTIME_NPDM)?;
        let required = address_space(game_folder, &record.source.executable)?;
        let space = record.source.address_space(required);
        npdm::set_title_id(&mut npdm, record.title_id)
            .and_then(|()| npdm::set_address_space(&mut npdm, space))
            .map_err(|error| AdapterError::Tool(format!("{RUNTIME_NPDM}: {error}")))?;
        write(&exefs.join("main.npdm"), &npdm)?;
        let logo = copy_logo(&runtime, &input)?;
        let runtime_files = runtime.extract(RUNTIME_ROMFS, &romfs, &mut |percent| {
            report(progress, BuildStage::Runtime, percent, None);
        })?;
        report(
            progress,
            BuildStage::Runtime,
            100,
            Some(format!(
                "runtime: {runtime_files} files match their checksums"
            )),
        );

        report(
            progress,
            BuildStage::Game,
            0,
            Some(format!(
                "game: {} → romfs/{}",
                game_folder.display(),
                package.game_dir
            )),
        );
        game.copy_to(&romfs.join(&package.game_dir), &mut |percent| {
            report(progress, BuildStage::Game, percent, None);
        })?;

        report(progress, BuildStage::Metadata, 0, None);
        for file in &package.romfs_files {
            let path = romfs.join(&file.path);
            if let Some(parent) = path.parent() {
                create_dir(parent)?;
            }
            write(&path, file.contents.as_bytes())?;
            report(
                progress,
                BuildStage::Metadata,
                0,
                Some(format!("romfs/{}", file.path)),
            );
        }
        create_dir(&control)?;
        write(&control.join("control.nacp"), &package.nacp)?;
        let icon_bytes = match icon {
            Some(icon) => icon.bytes().to_vec(),
            None => runtime.read(RUNTIME_ICON)?,
        };
        for name in &package.icon_files {
            write(&control.join(name), &icon_bytes)?;
        }
        let romfs_icon = romfs.join(ROMFS_ICON_FILE);
        if let Some(parent) = romfs_icon.parent() {
            create_dir(parent)?;
        }
        write(&romfs_icon, &icon_bytes)?;
        let icon_origin = if icon.is_some() {
            "game"
        } else {
            "runtime default"
        };
        report(
            progress,
            BuildStage::Metadata,
            100,
            Some(format!(
                "control: control.nacp, {} ({icon_origin} icon)",
                package.icon_files.join(", ")
            )),
        );

        let nca_dir = work.join("nca");
        let romfs_files = runtime_files + game.file_count() + package.romfs_files.len() + 1;
        self.run_tool(&input, logo, work, &nca_dir, target, romfs_files, progress)?;

        report(progress, BuildStage::Finish, 0, None);
        let file_name = format!("{}.nsp", record.title_id);
        let library = Path::new(&target.library_dir);
        let size_bytes = write_nsp(&nca_dir, record, icon, &library.join(&file_name))?;
        report(
            progress,
            BuildStage::Finish,
            100,
            Some(format!(
                "{} ({size_bytes} bytes)",
                library.join(&file_name).display()
            )),
        );
        Ok(StoredGame {
            record: record.clone(),
            file_name,
            size_bytes,
            icon: icon.cloned(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn run_tool(
        &self,
        input: &Path,
        logo: bool,
        work: &Path,
        nca_dir: &Path,
        target: &PackTarget,
        romfs_files: usize,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<(), AdapterError> {
        let keys = Path::new(&target.keys_path);
        if !keys.is_file() {
            return Err(AdapterError::NotFound(format!(
                "{}: no keys file",
                keys.display()
            )));
        }
        let mut args: Vec<OsString> = Vec::new();
        let mut arg = |name: &str, value: &Path| {
            args.push(name.into());
            args.push(value.into());
        };
        arg("--keyset", keys);
        arg("--exefsdir", &input.join("exefs"));
        arg("--controldir", &input.join("control"));
        arg("--romfsdir", &input.join("romfs"));
        if logo {
            arg("--logodir", &input.join(RUNTIME_LOGO));
        }
        arg("--tempdir", &work.join("temp"));
        arg("--ncadir", nca_dir);
        arg("--backupdir", &work.join("backup"));
        arg("--nspdir", &work.join("nsp"));
        if !logo {
            args.push("--nologo".into());
        }
        args.push("--keepncadir".into());

        report(
            progress,
            BuildStage::Pack,
            0,
            Some(format!("hacBrewPack: {}", self.tool.display())),
        );
        let mut written = 0usize;
        let mut percent = 0u8;
        let mut unreadable = None;
        let result = tool::run(&self.tool, &args, &mut |line| {
            if line.starts_with(ROMFS_WRITE_PREFIX) && line.contains("romfs") {
                written += 1;
                let share = written.min(romfs_files) * 95 / romfs_files.max(1);
                percent = u8::try_from(share).unwrap_or(95);
            }
            if let Some(path) = line
                .strip_prefix(OPEN_FAILED_PREFIX)
                .and_then(|rest| rest.strip_suffix('!'))
            {
                unreadable = Some(PathBuf::from(path));
            }
            report(progress, BuildStage::Pack, percent, Some(line.to_owned()));
        });
        if let Err(error) = result {
            return Err(match unreadable {
                Some(path) if path.starts_with(work) => {
                    AdapterError::Blocked(path.display().to_string())
                }
                _ => error,
            });
        }
        report(progress, BuildStage::Pack, 100, None);
        Ok(())
    }
}

impl Packer for HacBrewPacker {
    fn pack(
        &self,
        record: &BuildRecord,
        icon: Option<&IconJpeg>,
        target: &PackTarget,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<StoredGame, AdapterError> {
        let package = package(record).map_err(|error| AdapterError::Tool(error.to_string()))?;
        let library = Path::new(&target.library_dir);
        let work = service::build_dir(library, &record.title_id.to_string());
        if service::is_busy(&work) {
            return Err(service::already_running(&work));
        }
        remove_dir(&work)?;
        create_dir(&work)?;
        service::hide(&library.join(service::SERVICE_DIR));
        let lock = service::BuildLock::acquire(&work)?;
        let result = self.build(record, icon, &package, target, &work, progress);
        drop(lock);
        let _ = remove_dir(&work);
        result
    }

    fn clean(&self, library_dir: &str) -> Result<usize, AdapterError> {
        service::clean(Path::new(library_dir))
    }
}

fn check_paths(
    romfs: &Path,
    runtime: &RuntimeArchive,
    game: &Tree,
    package: &GamePackage,
) -> Result<(), AdapterError> {
    let game_paths: Vec<String> = game
        .paths()
        .iter()
        .map(|path| format!("{}/{path}", package.game_dir))
        .collect();
    let entries = runtime
        .paths(RUNTIME_ROMFS)
        .chain([package.game_dir.as_str()])
        .chain(game_paths.iter().map(String::as_str))
        .chain(package.romfs_files.iter().map(|file| file.path.as_str()))
        .chain([ROMFS_ICON_FILE]);
    path_limit::check(
        &romfs.to_string_lossy(),
        MAIN_SEPARATOR,
        entries,
        PATH_LIMIT,
    )
    .map_err(|error| AdapterError::PathTooLong(error.to_string()))
}

fn check_space(work: &Path, target: &PackTarget, content_bytes: u64) -> Result<(), AdapterError> {
    let available = fs4::available_space(work).map_err(|error| io_error(work, &error))?;
    disk_space::check_space(content_bytes, available)
        .map_err(|error| AdapterError::NoSpace(format!("{}: {error}", target.library_dir)))
}

fn copy_logo(runtime: &RuntimeArchive, input: &Path) -> Result<bool, AdapterError> {
    let paths: Vec<String> = LOGO_FILES
        .iter()
        .map(|name| format!("{RUNTIME_LOGO}/{name}"))
        .collect();
    if !paths.iter().all(|path| runtime.contains(path)) {
        return Ok(false);
    }
    let dest = input.join(RUNTIME_LOGO);
    create_dir(&dest)?;
    for (name, path) in LOGO_FILES.iter().zip(&paths) {
        write(&dest.join(name), &runtime.read(path)?)?;
    }
    Ok(true)
}

fn write_nsp(
    nca_dir: &Path,
    record: &BuildRecord,
    icon: Option<&IconJpeg>,
    nsp: &Path,
) -> Result<u64, AdapterError> {
    let mut ncas = Vec::new();
    for entry in fs::read_dir(nca_dir).map_err(|error| io_error(nca_dir, &error))? {
        let entry = entry.map_err(|error| io_error(nca_dir, &error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry
            .metadata()
            .map_err(|error| io_error(&entry.path(), &error))?;
        if metadata.is_file() && name.ends_with(".nca") {
            ncas.push((name, entry.path(), metadata.len()));
        }
    }
    if ncas.is_empty() {
        return Err(AdapterError::Tool(format!(
            "{}: hacBrewPack left no NCA",
            nca_dir.display()
        )));
    }
    ncas.sort();
    let json = serde_json::to_vec_pretty(record)
        .map_err(|error| AdapterError::Io(format!("{RECORD_FILE}: {error}")))?;
    let mut entries: Vec<Pfs0Entry> = ncas
        .iter()
        .map(|(name, _, size)| Pfs0Entry {
            name: name.clone(),
            size: *size,
        })
        .collect();
    let mut extras: Vec<(&str, &[u8])> = vec![(RECORD_FILE, &json)];
    if let Some(icon) = icon {
        extras.push((ICON_FILE, icon.bytes()));
    }
    entries.extend(extras.iter().map(|(name, bytes)| Pfs0Entry {
        name: (*name).to_owned(),
        size: bytes.len() as u64,
    }));
    let header = pfs0::header(&entries).map_err(|error| AdapterError::Tool(error.to_string()))?;

    let partial = nsp.with_extension("nsp.part");
    let written = (|| -> io::Result<u64> {
        let mut out = BufWriter::with_capacity(1 << 20, File::create(&partial)?);
        out.write_all(&header)?;
        for (_, path, _) in &ncas {
            io::copy(&mut File::open(path)?, &mut out)?;
        }
        for (_, bytes) in &extras {
            out.write_all(bytes)?;
        }
        out.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        fs::rename(&partial, nsp)?;
        fs::metadata(nsp).map(|metadata| metadata.len())
    })();
    written.map_err(|error| {
        let _ = fs::remove_file(&partial);
        io_error(nsp, &error)
    })
}

fn report(
    progress: &mut dyn FnMut(BuildProgress),
    stage: BuildStage,
    percent: u8,
    log: Option<String>,
) {
    progress(BuildProgress {
        stage,
        percent,
        log,
    });
}

fn address_space(game_folder: &Path, executable: &str) -> Result<AddressSpace, AdapterError> {
    let path = executable
        .split(['/', '\\'])
        .fold(game_folder.to_path_buf(), |path, part| path.join(part));
    let mut file = File::open(&path).map_err(|error| io_error(&path, &error))?;
    let size = file
        .metadata()
        .map_err(|error| io_error(&path, &error))?
        .len();
    let mut read = |offset: u64, len: usize| read_at(&mut file, size, offset, len);
    pe::required_address_space(&mut read)
        .map_err(|error| AdapterError::Tool(format!("{}: {error}", path.display())))
}

fn read_at(file: &mut File, size: u64, offset: u64, len: usize) -> Option<Vec<u8>> {
    if offset.checked_add(len as u64)? > size {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0; len];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), AdapterError> {
    fs::write(path, bytes).map_err(|error| io_error(path, &error))
}

fn create_dir(path: &Path) -> Result<(), AdapterError> {
    fs::create_dir_all(path).map_err(|error| io_error(path, &error))
}

fn remove_dir(path: &Path) -> Result<(), AdapterError> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(io_error(path, &error)),
        _ => Ok(()),
    }
}

pub(crate) fn io_error(path: &Path, error: &io::Error) -> AdapterError {
    let message = format!("{}: {error}", path.display());
    if is_blocked(error) {
        AdapterError::Blocked(message)
    } else if error.kind() == ErrorKind::NotFound {
        AdapterError::NotFound(message)
    } else {
        AdapterError::Io(message)
    }
}

/// Returns whether the system refused because the antivirus considered the file infected or has already deleted it.
#[cfg(target_os = "windows")]
pub(crate) fn is_blocked(error: &io::Error) -> bool {
    const ERROR_VIRUS_INFECTED: i32 = 225;
    const ERROR_VIRUS_DELETED: i32 = 226;
    matches!(
        error.raw_os_error(),
        Some(ERROR_VIRUS_INFECTED | ERROR_VIRUS_DELETED)
    )
}

/// Returns `false`: outside Windows the antivirus does not report a block with a separate error code.
#[cfg(not(target_os = "windows"))]
pub(crate) fn is_blocked(_error: &io::Error) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use carafe_core::TitleId;
    use carafe_core::metadata::Metadata;
    use carafe_core::record::{Arch, FORMAT_VERSION, GameSource};
    use carafe_core::settings::AutorunSettings;

    use super::*;

    fn record() -> BuildRecord {
        BuildRecord {
            format_version: FORMAT_VERSION,
            title_id: TitleId::parse("056694dd13640000").unwrap(),
            build_number: 1,
            runtime_version: "0.1.0".to_owned(),
            source: GameSource {
                folder: "D:\\Game\\openttd".to_owned(),
                executable: "openttd.exe".to_owned(),
                arch: Arch::X86,
                arch_override: None,
                arguments: Vec::new(),
            },
            metadata: Metadata::new("OpenTTD"),
            settings: AutorunSettings::default(),
        }
    }

    fn u32_at(bytes: &[u8], offset: usize) -> usize {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    }

    fn u64_at(bytes: &[u8], offset: usize) -> usize {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize
    }

    #[test]
    fn nsp_holds_the_ncas_and_the_record() {
        let dir = std::env::temp_dir().join(format!("carafe-pack-nsp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let nca_dir = dir.join("nca");
        fs::create_dir_all(&nca_dir).unwrap();
        fs::write(nca_dir.join("b.nca"), b"program").unwrap();
        fs::write(nca_dir.join("a.cnmt.nca"), b"meta").unwrap();
        fs::write(nca_dir.join("notes.txt"), b"skip").unwrap();
        let nsp = dir.join("056694dd13640000.nsp");
        let icon = IconJpeg::new(jpeg_256()).unwrap();
        let size = write_nsp(&nca_dir, &record(), Some(&icon), &nsp).unwrap();
        let bytes = fs::read(&nsp).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(size as usize, bytes.len());
        assert_eq!(&bytes[..4], b"PFS0");
        let count = u32_at(&bytes, 4);
        assert_eq!(count, 4);
        let data = 0x10 + count * 0x18 + u32_at(&bytes, 8);
        let table = 0x10 + count * 0x18;
        let entry = |index: usize| {
            let at = 0x10 + index * 0x18;
            let start = data + u64_at(&bytes, at);
            let end = start + u64_at(&bytes, at + 8);
            let name_at = table + u32_at(&bytes, at + 0x10);
            let name_end = bytes[name_at..].iter().position(|&b| b == 0).unwrap() + name_at;
            let name = String::from_utf8(bytes[name_at..name_end].to_vec()).unwrap();
            (name, bytes[start..end].to_vec())
        };
        assert_eq!(entry(0), ("a.cnmt.nca".to_owned(), b"meta".to_vec()));
        assert_eq!(entry(1), ("b.nca".to_owned(), b"program".to_vec()));
        let (name, json) = entry(2);
        assert_eq!(name, RECORD_FILE);
        let stored: BuildRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(stored, record());
        assert_eq!(entry(3), (ICON_FILE.to_owned(), jpeg_256()));
    }

    fn jpeg_256() -> Vec<u8> {
        let mut bytes = vec![
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x01, 0x00, 0x01, 0x00,
        ];
        bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00, 0xFF, 0xD9]);
        bytes
    }
}
