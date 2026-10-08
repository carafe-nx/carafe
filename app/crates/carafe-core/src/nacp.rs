//! The `control.nacp` file: the application title, version and properties that the Switch reads.

use crate::metadata::{Metadata, NacpLanguage};
use crate::title_id::TitleId;

/// Size of `control.nacp` in bytes.
pub const NACP_SIZE: usize = 0x4000;

/// Size of the application save data that the overlay writes files under `sdmc:` to.
pub const DEVICE_SAVE_DATA_SIZE: u64 = 0x1000_0000;

/// Size of the application save data journal.
pub const DEVICE_SAVE_DATA_JOURNAL_SIZE: u64 = 0x400_0000;

const TITLE_ENTRY_SIZE: usize = 0x300;
const NAME_SIZE: usize = 0x200;
const PUBLISHER_SIZE: usize = 0x100;
const ADD_ON_CONTENT_REGISTRATION_TYPE: usize = 0x3027;
const SUPPORTED_LANGUAGE_FLAG: usize = 0x302C;
const VIDEO_CAPTURE: usize = 0x3035;
const PRESENCE_GROUP_ID: usize = 0x3038;
const RATING_AGE: usize = 0x3040;
const DISPLAY_VERSION: usize = 0x3060;
const DISPLAY_VERSION_SIZE: usize = 0x10;
const ADD_ON_CONTENT_BASE_ID: usize = 0x3070;
const SAVE_DATA_OWNER_ID: usize = 0x3078;
const USER_ACCOUNT_SAVE_DATA_SIZE: usize = 0x3080;
const USER_ACCOUNT_SAVE_DATA_JOURNAL_SIZE: usize = 0x3088;
const DEVICE_SAVE_DATA_SIZE_OFFSET: usize = 0x3090;
const DEVICE_SAVE_DATA_JOURNAL_SIZE_OFFSET: usize = 0x3098;
const LOCAL_COMMUNICATION_ID: usize = 0x30B0;
const LOGO_TYPE: usize = 0x30F0;

const RATING_AGES: [u8; 12] = [
    0x0C, 0xFF, 0xFF, 0x0A, 0xFF, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0D, 0x0D,
];
const USER_ACCOUNT_SAVE_SIZE: u64 = 0x3E0_0000;
const USER_ACCOUNT_SAVE_JOURNAL_SIZE: u64 = 0x18_0000;
const ADD_ON_CONTENT_OFFSET: u64 = 0x1000;
const VIDEO_CAPTURE_AUTOMATIC: u8 = 2;
const LOGO_TYPE_NINTENDO: u8 = 2;

/// Returns the languages the NSP declares as supported: American English and every language
/// that has its own title.
///
/// For each of them, control gets its own icon `icon_<language>.dat`.
#[must_use]
pub fn supported_languages(metadata: &Metadata) -> Vec<NacpLanguage> {
    NacpLanguage::ALL
        .into_iter()
        .filter(|language| {
            *language == NacpLanguage::AmericanEnglish
                || metadata
                    .localized
                    .iter()
                    .any(|localized| localized.language == *language)
        })
        .collect()
}

/// Creates the game's `control.nacp`.
///
/// The main title and publisher are written into all 16 entries, per-language titles over their own entries.
/// Strings are UTF-8; those that do not fit the field are cut on a character boundary: the title to 511 bytes,
/// the publisher to 255, the version to 15. Other fields are as in the NACP made by `nacptool`
/// and the Carafe scripts: application save data [`DEVICE_SAVE_DATA_SIZE`] with a journal of
/// [`DEVICE_SAVE_DATA_JOURNAL_SIZE`], the Nintendo logo, the Title ID in the presence, save data owner
/// and local communication fields.
///
/// # Returns
///
/// Exactly [`NACP_SIZE`] bytes.
#[must_use]
pub fn render(metadata: &Metadata, display_version: &str, title_id: TitleId) -> Vec<u8> {
    let mut nacp = vec![0; NACP_SIZE];
    for language in NacpLanguage::ALL {
        let localized = metadata
            .localized
            .iter()
            .find(|localized| localized.language == language);
        let title = localized.map_or(metadata.title.as_str(), |localized| &localized.title);
        let publisher = localized
            .and_then(|localized| localized.publisher.as_deref())
            .unwrap_or(&metadata.publisher);
        let entry = language.index() * TITLE_ENTRY_SIZE;
        put_str(&mut nacp, entry, NAME_SIZE, title);
        put_str(&mut nacp, entry + NAME_SIZE, PUBLISHER_SIZE, publisher);
    }
    let language_flag = supported_languages(metadata)
        .into_iter()
        .fold(0u32, |flag, language| flag | 1 << language.index());
    let id = title_id.value();
    nacp[ADD_ON_CONTENT_REGISTRATION_TYPE] = 1;
    put(
        &mut nacp,
        SUPPORTED_LANGUAGE_FLAG,
        &language_flag.to_le_bytes(),
    );
    nacp[VIDEO_CAPTURE] = VIDEO_CAPTURE_AUTOMATIC;
    put(&mut nacp, PRESENCE_GROUP_ID, &id.to_le_bytes());
    nacp[RATING_AGE..RATING_AGE + 0x20].fill(0xFF);
    put(&mut nacp, RATING_AGE, &RATING_AGES);
    put_str(
        &mut nacp,
        DISPLAY_VERSION,
        DISPLAY_VERSION_SIZE,
        display_version,
    );
    let add_on_content_base = id + ADD_ON_CONTENT_OFFSET;
    put(
        &mut nacp,
        ADD_ON_CONTENT_BASE_ID,
        &add_on_content_base.to_le_bytes(),
    );
    put(&mut nacp, SAVE_DATA_OWNER_ID, &id.to_le_bytes());
    let user_save = USER_ACCOUNT_SAVE_SIZE.to_le_bytes();
    put(&mut nacp, USER_ACCOUNT_SAVE_DATA_SIZE, &user_save);
    let user_journal = USER_ACCOUNT_SAVE_JOURNAL_SIZE.to_le_bytes();
    put(
        &mut nacp,
        USER_ACCOUNT_SAVE_DATA_JOURNAL_SIZE,
        &user_journal,
    );
    let device_save = DEVICE_SAVE_DATA_SIZE.to_le_bytes();
    put(&mut nacp, DEVICE_SAVE_DATA_SIZE_OFFSET, &device_save);
    let device_journal = DEVICE_SAVE_DATA_JOURNAL_SIZE.to_le_bytes();
    put(
        &mut nacp,
        DEVICE_SAVE_DATA_JOURNAL_SIZE_OFFSET,
        &device_journal,
    );
    put(&mut nacp, LOCAL_COMMUNICATION_ID, &id.to_le_bytes());
    nacp[LOGO_TYPE] = LOGO_TYPE_NINTENDO;
    nacp
}

fn put(nacp: &mut [u8], offset: usize, bytes: &[u8]) {
    nacp[offset..offset + bytes.len()].copy_from_slice(bytes);
}

fn put_str(nacp: &mut [u8], offset: usize, field_size: usize, text: &str) {
    let mut end = text.len().min(field_size - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    put(nacp, offset, &text.as_bytes()[..end]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::LocalizedTitle;

    fn title_id() -> TitleId {
        TitleId::parse("056694dd13640000").unwrap()
    }

    fn u64_at(nacp: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(nacp[offset..offset + 8].try_into().unwrap())
    }

    fn str_at(nacp: &[u8], offset: usize, size: usize) -> &str {
        let field = &nacp[offset..offset + size];
        let end = field.iter().position(|&byte| byte == 0).unwrap_or(size);
        std::str::from_utf8(&field[..end]).unwrap()
    }

    #[test]
    fn title_and_publisher_fill_every_language() {
        let mut metadata = Metadata::new("OpenTTD x86");
        metadata.localized.push(LocalizedTitle {
            language: NacpLanguage::Russian,
            title: "Транспортный магнат".to_owned(),
            publisher: None,
        });
        let nacp = render(&metadata, "0.1.0", title_id());
        assert_eq!(str_at(&nacp, 0, NAME_SIZE), "OpenTTD x86");
        assert_eq!(str_at(&nacp, NAME_SIZE, PUBLISHER_SIZE), "Carafe");
        assert_eq!(
            str_at(&nacp, 15 * TITLE_ENTRY_SIZE, NAME_SIZE),
            "OpenTTD x86"
        );
        assert_eq!(
            str_at(&nacp, 11 * TITLE_ENTRY_SIZE, NAME_SIZE),
            "Транспортный магнат"
        );
        assert_eq!(
            str_at(&nacp, 11 * TITLE_ENTRY_SIZE + NAME_SIZE, PUBLISHER_SIZE),
            "Carafe"
        );
        let flag = u32::from_le_bytes(nacp[SUPPORTED_LANGUAGE_FLAG..][..4].try_into().unwrap());
        assert_eq!(flag, 1 | 1 << 11);
    }

    #[test]
    fn title_id_goes_to_owner_fields() {
        let nacp = render(&Metadata::new("OpenTTD"), "1.3", title_id());
        let id = title_id().value();
        assert_eq!(nacp.len(), NACP_SIZE);
        assert_eq!(u64_at(&nacp, PRESENCE_GROUP_ID), id);
        assert_eq!(u64_at(&nacp, SAVE_DATA_OWNER_ID), id);
        assert_eq!(u64_at(&nacp, LOCAL_COMMUNICATION_ID), id);
        assert_eq!(u64_at(&nacp, LOCAL_COMMUNICATION_ID + 8), 0);
        assert_eq!(u64_at(&nacp, ADD_ON_CONTENT_BASE_ID), id + 0x1000);
        assert_eq!(str_at(&nacp, DISPLAY_VERSION, DISPLAY_VERSION_SIZE), "1.3");
    }

    #[test]
    fn save_data_and_logo_match_the_scripts() {
        let nacp = render(&Metadata::new("OpenTTD"), "1.1", title_id());
        assert_eq!(u64_at(&nacp, DEVICE_SAVE_DATA_SIZE_OFFSET), 0x1000_0000);
        assert_eq!(
            u64_at(&nacp, DEVICE_SAVE_DATA_JOURNAL_SIZE_OFFSET),
            0x400_0000
        );
        assert_eq!(nacp[VIDEO_CAPTURE], 2);
        assert_eq!(nacp[LOGO_TYPE], 2);
        assert_eq!(nacp[LOGO_TYPE + 1], 0);
        assert_eq!(nacp[RATING_AGE + 1], 0xFF);
        assert_eq!(nacp[RATING_AGE + 0x1F], 0xFF);
    }

    #[test]
    fn long_strings_are_cut_on_a_character_boundary() {
        let nacp = render(&Metadata::new("ж".repeat(300)), "версия-1.2.3", title_id());
        assert_eq!(str_at(&nacp, 0, NAME_SIZE), "ж".repeat(255));
        assert_eq!(
            str_at(&nacp, DISPLAY_VERSION, DISPLAY_VERSION_SIZE),
            "версия-1."
        );
    }
}
