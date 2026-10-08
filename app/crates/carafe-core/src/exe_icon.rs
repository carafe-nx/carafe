//! Icon from `.exe` resources: the first icon group, and the largest icon in it.

use crate::pe::{PeError, PeImage, ReadAt, u16_at};

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const GROUP_ENTRY_SIZE: usize = 14;
const ICO_HEADER_SIZE: usize = 6 + 16;

/// Icon from an `.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeIcon {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Ready-made file: PNG if the icon is stored as PNG, otherwise a single-image ICO.
    pub file: Vec<u8>,
}

/// Finds the program icon in the `.exe` resources.
///
/// The first icon group (`RT_GROUP_ICON`) is taken, which is the one Explorer shows, and in it the icon
/// with the largest side; on a tie, the one with the greater color depth.
///
/// # Returns
///
/// `None` if the `.exe` has no icons.
///
/// # Errors
///
/// [`PeError`] if the file is not PE or its resources are corrupted.
pub fn find_icon(read: &mut ReadAt<'_>) -> Result<Option<ExeIcon>, PeError> {
    let image = PeImage::open(read)?;
    let Some(group) = image.first_resource(read, RT_GROUP_ICON, None)? else {
        return Ok(None);
    };
    let count = usize::from(u16_at(&group, 4).ok_or(PeError::BadResources)?);
    let mut best: Option<GroupEntry> = None;
    for index in 0..count {
        let at = 6 + index * GROUP_ENTRY_SIZE;
        let entry = group
            .get(at..at + GROUP_ENTRY_SIZE)
            .map(GroupEntry::parse)
            .ok_or(PeError::BadResources)?;
        if best.as_ref().is_none_or(|current| entry.beats(current)) {
            best = Some(entry);
        }
    }
    let Some(best) = best else {
        return Ok(None);
    };
    let Some(data) = image.first_resource(read, RT_ICON, Some(u32::from(best.id)))? else {
        return Ok(None);
    };
    let file = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        data
    } else {
        single_icon_file(&best, &data)
    };
    Ok(Some(ExeIcon {
        width: best.side(best.width),
        height: best.side(best.height),
        file,
    }))
}

struct GroupEntry {
    width: u8,
    height: u8,
    color_count: u8,
    planes: u16,
    bit_count: u16,
    id: u16,
}

impl GroupEntry {
    fn parse(bytes: &[u8]) -> Self {
        Self {
            width: bytes[0],
            height: bytes[1],
            color_count: bytes[2],
            planes: u16::from_le_bytes([bytes[4], bytes[5]]),
            bit_count: u16::from_le_bytes([bytes[6], bytes[7]]),
            id: u16::from_le_bytes([bytes[12], bytes[13]]),
        }
    }

    fn side(&self, value: u8) -> u32 {
        if value == 0 { 256 } else { u32::from(value) }
    }

    fn beats(&self, other: &Self) -> bool {
        let size = self.side(self.width).max(self.side(self.height));
        let other_size = other.side(other.width).max(other.side(other.height));
        (size, self.bit_count) > (other_size, other.bit_count)
    }
}

fn single_icon_file(entry: &GroupEntry, data: &[u8]) -> Vec<u8> {
    let mut file = Vec::with_capacity(ICO_HEADER_SIZE + data.len());
    file.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    file.extend_from_slice(&[entry.width, entry.height, entry.color_count, 0]);
    file.extend_from_slice(&entry.planes.to_le_bytes());
    file.extend_from_slice(&entry.bit_count.to_le_bytes());
    file.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_le_bytes());
    file.extend_from_slice(&u32::try_from(ICO_HEADER_SIZE).unwrap_or(0).to_le_bytes());
    file.extend_from_slice(data);
    file
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::fixture::{I386, exe, reader};
    use crate::pe::u32_at;

    fn with_icons(icons: &[(u8, u16, u16, &[u8])]) -> Vec<u8> {
        let mut group = vec![0, 0, 1, 0];
        group.extend_from_slice(&u16::try_from(icons.len()).unwrap().to_le_bytes());
        for (width, bits, id, data) in icons {
            group.extend_from_slice(&[*width, *width, 0, 0, 1, 0]);
            group.extend_from_slice(&bits.to_le_bytes());
            group.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
            group.extend_from_slice(&id.to_le_bytes());
        }
        let mut resources: Vec<(u32, u32, &[u8])> = icons
            .iter()
            .map(|(_, _, id, data)| (RT_ICON, u32::from(*id), *data))
            .collect();
        resources.push((RT_GROUP_ICON, 1, &group));
        exe(I386, &resources)
    }

    fn find(file: &[u8]) -> Result<Option<ExeIcon>, PeError> {
        find_icon(&mut reader(file))
    }

    #[test]
    fn largest_icon_wins_and_png_is_returned_as_is() {
        let png = b"\x89PNG\r\n\x1a\nbig";
        let file = with_icons(&[
            (32, 32, 1, b"small-bmp"),
            (0, 32, 2, png),
            (48, 32, 3, b"mid-bmp"),
        ]);
        let icon = find(&file).unwrap().unwrap();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.file, png);
    }

    #[test]
    fn bitmap_icons_are_wrapped_into_ico() {
        let file = with_icons(&[(32, 8, 1, b"low-color"), (32, 32, 2, b"true-color")]);
        let icon = find(&file).unwrap().unwrap();
        assert_eq!((icon.width, icon.height), (32, 32));
        assert_eq!(&icon.file[..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(icon.file[6], 32);
        assert_eq!(u16_at(&icon.file, 12), Some(32));
        assert_eq!(u32_at(&icon.file, 14), Some(10));
        assert_eq!(u32_at(&icon.file, 18), Some(22));
        assert_eq!(&icon.file[22..], b"true-color");
    }

    #[test]
    fn executables_without_icons_give_none() {
        assert_eq!(find(&exe(I386, &[])), Ok(None));
        assert_eq!(find(&with_icons(&[])), Ok(None));
    }

    #[test]
    fn foreign_files_are_not_pe() {
        assert_eq!(
            find(b"not an executable at all, just some text....................."),
            Err(PeError::NotPe)
        );
        assert_eq!(find(b"MZ"), Err(PeError::NotPe));
    }
}
