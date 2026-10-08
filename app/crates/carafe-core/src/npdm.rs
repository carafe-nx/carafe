//! The loader's `main.npdm` file: process permissions, the application Title ID and its address space.

use thiserror::Error;

use crate::title_id::TitleId;

const META_MAGIC: &[u8; 4] = b"META";
const META_FLAGS: usize = 0x0C;
const ADDRESS_SPACE_MASK: u8 = 0x0E;
const ADDRESS_SPACE_SHIFT: u8 = 1;
const ACID_MAGIC: &[u8; 4] = b"ACID";
const ACI0_MAGIC: &[u8; 4] = b"ACI0";
const ACI0_OFFSET_FIELD: usize = 0x70;
const ACID_OFFSET_FIELD: usize = 0x78;
const ACID_MAGIC_OFFSET: usize = 0x200;
const ACID_RANGE_MIN: usize = 0x210;
const ACID_RANGE_MAX: usize = 0x218;
const ACI0_TITLE_ID: usize = 0x10;

/// Error parsing `main.npdm`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NpdmError {
    /// The file is shorter than its headers.
    #[error("main.npdm is cut off at offset {0:#x}")]
    Truncated(usize),
    /// There is no `META`, `ACID` or `ACI0` header where it should be.
    #[error("main.npdm has no {0} header")]
    MissingHeader(&'static str),
}

/// The address space the Switch kernel gives the application process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpace {
    /// 39 bits — for all programs that can be loaded at any address.
    Bits39,
    /// 32 bits without the alias region — for a 32-bit program that only works at its own
    /// address below 4 GB: in a 39-bit process memory below `0x8000000` is unavailable.
    Bits32NoAlias,
}

impl AddressSpace {
    const fn npdm_type(self) -> u8 {
        match self {
            Self::Bits39 => 3,
            Self::Bits32NoAlias => 2,
        }
    }
}

/// Writes the process address space type into `main.npdm`.
///
/// Only the type bits in the `META` flags field change; the code bitness and other flags stay.
///
/// # Errors
///
/// [`NpdmError::MissingHeader`] if this is not an NPDM; [`NpdmError::Truncated`] if the file is shorter
/// than the `META` header. In both cases `npdm` is left unchanged.
pub fn set_address_space(npdm: &mut [u8], space: AddressSpace) -> Result<(), NpdmError> {
    expect_magic(npdm, 0, META_MAGIC, "META")?;
    let flags = npdm
        .get_mut(META_FLAGS)
        .ok_or(NpdmError::Truncated(META_FLAGS))?;
    *flags = (*flags & !ADDRESS_SPACE_MASK) | (space.npdm_type() << ADDRESS_SPACE_SHIFT);
    Ok(())
}

/// Writes the application Title ID into `main.npdm`: into ACI0 and both bounds of the ACID allowed range.
///
/// Other bytes, including the ACID signature and key, are not changed.
///
/// # Errors
///
/// [`NpdmError::MissingHeader`] if this is not an NPDM; [`NpdmError::Truncated`] if the header
/// offsets go past the end of the file. In both cases `npdm` is left unchanged.
pub fn set_title_id(npdm: &mut [u8], title_id: TitleId) -> Result<(), NpdmError> {
    expect_magic(npdm, 0, META_MAGIC, "META")?;
    let aci0 = read_offset(npdm, ACI0_OFFSET_FIELD)?;
    let acid = read_offset(npdm, ACID_OFFSET_FIELD)?;
    expect_magic(npdm, acid + ACID_MAGIC_OFFSET, ACID_MAGIC, "ACID")?;
    expect_magic(npdm, aci0, ACI0_MAGIC, "ACI0")?;
    let fields = [
        acid + ACID_RANGE_MIN,
        acid + ACID_RANGE_MAX,
        aci0 + ACI0_TITLE_ID,
    ];
    if let Some(&end) = fields.iter().find(|&&field| field + 8 > npdm.len()) {
        return Err(NpdmError::Truncated(end));
    }
    let bytes = title_id.value().to_le_bytes();
    for field in fields {
        npdm[field..field + 8].copy_from_slice(&bytes);
    }
    Ok(())
}

fn read_offset(npdm: &[u8], field: usize) -> Result<usize, NpdmError> {
    let bytes = npdm
        .get(field..field + 4)
        .ok_or(NpdmError::Truncated(field))?;
    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    usize::try_from(value).map_err(|_| NpdmError::Truncated(field))
}

fn expect_magic(
    npdm: &[u8],
    offset: usize,
    magic: &[u8; 4],
    name: &'static str,
) -> Result<(), NpdmError> {
    match npdm.get(offset..offset + 4) {
        Some(found) if found == magic => Ok(()),
        Some(_) => Err(NpdmError::MissingHeader(name)),
        None => Err(NpdmError::Truncated(offset)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACID: usize = 0x80;
    const ACI0: usize = 0x340;

    fn template() -> Vec<u8> {
        let mut npdm = vec![0; 0x3F8];
        npdm[..4].copy_from_slice(META_MAGIC);
        npdm[ACI0_OFFSET_FIELD..][..4].copy_from_slice(&0x340u32.to_le_bytes());
        npdm[ACID_OFFSET_FIELD..][..4].copy_from_slice(&0x80u32.to_le_bytes());
        npdm[ACID + ACID_MAGIC_OFFSET..][..4].copy_from_slice(ACID_MAGIC);
        npdm[ACI0..][..4].copy_from_slice(ACI0_MAGIC);
        npdm
    }

    fn u64_at(npdm: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(npdm[offset..offset + 8].try_into().unwrap())
    }

    #[test]
    fn title_id_goes_to_aci0_and_acid_range() {
        let mut npdm = template();
        let id = TitleId::parse("056694dd13640000").unwrap();
        set_title_id(&mut npdm, id).unwrap();
        assert_eq!(u64_at(&npdm, ACI0 + ACI0_TITLE_ID), id.value());
        assert_eq!(u64_at(&npdm, ACID + ACID_RANGE_MIN), id.value());
        assert_eq!(u64_at(&npdm, ACID + ACID_RANGE_MAX), id.value());
        assert_eq!(npdm.len(), 0x3F8);
    }

    #[test]
    fn foreign_files_are_left_alone() {
        let mut npdm = template();
        npdm[0] = b'X';
        let before = npdm.clone();
        let id = TitleId::from_entropy(1);
        assert_eq!(
            set_title_id(&mut npdm, id),
            Err(NpdmError::MissingHeader("META"))
        );
        assert_eq!(npdm, before);
    }

    #[test]
    fn address_space_replaces_only_its_bits() {
        let mut npdm = template();
        npdm[META_FLAGS] = 0b0111;
        set_address_space(&mut npdm, AddressSpace::Bits32NoAlias).unwrap();
        assert_eq!(npdm[META_FLAGS], 0b0101);
        set_address_space(&mut npdm, AddressSpace::Bits39).unwrap();
        assert_eq!(npdm[META_FLAGS], 0b0111);
    }

    #[test]
    fn address_space_leaves_foreign_files_alone() {
        let mut npdm = template();
        npdm[0] = b'X';
        let before = npdm.clone();
        assert_eq!(
            set_address_space(&mut npdm, AddressSpace::Bits32NoAlias),
            Err(NpdmError::MissingHeader("META"))
        );
        assert_eq!(npdm, before);
    }

    #[test]
    fn truncated_files_are_rejected() {
        let mut npdm = template();
        npdm.truncate(ACI0 + 0x10);
        let id = TitleId::from_entropy(1);
        assert_eq!(
            set_title_id(&mut npdm, id),
            Err(NpdmError::Truncated(ACI0 + ACI0_TITLE_ID))
        );
    }
}
