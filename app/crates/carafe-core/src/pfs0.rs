//! Header of PFS0, the container an NSP is: writing and reading.

/// File inside a PFS0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pfs0Entry {
    /// File name without folders.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
}

const HEADER_SIZE: usize = 0x10;
const ENTRY_SIZE: usize = 0x18;
const STRING_TABLE_ALIGNMENT: usize = 0x20;

/// Creates a PFS0 header for the files that follow it back to back in the order of `entries`.
///
/// The layout is as in hacBrewPack: the header, the file entries, the name table padded with zeros
/// to a size that is a multiple of 0x20.
///
/// # Returns
///
/// The header bytes; the data of the first file starts right after them.
///
/// # Errors
///
/// [`Pfs0TooLarge`] if there are more files or name bytes than fit into the 32-bit PFS0 fields.
pub fn header(entries: &[Pfs0Entry]) -> Result<Vec<u8>, Pfs0TooLarge> {
    let names: usize = entries.iter().map(|entry| entry.name.len() + 1).sum();
    let table_size = names.div_ceil(STRING_TABLE_ALIGNMENT) * STRING_TABLE_ALIGNMENT;
    let mut out = Vec::with_capacity(HEADER_SIZE + entries.len() * ENTRY_SIZE + table_size);
    out.extend_from_slice(b"PFS0");
    out.extend_from_slice(&u32_field(entries.len())?.to_le_bytes());
    out.extend_from_slice(&u32_field(table_size)?.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    let mut data_offset = 0u64;
    let mut name_offset = 0usize;
    for entry in entries {
        out.extend_from_slice(&data_offset.to_le_bytes());
        out.extend_from_slice(&entry.size.to_le_bytes());
        out.extend_from_slice(&u32_field(name_offset)?.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        data_offset += entry.size;
        name_offset += entry.name.len() + 1;
    }
    for entry in entries {
        out.extend_from_slice(entry.name.as_bytes());
        out.push(0);
    }
    out.resize(out.len() + table_size - names, 0);
    Ok(out)
}

/// There are more files or names than a PFS0 can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("PFS0 cannot hold this many files or names")]
pub struct Pfs0TooLarge;

fn u32_field(value: usize) -> Result<u32, Pfs0TooLarge> {
    u32::try_from(value).map_err(|_| Pfs0TooLarge)
}

/// How many bytes at the start of a PFS0 must be read to learn the size of the whole header.
pub const PREFIX_SIZE: usize = HEADER_SIZE;

/// More files than this in a PFS0 are not read: an NSP has only a few.
pub const MAX_FILES: usize = 4096;

/// A name table larger than this is not read.
pub const MAX_STRING_TABLE: usize = 1 << 20;

/// File inside a read PFS0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pfs0File {
    /// File name.
    pub name: String,
    /// Data offset from the start of the container.
    pub offset: u64,
    /// Size in bytes.
    pub size: u64,
}

/// Error reading a PFS0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Pfs0Error {
    /// The file does not start with `PFS0`.
    #[error("not a PFS0")]
    NotPfs0,
    /// The header is cut off, too large or points beyond its bounds.
    #[error("the PFS0 header is corrupted")]
    Malformed,
}

/// Returns the size of a PFS0 header from its first [`PREFIX_SIZE`] bytes.
///
/// # Errors
///
/// [`Pfs0Error::NotPfs0`] if this is not a PFS0; [`Pfs0Error::Malformed`] if there are more files than
/// [`MAX_FILES`] or the name table is larger than [`MAX_STRING_TABLE`].
pub fn header_size(prefix: &[u8]) -> Result<usize, Pfs0Error> {
    if prefix.len() < PREFIX_SIZE {
        return Err(Pfs0Error::Malformed);
    }
    if !prefix.starts_with(b"PFS0") {
        return Err(Pfs0Error::NotPfs0);
    }
    let count = field(prefix, 4)?;
    let table = field(prefix, 8)?;
    if count > MAX_FILES || table > MAX_STRING_TABLE {
        return Err(Pfs0Error::Malformed);
    }
    Ok(HEADER_SIZE + count * ENTRY_SIZE + table)
}

/// Returns the PFS0 files from its whole header (of size [`header_size`]).
///
/// # Errors
///
/// [`Pfs0Error`] if the header is cut off or a file name goes beyond the name table.
pub fn files(header: &[u8]) -> Result<Vec<Pfs0File>, Pfs0Error> {
    let header_len = header_size(header)?;
    let header = header.get(..header_len).ok_or(Pfs0Error::Malformed)?;
    let count = field(header, 4)?;
    let names = &header[HEADER_SIZE + count * ENTRY_SIZE..];
    (0..count)
        .map(|index| {
            let entry = &header[HEADER_SIZE + index * ENTRY_SIZE..][..ENTRY_SIZE];
            let name = names.get(field(entry, 16)?..).ok_or(Pfs0Error::Malformed)?;
            let end = name
                .iter()
                .position(|&byte| byte == 0)
                .ok_or(Pfs0Error::Malformed)?;
            Ok(Pfs0File {
                name: String::from_utf8_lossy(&name[..end]).into_owned(),
                offset: u64_field(entry, 0)
                    .checked_add(header_len as u64)
                    .ok_or(Pfs0Error::Malformed)?,
                size: u64_field(entry, 8),
            })
        })
        .collect()
}

fn u64_field(entry: &[u8], at: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&entry[at..at + 8]);
    u64::from_le_bytes(bytes)
}

fn field(bytes: &[u8], at: usize) -> Result<usize, Pfs0Error> {
    let slice = bytes.get(at..at + 4).ok_or(Pfs0Error::Malformed)?;
    let value = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
    usize::try_from(value).map_err(|_| Pfs0Error::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, size: u64) -> Pfs0Entry {
        Pfs0Entry {
            name: name.to_owned(),
            size,
        }
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }

    #[test]
    fn entries_point_at_consecutive_data() {
        let header = header(&[entry("a.nca", 0x100), entry("carafe.json", 7)]).unwrap();
        assert_eq!(&header[..4], b"PFS0");
        assert_eq!(u32_at(&header, 4), 2);
        assert_eq!(u32_at(&header, 8), 0x20);
        assert_eq!(header.len(), 0x10 + 2 * 0x18 + 0x20);
        assert_eq!(u64_at(&header, 0x10), 0);
        assert_eq!(u64_at(&header, 0x18), 0x100);
        assert_eq!(u32_at(&header, 0x20), 0);
        assert_eq!(u64_at(&header, 0x28), 0x100);
        assert_eq!(u64_at(&header, 0x30), 7);
        assert_eq!(u32_at(&header, 0x38), 6);
        let table = &header[0x40..];
        assert_eq!(&table[..18], b"a.nca\0carafe.json\0");
        assert!(table[18..].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn aligned_table_gets_no_extra_padding() {
        let name = "x".repeat(0x1F);
        let header = header(&[entry(&name, 1)]).unwrap();
        assert_eq!(u32_at(&header, 8), 0x20);
        assert_eq!(header.len(), 0x10 + 0x18 + 0x20);
    }

    #[test]
    fn written_header_reads_back_with_absolute_offsets() {
        let written = header(&[entry("a.nca", 0x100), entry("carafe.json", 7)]).unwrap();
        assert_eq!(header_size(&written[..PREFIX_SIZE]), Ok(written.len()));
        let start = written.len() as u64;
        assert_eq!(
            files(&written).unwrap(),
            vec![
                Pfs0File {
                    name: "a.nca".to_owned(),
                    offset: start,
                    size: 0x100,
                },
                Pfs0File {
                    name: "carafe.json".to_owned(),
                    offset: start + 0x100,
                    size: 7,
                },
            ]
        );
    }

    #[test]
    fn foreign_and_broken_headers_are_rejected() {
        assert_eq!(header_size(b"NCA3............"), Err(Pfs0Error::NotPfs0));
        assert_eq!(header_size(b"PFS0"), Err(Pfs0Error::Malformed));
        let mut huge = b"PFS0".to_vec();
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        huge.extend_from_slice(&[0; 8]);
        assert_eq!(header_size(&huge), Err(Pfs0Error::Malformed));
        let written = header(&[entry("a.nca", 1)]).unwrap();
        assert_eq!(files(&written[..0x20]), Err(Pfs0Error::Malformed));
        let mut unterminated = written.clone();
        let table = unterminated.len() - 0x20;
        unterminated[table..].fill(b'x');
        assert_eq!(files(&unterminated), Err(Pfs0Error::Malformed));
    }
}
