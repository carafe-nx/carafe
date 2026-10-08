//! Installation and logs through Windows Portable Devices.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, ErrorKind, Write};
use std::path::Path;

use carafe_core::TitleId;
use carafe_core::dbi::{InstallTarget, SD_CARD_STORAGE, is_log_file, logs_path, storage_matches};
use carafe_core::ports::{AdapterError, TransferProgress};

use crate::Throttle;
use crate::wpd::{Com, Object, Session, find_switch};

const READ_BUFFER: usize = 1 << 20;
const NOT_CONNECTED: &str =
    "Switch is not connected: start DBI in MTP mode and plug in the USB cable";

pub fn is_connected() -> bool {
    Com::init()
        .and_then(|com| find_switch(&com))
        .is_ok_and(|found| found.is_some())
}

pub fn install(
    nsp_path: &str,
    target: InstallTarget,
    progress: &mut dyn FnMut(TransferProgress),
) -> Result<(), AdapterError> {
    let path = Path::new(nsp_path);
    let file = File::open(path).map_err(|error| io_error(path, &error))?;
    let size = file
        .metadata()
        .map_err(|error| io_error(path, &error))?
        .len();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AdapterError::NotFound(nsp_path.to_owned()))?;
    let com = Com::init()?;
    let session = open(&com)?;
    let storage = session
        .storages()?
        .into_iter()
        .find(|storage| storage_matches(&storage.name, target.storage()))
        .ok_or_else(|| {
            AdapterError::Device(format!(
                "DBI shows no \"{}\" storage: is DBI in MTP mode?",
                target.storage()
            ))
        })?;
    let mut throttle = Throttle::new(size);
    let mut source = BufReader::with_capacity(READ_BUFFER, file);
    session.upload(&storage.id, name, size, &mut source, &mut |done| {
        if throttle.should_report(done) {
            progress(TransferProgress {
                done_bytes: done,
                total_bytes: size,
            });
        }
    })
}

pub fn fetch_logs(title_id: TitleId, dest_dir: &str) -> Result<usize, AdapterError> {
    let com = Com::init()?;
    let session = open(&com)?;
    let missing = || AdapterError::NotFound(format!("SD Card/{}", logs_path(title_id).join("/")));
    let mut folder = session
        .storages()?
        .into_iter()
        .find(|storage| storage_matches(&storage.name, SD_CARD_STORAGE))
        .ok_or_else(|| {
            AdapterError::Device(format!("DBI shows no \"{SD_CARD_STORAGE}\" storage"))
        })?;
    for part in logs_path(title_id) {
        folder = session
            .children(&folder.id)?
            .into_iter()
            .find(|child| child.is_folder && child.name.eq_ignore_ascii_case(&part))
            .ok_or_else(missing)?;
    }
    let dest = Path::new(dest_dir);
    match fs::remove_dir_all(dest) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(dest, &error)),
    }
    copy_folder(&session, &folder, dest)
}

pub fn storage_names() -> Result<Vec<String>, AdapterError> {
    let com = Com::init()?;
    let session = open(&com)?;
    Ok(session
        .storages()?
        .into_iter()
        .map(|storage| storage.name)
        .collect())
}

fn open(com: &Com) -> Result<Session<'_>, AdapterError> {
    let id = find_switch(com)?.ok_or_else(|| AdapterError::Device(NOT_CONNECTED.to_owned()))?;
    Session::open(com, &id)
}

fn copy_folder(session: &Session<'_>, folder: &Object, dest: &Path) -> Result<usize, AdapterError> {
    fs::create_dir_all(dest).map_err(|error| io_error(dest, &error))?;
    let mut copied = 0;
    for child in session.children(&folder.id)? {
        if child.name.is_empty() || child.name.contains(['/', '\\']) || child.name == ".." {
            continue;
        }
        let target = dest.join(&child.name);
        if child.is_folder {
            copied += copy_folder(session, &child, &target)?;
        } else if is_log_file(&child.name) {
            let file = File::create(&target).map_err(|error| io_error(&target, &error))?;
            let mut out = BufWriter::new(file);
            session.download(&child.id, &mut out)?;
            out.flush().map_err(|error| io_error(&target, &error))?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn io_error(path: &Path, error: &io::Error) -> AdapterError {
    let message = format!("{}: {error}", path.display());
    if error.kind() == ErrorKind::NotFound {
        AdapterError::NotFound(message)
    } else {
        AdapterError::Io(message)
    }
}
