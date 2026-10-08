//! Icon pictures from disk: an adapter for the [`ImageReader`] port.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

use carafe_core::exe_icon::find_icon;
use carafe_core::icon::image_mime;
use carafe_core::ports::{AdapterError, ImageFile, ImageReader};

/// Pictures larger than this are not read: an icon needs less, and the window struggles with them.
pub const MAX_IMAGE_BYTES: u64 = 32 << 20;

/// Reads pictures and `.exe` files from a local drive.
pub struct FileImageReader;

impl ImageReader for FileImageReader {
    fn read_image(&self, path: &str) -> Result<ImageFile, AdapterError> {
        let path = Path::new(path);
        let size = fs::metadata(path)
            .map_err(|error| io_error(path, &error))?
            .len();
        if size > MAX_IMAGE_BYTES {
            return Err(AdapterError::Unsupported(format!(
                "{}: {size} bytes, more than {MAX_IMAGE_BYTES}",
                path.display()
            )));
        }
        let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
        let mime = image_mime(&bytes).ok_or_else(|| {
            AdapterError::Unsupported(format!("{}: not an image", path.display()))
        })?;
        Ok(ImageFile { mime, bytes })
    }

    fn executable_icon(&self, path: &str) -> Result<Option<ImageFile>, AdapterError> {
        let path = Path::new(path);
        let mut file = File::open(path).map_err(|error| io_error(path, &error))?;
        let mut read_error = None;
        let mut read = |offset: u64, len: usize| match read_at(&mut file, offset, len) {
            Ok(bytes) => bytes,
            Err(error) => {
                read_error = Some(error);
                None
            }
        };
        let found = find_icon(&mut read);
        if let Some(error) = read_error {
            return Err(io_error(path, &error));
        }
        let icon = found
            .map_err(|error| AdapterError::Unsupported(format!("{}: {error}", path.display())))?;
        Ok(icon.and_then(|icon| {
            image_mime(&icon.file).map(|mime| ImageFile {
                mime,
                bytes: icon.file,
            })
        }))
    }
}

fn read_at(file: &mut File, offset: u64, len: usize) -> io::Result<Option<Vec<u8>>> {
    let size = file.metadata()?.len();
    let end = offset.checked_add(len as u64);
    if end.is_none_or(|end| end > size) {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; len];
    file.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}

fn io_error(path: &Path, error: &io::Error) -> AdapterError {
    let message = format!("{}: {error}", path.display());
    if error.kind() == ErrorKind::NotFound {
        AdapterError::NotFound(message)
    } else {
        AdapterError::Io(message)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("carafe-images-{}-{name}", std::process::id()));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn images_are_read_with_their_type() {
        let path = temp_file("icon.png", b"\x89PNG\r\n\x1a\nrest");
        let image = FileImageReader.read_image(path.to_str().unwrap()).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(image.mime, "image/png");
    }

    #[test]
    fn other_files_are_unsupported() {
        let path = temp_file("notes.txt", b"just text");
        let result = FileImageReader.read_image(path.to_str().unwrap());
        let exe = FileImageReader.executable_icon(path.to_str().unwrap());
        fs::remove_file(&path).unwrap();
        assert!(matches!(result, Err(AdapterError::Unsupported(_))));
        assert!(matches!(exe, Err(AdapterError::Unsupported(_))));
    }

    #[test]
    fn missing_files_are_not_found() {
        let missing = "Z:/carafe/missing/game.exe";
        assert!(matches!(
            FileImageReader.executable_icon(missing),
            Err(AdapterError::NotFound(_))
        ));
    }
}
