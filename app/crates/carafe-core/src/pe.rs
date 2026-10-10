//! PE file headers and resources: the bitness of an `.exe`, its `ProductName` and `CompanyName`,
//! the address space it needs.

use thiserror::Error;

use crate::npdm::AddressSpace;
use crate::record::Arch;

const PE32: u16 = 0x10B;
const PE32_PLUS: u16 = 0x20B;
const MACHINE_I386: u16 = 0x14C;
const MACHINE_AMD64: u16 = 0x8664;
const RELOCS_STRIPPED: u16 = 0x0001;
const DYNAMIC_BASE: u16 = 0x0040;
const LIMIT_4G: u64 = 1 << 32;
const RESOURCE_DIRECTORY: usize = 2;
const RELOCATION_DIRECTORY: usize = 5;
const SUBDIRECTORY: u32 = 0x8000_0000;
const MAX_RESOURCE_BYTES: u32 = 16 << 20;
const RT_VERSION: u32 = 16;
const TEXT_VALUE: u16 = 1;
const ENGLISH_US: &str = "0409";

/// Reads `len` bytes of the file from offset `offset`; `None` means past the end of the file or a read error.
pub type ReadAt<'a> = dyn FnMut(u64, usize) -> Option<Vec<u8>> + 'a;

/// Error parsing a PE file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PeError {
    /// This is not a PE file, or its headers are cut off.
    #[error("not a PE file")]
    NotPe,
    /// Resources are present but not laid out as the format describes.
    #[error("PE resources are corrupted")]
    BadResources,
}

/// Information about an `.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeInfo {
    /// Bitness; `None` means neither x86 nor x86-64, for example ARM64.
    pub arch: Option<Arch>,
    /// `ProductName` from the version resource.
    pub product_name: Option<String>,
    /// `CompanyName` from the version resource.
    pub company_name: Option<String>,
}

/// Returns the bitness of an `.exe` and the strings of its version resource.
///
/// Strings are taken from the English (US) table, and if there is none or it lacks the string, from the first
/// table that has the string. Empty strings are treated as missing.
///
/// # Errors
///
/// [`PeError::NotPe`] if the file is not PE. A corrupted version resource is not an error: the strings
/// are `None` then.
pub fn read_info(read: &mut ReadAt<'_>) -> Result<PeInfo, PeError> {
    let image = PeImage::open(read)?;
    let arch = match image.machine {
        MACHINE_I386 => Some(Arch::X86),
        MACHINE_AMD64 => Some(Arch::X64),
        _ => None,
    };
    let strings = image
        .first_resource(read, RT_VERSION, None)
        .ok()
        .flatten()
        .map(|version| version_strings(&version))
        .unwrap_or_default();
    Ok(PeInfo {
        arch,
        product_name: pick(&strings, "ProductName"),
        company_name: pick(&strings, "CompanyName"),
    })
}

/// Returns the address space in which an `.exe` can run.
///
/// # Returns
///
/// [`AddressSpace::Bits32NoAlias`] for a 32-bit x86 image that cannot be loaded at a different
/// address: it has no relocation table (or has `IMAGE_FILE_RELOCS_STRIPPED` set), no
/// `IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE`, and the whole image lies below 4 GB. For all others,
/// [`AddressSpace::Bits39`].
///
/// # Errors
///
/// [`PeError::NotPe`] if the file is not PE or its headers are cut off.
pub fn required_address_space(read: &mut ReadAt<'_>) -> Result<AddressSpace, PeError> {
    let image = PeImage::open(read)?;
    let movable = (image.relocations_size != 0 && image.characteristics & RELOCS_STRIPPED == 0)
        || image.dll_characteristics & DYNAMIC_BASE != 0;
    let below_4g = image
        .image_base
        .checked_add(u64::from(image.image_size))
        .is_some_and(|end| end <= LIMIT_4G);
    let fixed_low = image.magic == PE32 && image.machine == MACHINE_I386 && !movable && below_4g;
    Ok(if fixed_low {
        AddressSpace::Bits32NoAlias
    } else {
        AddressSpace::Bits39
    })
}

struct StringTable {
    language: String,
    strings: Vec<(String, String)>,
}

fn pick(tables: &[StringTable], name: &str) -> Option<String> {
    let english = tables
        .iter()
        .filter(|table| table.language.to_ascii_lowercase().starts_with(ENGLISH_US));
    let others = tables
        .iter()
        .filter(|table| !table.language.to_ascii_lowercase().starts_with(ENGLISH_US));
    english.chain(others).find_map(|table| {
        table
            .strings
            .iter()
            .find(|(key, value)| key == name && !value.is_empty())
            .map(|(_, value)| value.clone())
    })
}

fn version_strings(resource: &[u8]) -> Vec<StringTable> {
    let Some(root) = blocks(resource).next() else {
        return Vec::new();
    };
    if root.key != "VS_VERSION_INFO" {
        return Vec::new();
    }
    blocks(root.children)
        .filter(|block| block.key == "StringFileInfo")
        .flat_map(|info| blocks(info.children).collect::<Vec<_>>())
        .map(|table| StringTable {
            language: table.key,
            strings: blocks(table.children)
                .map(|entry| (entry.key, utf16_until_nul(entry.value).trim().to_owned()))
                .collect(),
        })
        .collect()
}

struct Block<'a> {
    key: String,
    value: &'a [u8],
    children: &'a [u8],
}

fn blocks(bytes: &[u8]) -> impl Iterator<Item = Block<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let length = usize::from(u16_at(bytes, at)?);
        if length < 6 {
            return None;
        }
        let block = bytes.get(at..(at + length).min(bytes.len()))?;
        at = align4(at + length);
        parse_block(block)
    })
}

fn parse_block(block: &[u8]) -> Option<Block<'_>> {
    let value_length = usize::from(u16_at(block, 2)?);
    let kind = u16_at(block, 4)?;
    let key_units: Vec<u16> = block
        .get(6..)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit))
        .take_while(|&unit| unit != 0)
        .collect();
    let key_end = 6 + (key_units.len() + 1) * 2;
    let value_at = align4(key_end).min(block.len());
    let value_bytes = if kind == TEXT_VALUE {
        value_length * 2
    } else {
        value_length
    };
    let value_end = (value_at + value_bytes).min(block.len());
    let children_at = align4(value_at + value_bytes).min(block.len());
    Some(Block {
        key: String::from_utf16_lossy(&key_units),
        value: &block[value_at..value_end],
        children: &block[children_at..],
    })
}

fn utf16_until_nul(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

fn align4(value: usize) -> usize {
    value.next_multiple_of(4)
}

struct Section {
    virtual_address: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

/// PE file headers needed to read resources and choose the address space.
pub(crate) struct PeImage {
    machine: u16,
    magic: u16,
    characteristics: u16,
    image_base: u64,
    image_size: u32,
    dll_characteristics: u16,
    relocations_size: u32,
    sections: Vec<Section>,
    resource_rva: Option<u32>,
}

impl PeImage {
    /// Reads the headers and the section table.
    ///
    /// # Errors
    ///
    /// [`PeError::NotPe`] if the file is not PE or the headers are cut off.
    pub(crate) fn open(read: &mut ReadAt<'_>) -> Result<Self, PeError> {
        let dos = read(0, 0x40).ok_or(PeError::NotPe)?;
        if !dos.starts_with(b"MZ") {
            return Err(PeError::NotPe);
        }
        let pe = u64::from(u32_at(&dos, 0x3C).ok_or(PeError::NotPe)?);
        let headers = read(pe, 24).ok_or(PeError::NotPe)?;
        if !headers.starts_with(b"PE\0\0") {
            return Err(PeError::NotPe);
        }
        let machine = u16_at(&headers, 4).ok_or(PeError::NotPe)?;
        let section_count = usize::from(u16_at(&headers, 6).ok_or(PeError::NotPe)?);
        let optional_size = usize::from(u16_at(&headers, 20).ok_or(PeError::NotPe)?);
        let characteristics = u16_at(&headers, 22).ok_or(PeError::NotPe)?;
        let optional = read(pe + 24, optional_size).ok_or(PeError::NotPe)?;
        let magic = u16_at(&optional, 0).ok_or(PeError::NotPe)?;
        let (count_at, directories_at, image_base) = match magic {
            PE32 => (92, 96, u32_at(&optional, 28).map(u64::from)),
            PE32_PLUS => (108, 112, u64_at(&optional, 24)),
            _ => return Err(PeError::NotPe),
        };
        let image_base = image_base.ok_or(PeError::NotPe)?;
        let image_size = u32_at(&optional, 56).ok_or(PeError::NotPe)?;
        let dll_characteristics = u16_at(&optional, 70).ok_or(PeError::NotPe)?;
        let directory_count = u32_at(&optional, count_at).ok_or(PeError::NotPe)? as usize;
        let directory = |index: usize, field: usize| {
            if directory_count > index {
                u32_at(&optional, directories_at + index * 8 + field).ok_or(PeError::NotPe)
            } else {
                Ok(0)
            }
        };
        let resource_rva = directory(RESOURCE_DIRECTORY, 0)?;
        let relocations_size = directory(RELOCATION_DIRECTORY, 4)?;
        let table_at = pe + 24 + optional_size as u64;
        let table = read(table_at, section_count * 40).ok_or(PeError::NotPe)?;
        let sections = table
            .as_chunks::<40>()
            .0
            .iter()
            .map(|section| Section {
                virtual_size: u32_at(section, 8).unwrap_or(0),
                virtual_address: u32_at(section, 12).unwrap_or(0),
                raw_size: u32_at(section, 16).unwrap_or(0),
                raw_offset: u32_at(section, 20).unwrap_or(0),
            })
            .collect();
        Ok(Self {
            machine,
            magic,
            characteristics,
            image_base,
            image_size,
            dll_characteristics,
            relocations_size,
            sections,
            resource_rva: (resource_rva != 0).then_some(resource_rva),
        })
    }

    fn offset_of(&self, rva: u32) -> Option<u64> {
        self.sections.iter().find_map(|section| {
            let span = section.virtual_size.max(section.raw_size);
            let inside = rva >= section.virtual_address && rva - section.virtual_address < span;
            let delta = rva.checked_sub(section.virtual_address)?;
            (inside && delta < section.raw_size)
                .then(|| u64::from(section.raw_offset) + u64::from(delta))
        })
    }

    fn read_rva(&self, read: &mut ReadAt<'_>, rva: u32, len: usize) -> Option<Vec<u8>> {
        read(self.offset_of(rva)?, len)
    }

    fn entries(
        &self,
        read: &mut ReadAt<'_>,
        resources: u32,
        directory: u32,
    ) -> Result<Vec<(u32, u32)>, PeError> {
        let rva = resources
            .checked_add(directory)
            .ok_or(PeError::BadResources)?;
        let header = self.read_rva(read, rva, 16).ok_or(PeError::BadResources)?;
        let named = u16_at(&header, 12).ok_or(PeError::BadResources)?;
        let ids = u16_at(&header, 14).ok_or(PeError::BadResources)?;
        let count = usize::from(named) + usize::from(ids);
        if count == 0 {
            return Ok(Vec::new());
        }
        let list = self
            .read_rva(read, rva + 16, count * 8)
            .ok_or(PeError::BadResources)?;
        Ok(list
            .as_chunks::<8>()
            .0
            .iter()
            .map(|entry| (u32_at(entry, 0).unwrap_or(0), u32_at(entry, 4).unwrap_or(0)))
            .collect())
    }

    /// Returns the data of a resource of type `kind`: the one with number `id`, or the first one if `id` is not set.
    /// Of the language variants, the first is taken.
    ///
    /// # Returns
    ///
    /// `None` if the file has no resources or the wanted one is not among them.
    ///
    /// # Errors
    ///
    /// [`PeError::BadResources`] if the resource tree is corrupted.
    pub(crate) fn first_resource(
        &self,
        read: &mut ReadAt<'_>,
        kind: u32,
        id: Option<u32>,
    ) -> Result<Option<Vec<u8>>, PeError> {
        let Some(resources) = self.resource_rva else {
            return Ok(None);
        };
        let Some(&(_, types)) = self
            .entries(read, resources, 0)?
            .iter()
            .find(|(name, _)| *name == kind)
        else {
            return Ok(None);
        };
        let names = self.entries(read, resources, subdirectory(types)?)?;
        let name = match id {
            Some(id) => names.iter().find(|(name, _)| *name == id),
            None => names.first(),
        };
        let Some(&(_, languages)) = name else {
            return Ok(None);
        };
        let Some(&(_, data)) = self
            .entries(read, resources, subdirectory(languages)?)?
            .first()
        else {
            return Ok(None);
        };
        if data & SUBDIRECTORY != 0 {
            return Err(PeError::BadResources);
        }
        let entry_rva = resources.checked_add(data).ok_or(PeError::BadResources)?;
        let entry = self
            .read_rva(read, entry_rva, 8)
            .ok_or(PeError::BadResources)?;
        let data_rva = u32_at(&entry, 0).ok_or(PeError::BadResources)?;
        let size = u32_at(&entry, 4).ok_or(PeError::BadResources)?;
        if size > MAX_RESOURCE_BYTES {
            return Err(PeError::BadResources);
        }
        self.read_rva(read, data_rva, size as usize)
            .map(Some)
            .ok_or(PeError::BadResources)
    }
}

fn subdirectory(offset: u32) -> Result<u32, PeError> {
    if offset & SUBDIRECTORY == 0 {
        return Err(PeError::BadResources);
    }
    Ok(offset & !SUBDIRECTORY)
}

/// Reads a little-endian `u16` at offset `at`.
pub(crate) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// Reads a little-endian `u32` at offset `at`.
pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::{MACHINE_I386, PE32, SUBDIRECTORY};

    const RSRC_RVA: u32 = 0x2000;
    const RSRC_RAW: usize = 0x400;

    pub(crate) const I386: u16 = MACHINE_I386;

    fn put(buffer: &mut Vec<u8>, at: usize, bytes: &[u8]) {
        if buffer.len() < at + bytes.len() {
            buffer.resize(at + bytes.len(), 0);
        }
        buffer[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn directory(rsrc: &mut Vec<u8>, count: usize) -> usize {
        let at = rsrc.len();
        rsrc.resize(at + 16 + count * 8, 0);
        put(rsrc, at + 14, &u16::try_from(count).unwrap().to_le_bytes());
        at
    }

    fn set_entry(rsrc: &mut Vec<u8>, directory: usize, index: usize, name: u32, offset: u32) {
        put(rsrc, directory + 16 + index * 8, &name.to_le_bytes());
        put(rsrc, directory + 20 + index * 8, &offset.to_le_bytes());
    }

    fn offset(at: usize) -> u32 {
        u32::try_from(at).unwrap()
    }

    /// Builds a PE file with machine `machine` and resources `(type, number, data)`.
    pub(crate) fn exe(machine: u16, resources: &[(u32, u32, &[u8])]) -> Vec<u8> {
        let mut kinds: Vec<u32> = resources.iter().map(|(kind, _, _)| *kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        let mut rsrc = Vec::new();
        let root = directory(&mut rsrc, kinds.len());
        let mut data = Vec::new();
        for (kind_index, kind) in kinds.iter().enumerate() {
            let items: Vec<_> = resources.iter().filter(|item| item.0 == *kind).collect();
            let names = directory(&mut rsrc, items.len());
            set_entry(
                &mut rsrc,
                root,
                kind_index,
                *kind,
                SUBDIRECTORY | offset(names),
            );
            for (name_index, (_, id, bytes)) in items.iter().enumerate() {
                let languages = directory(&mut rsrc, 1);
                set_entry(
                    &mut rsrc,
                    names,
                    name_index,
                    *id,
                    SUBDIRECTORY | offset(languages),
                );
                let entry = rsrc.len();
                rsrc.resize(entry + 16, 0);
                set_entry(&mut rsrc, languages, 0, 0x409, offset(entry));
                data.push((entry, *bytes));
            }
        }
        for (entry, bytes) in data {
            let at = rsrc.len().next_multiple_of(16);
            put(&mut rsrc, at, bytes);
            put(&mut rsrc, entry, &(RSRC_RVA + offset(at)).to_le_bytes());
            put(&mut rsrc, entry + 4, &offset(bytes.len()).to_le_bytes());
        }

        let mut file = Vec::new();
        put(&mut file, 0, b"MZ");
        put(&mut file, 0x3C, &0x80u32.to_le_bytes());
        put(&mut file, 0x80, b"PE\0\0");
        put(&mut file, 0x84, &machine.to_le_bytes());
        put(&mut file, 0x86, &1u16.to_le_bytes());
        put(&mut file, 0x94, &0xE0u16.to_le_bytes());
        put(&mut file, 0x98, &PE32.to_le_bytes());
        put(&mut file, 0x98 + 92, &16u32.to_le_bytes());
        put(&mut file, 0x98 + 96 + 16, &RSRC_RVA.to_le_bytes());
        let section = 0x98 + 0xE0;
        let size = offset(rsrc.len()).to_le_bytes();
        put(&mut file, section, b".rsrc\0\0\0");
        put(&mut file, section + 8, &size);
        put(&mut file, section + 12, &RSRC_RVA.to_le_bytes());
        put(&mut file, section + 16, &size);
        put(&mut file, section + 20, &offset(RSRC_RAW).to_le_bytes());
        put(&mut file, RSRC_RAW, &rsrc);
        file
    }

    /// Returns a function that reads from an in-memory buffer.
    pub(crate) fn reader(file: &[u8]) -> impl FnMut(u64, usize) -> Option<Vec<u8>> + '_ {
        |offset: u64, len: usize| {
            let start = usize::try_from(offset).ok()?;
            file.get(start..start.checked_add(len)?).map(<[u8]>::to_vec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{I386, exe, reader};
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    fn block(key: &str, kind: u16, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = vec![0; 6];
        bytes.extend(utf16(key));
        bytes.resize(align4(bytes.len()), 0);
        bytes.extend_from_slice(value);
        let value_length = if kind == TEXT_VALUE {
            value.len() / 2
        } else {
            value.len()
        };
        for child in children {
            bytes.resize(align4(bytes.len()), 0);
            bytes.extend_from_slice(child);
        }
        let length = u16::try_from(bytes.len()).unwrap();
        bytes[0..2].copy_from_slice(&length.to_le_bytes());
        bytes[2..4].copy_from_slice(&u16::try_from(value_length).unwrap().to_le_bytes());
        bytes[4..6].copy_from_slice(&kind.to_le_bytes());
        bytes
    }

    fn version(tables: &[(&str, &[(&str, &str)])]) -> Vec<u8> {
        let tables: Vec<Vec<u8>> = tables
            .iter()
            .map(|(language, strings)| {
                let strings: Vec<Vec<u8>> = strings
                    .iter()
                    .map(|(key, value)| block(key, TEXT_VALUE, &utf16(value), &[]))
                    .collect();
                block(language, TEXT_VALUE, &[], &strings)
            })
            .collect();
        let info = block("StringFileInfo", TEXT_VALUE, &[], &tables);
        let translation = block("VarFileInfo", TEXT_VALUE, &[], &[]);
        block("VS_VERSION_INFO", 0, &[0xBD; 52], &[info, translation])
    }

    fn info(file: &[u8]) -> Result<PeInfo, PeError> {
        read_info(&mut reader(file))
    }

    #[test]
    fn product_and_company_are_read_from_the_version_resource() {
        let resource = version(&[(
            "040904b0",
            &[("CompanyName", "OpenTTD"), ("ProductName", "OpenTTD ")],
        )]);
        let found = info(&exe(I386, &[(RT_VERSION, 1, &resource)])).unwrap();
        assert_eq!(found.arch, Some(Arch::X86));
        assert_eq!(found.product_name.as_deref(), Some("OpenTTD"));
        assert_eq!(found.company_name.as_deref(), Some("OpenTTD"));
    }

    #[test]
    fn english_table_wins_and_others_fill_gaps() {
        let resource = version(&[
            (
                "041904b0",
                &[("ProductName", "Герои"), ("CompanyName", "Бука")],
            ),
            (
                "040904b0",
                &[("ProductName", "Heroes"), ("CompanyName", "")],
            ),
        ]);
        let found = info(&exe(I386, &[(RT_VERSION, 1, &resource)])).unwrap();
        assert_eq!(found.product_name.as_deref(), Some("Heroes"));
        assert_eq!(found.company_name.as_deref(), Some("Бука"));
    }

    #[test]
    fn machine_gives_the_arch() {
        assert_eq!(
            info(&exe(MACHINE_AMD64, &[])).unwrap().arch,
            Some(Arch::X64)
        );
        assert_eq!(info(&exe(0xAA64, &[])).unwrap().arch, None);
    }

    #[test]
    fn missing_or_broken_version_gives_no_strings() {
        let plain = info(&exe(I386, &[])).unwrap();
        assert_eq!((plain.product_name, plain.company_name), (None, None));
        let broken = info(&exe(I386, &[(RT_VERSION, 1, &[0xFF; 3])])).unwrap();
        assert_eq!(broken.product_name, None);
    }

    struct Header {
        machine: u16,
        image_base: u32,
        image_size: u32,
        relocations: u32,
        characteristics: u16,
        dll_characteristics: u16,
    }

    const NFSU: Header = Header {
        machine: I386,
        image_base: 0x40_0000,
        image_size: 0x39_5000,
        relocations: 0,
        characteristics: 0x010F,
        dll_characteristics: 0,
    };

    fn space(header: &Header) -> Result<AddressSpace, PeError> {
        const OPTIONAL: usize = 0x98;
        let mut file = exe(header.machine, &[]);
        file[0x96..0x98].copy_from_slice(&header.characteristics.to_le_bytes());
        file[OPTIONAL + 28..OPTIONAL + 32].copy_from_slice(&header.image_base.to_le_bytes());
        file[OPTIONAL + 56..OPTIONAL + 60].copy_from_slice(&header.image_size.to_le_bytes());
        file[OPTIONAL + 70..OPTIONAL + 72]
            .copy_from_slice(&header.dll_characteristics.to_le_bytes());
        let relocations = OPTIONAL + 96 + RELOCATION_DIRECTORY * 8 + 4;
        file[relocations..relocations + 4].copy_from_slice(&header.relocations.to_le_bytes());
        required_address_space(&mut reader(&file))
    }

    #[test]
    fn fixed_base_x86_image_needs_a_32_bit_space() {
        assert_eq!(space(&NFSU), Ok(AddressSpace::Bits32NoAlias));
    }

    #[test]
    fn stripped_relocations_do_not_count() {
        let header = Header {
            relocations: 0x80,
            characteristics: NFSU.characteristics | RELOCS_STRIPPED,
            ..NFSU
        };
        assert_eq!(space(&header), Ok(AddressSpace::Bits32NoAlias));
    }

    #[test]
    fn movable_images_keep_the_39_bit_space() {
        let relocated = Header {
            relocations: 0x8_0A00,
            characteristics: 0x0102,
            ..NFSU
        };
        let dynamic = Header {
            dll_characteristics: DYNAMIC_BASE,
            ..NFSU
        };
        assert_eq!(space(&relocated), Ok(AddressSpace::Bits39));
        assert_eq!(space(&dynamic), Ok(AddressSpace::Bits39));
    }

    #[test]
    fn other_machines_and_high_images_keep_the_39_bit_space() {
        let amd64 = Header {
            machine: MACHINE_AMD64,
            ..NFSU
        };
        let crossing_4g = Header {
            image_base: 0xFFFF_0000,
            image_size: 0x2_0000,
            ..NFSU
        };
        assert_eq!(space(&amd64), Ok(AddressSpace::Bits39));
        assert_eq!(space(&crossing_4g), Ok(AddressSpace::Bits39));
    }

    #[test]
    fn address_space_of_a_foreign_file_is_an_error() {
        assert_eq!(
            required_address_space(&mut reader(b"MZ")),
            Err(PeError::NotPe)
        );
    }

    #[test]
    fn foreign_files_are_not_pe() {
        assert_eq!(info(b"MZ"), Err(PeError::NotPe));
        assert_eq!(info(&[b'Z'; 0x80]), Err(PeError::NotPe));
    }
}
