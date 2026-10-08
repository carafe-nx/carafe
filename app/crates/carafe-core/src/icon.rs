//! NSP icon: the finished 256×256 JPEG and recognition of the images the window makes it from.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Icon side in pixels.
pub const ICON_SIZE: u16 = 256;

/// Largest icon size in bytes.
pub const MAX_ICON_BYTES: usize = 0x2_0000;

/// Icon validation error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IconError {
    /// This is not a JPEG, or the file is cut off before the frame header.
    #[error("the icon is not a JPEG")]
    NotJpeg,
    /// The JPEG is not baseline: progressive, arithmetic or lossless.
    #[error("the icon is not a baseline JPEG")]
    NotBaseline,
    /// The side is not 256 pixels.
    #[error("the icon is {width}×{height}, but 256×256 is required")]
    WrongSize {
        /// Width.
        width: u16,
        /// Height.
        height: u16,
    },
    /// The file is larger than [`MAX_ICON_BYTES`].
    #[error("the icon is {0} bytes, more than 128 KiB")]
    TooLarge(usize),
}

/// Finished NSP icon: a baseline 256×256 JPEG no larger than [`MAX_ICON_BYTES`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<u8>", into = "Vec<u8>")]
pub struct IconJpeg(Vec<u8>);

impl IconJpeg {
    /// Validates the bytes and creates the icon.
    ///
    /// # Errors
    ///
    /// [`IconError`] if this is not a baseline 256×256 JPEG or it is larger than [`MAX_ICON_BYTES`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, IconError> {
        if bytes.len() > MAX_ICON_BYTES {
            return Err(IconError::TooLarge(bytes.len()));
        }
        let (width, height) = baseline_size(&bytes)?;
        if (width, height) != (ICON_SIZE, ICON_SIZE) {
            return Err(IconError::WrongSize { width, height });
        }
        Ok(Self(bytes))
    }

    /// Returns the JPEG bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<Vec<u8>> for IconJpeg {
    type Error = IconError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::new(bytes)
    }
}

impl From<IconJpeg> for Vec<u8> {
    fn from(icon: IconJpeg) -> Self {
        icon.0
    }
}

fn baseline_size(bytes: &[u8]) -> Result<(u16, u16), IconError> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Err(IconError::NotJpeg);
    }
    let mut at = 2;
    loop {
        let rest = bytes.get(at..).ok_or(IconError::NotJpeg)?;
        let fill = rest.iter().take_while(|&&byte| byte == 0xFF).count();
        if fill == 0 {
            return Err(IconError::NotJpeg);
        }
        let marker = *rest.get(fill).ok_or(IconError::NotJpeg)?;
        let segment = at + fill + 1;
        match marker {
            0x01 | 0xD0..=0xD7 => {
                at = segment;
                continue;
            }
            0xC0 => {
                let height = read_u16(bytes, segment + 3)?;
                let width = read_u16(bytes, segment + 5)?;
                return Ok((width, height));
            }
            0xC1..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(IconError::NotBaseline);
            }
            0xD9 | 0xDA => return Err(IconError::NotJpeg),
            _ => {}
        }
        at = segment + usize::from(read_u16(bytes, segment)?);
    }
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, IconError> {
    match bytes.get(at..at + 2) {
        Some(&[high, low]) => Ok(u16::from_be_bytes([high, low])),
        _ => Err(IconError::NotJpeg),
    }
}

/// Returns a `data:` URL with base64 contents, which is how images are sent to the window.
#[must_use]
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(13 + mime.len() + bytes.len().div_ceil(3) * 4);
    out.push_str("data:");
    out.push_str(mime);
    out.push_str(";base64,");
    for chunk in bytes.chunks(3) {
        let triple = chunk.iter().enumerate().fold(0u32, |acc, (index, &byte)| {
            acc | u32::from(byte) << (16 - 8 * index)
        });
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (triple >> (18 - 6 * index)) & 0x3F;
                out.push(char::from(ALPHABET[sextet as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Returns the image MIME type by its first bytes: PNG, JPEG, GIF, WebP, BMP or ICO.
///
/// # Returns
///
/// `None` if the format is not in this list.
#[must_use]
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(sof: u8, width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, b'J', b'F'];
        bytes.extend_from_slice(&[0xFF, sof, 0x00, 0x0B, 0x08]);
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        bytes.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02, 0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn baseline_256_is_accepted() {
        let icon = IconJpeg::new(jpeg(0xC0, 256, 256)).unwrap();
        assert_eq!(icon.bytes()[..2], [0xFF, 0xD8]);
    }

    #[test]
    fn other_sizes_and_progressive_are_rejected() {
        assert_eq!(
            IconJpeg::new(jpeg(0xC0, 512, 256)),
            Err(IconError::WrongSize {
                width: 512,
                height: 256
            })
        );
        assert_eq!(
            IconJpeg::new(jpeg(0xC2, 256, 256)),
            Err(IconError::NotBaseline)
        );
        assert_eq!(
            IconJpeg::new(b"\x89PNG\r\n\x1a\n".to_vec()),
            Err(IconError::NotJpeg)
        );
        assert_eq!(IconJpeg::new(vec![0xFF, 0xD8]), Err(IconError::NotJpeg));
    }

    #[test]
    fn oversized_icons_are_rejected() {
        let mut bytes = jpeg(0xC0, 256, 256);
        bytes.resize(MAX_ICON_BYTES + 1, 0);
        assert_eq!(
            IconJpeg::new(bytes),
            Err(IconError::TooLarge(MAX_ICON_BYTES + 1))
        );
    }

    #[test]
    fn icons_travel_as_byte_arrays() {
        let bytes = jpeg(0xC0, 256, 256);
        let json = serde_json::to_string(&bytes).unwrap();
        let icon: IconJpeg = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&icon).unwrap(), json);
        assert!(serde_json::from_str::<IconJpeg>("[1,2,3]").is_err());
    }

    #[test]
    fn data_urls_use_padded_base64() {
        assert_eq!(data_url("image/png", b""), "data:image/png;base64,");
        assert_eq!(data_url("text/plain", b"f"), "data:text/plain;base64,Zg==");
        assert_eq!(data_url("text/plain", b"fo"), "data:text/plain;base64,Zm8=");
        assert_eq!(
            data_url("text/plain", b"foo"),
            "data:text/plain;base64,Zm9v"
        );
        assert_eq!(
            data_url("text/plain", &[0xFF, 0xEE, 0xDD, 0x01]),
            "data:text/plain;base64,/+7dAQ=="
        );
    }

    #[test]
    fn formats_are_recognised_by_magic() {
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_mime(&[0, 0, 1, 0, 1, 0]), Some("image/x-icon"));
        assert_eq!(image_mime(b"MZ\x90\0"), None);
    }
}
