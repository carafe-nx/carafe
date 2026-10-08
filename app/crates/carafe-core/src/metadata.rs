//! NSP metadata: title, publisher, version, icon.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Default publisher for a new game.
pub const DEFAULT_PUBLISHER: &str = "Carafe";

/// Switch system language: one of the 16 title entries in the NACP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum NacpLanguage {
    AmericanEnglish,
    BritishEnglish,
    Japanese,
    French,
    German,
    LatinAmericanSpanish,
    Spanish,
    Italian,
    Dutch,
    CanadianFrench,
    Portuguese,
    Russian,
    Korean,
    TraditionalChinese,
    SimplifiedChinese,
    BrazilianPortuguese,
}

impl NacpLanguage {
    /// All languages in NACP entry order.
    pub const ALL: [Self; 16] = [
        Self::AmericanEnglish,
        Self::BritishEnglish,
        Self::Japanese,
        Self::French,
        Self::German,
        Self::LatinAmericanSpanish,
        Self::Spanish,
        Self::Italian,
        Self::Dutch,
        Self::CanadianFrench,
        Self::Portuguese,
        Self::Russian,
        Self::Korean,
        Self::TraditionalChinese,
        Self::SimplifiedChinese,
        Self::BrazilianPortuguese,
    ];

    /// Returns the language entry number in the NACP, 0…15; it is also the bit number in `SupportedLanguageFlag`.
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Returns the language name in the form it takes in the icon file name `icon_<name>.dat`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::AmericanEnglish => "AmericanEnglish",
            Self::BritishEnglish => "BritishEnglish",
            Self::Japanese => "Japanese",
            Self::French => "French",
            Self::German => "German",
            Self::LatinAmericanSpanish => "LatinAmericanSpanish",
            Self::Spanish => "Spanish",
            Self::Italian => "Italian",
            Self::Dutch => "Dutch",
            Self::CanadianFrench => "CanadianFrench",
            Self::Portuguese => "Portuguese",
            Self::Russian => "Russian",
            Self::Korean => "Korean",
            Self::TraditionalChinese => "TraditionalChinese",
            Self::SimplifiedChinese => "SimplifiedChinese",
            Self::BrazilianPortuguese => "BrazilianPortuguese",
        }
    }
}

/// Title and publisher for one system language, replacing the main ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocalizedTitle {
    /// Switch system language.
    pub language: NacpLanguage,
    /// Title for this language.
    pub title: String,
    /// Publisher for this language; `None` means the main one.
    pub publisher: Option<String>,
}

/// Crop square in pixels of the source image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Crop {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Side of the square.
    pub size: u32,
}

/// Where the 256×256 icon comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum IconSource {
    /// Icon from the `.exe` on a colored background with the title below.
    #[default]
    Executable,
    /// Custom image with cropping.
    File {
        /// Path to the image on the PC.
        path: String,
        /// Crop square.
        crop: Crop,
    },
    /// Square cover from SteamGridDB with cropping.
    #[serde(rename = "steamGridDb", rename_all = "camelCase")]
    SteamGridDb {
        /// Game on SteamGridDB.
        game_id: u32,
        /// Cover on SteamGridDB.
        image_id: u32,
        /// URL of the full-size cover.
        url: String,
        /// Crop square.
        crop: Crop,
    },
}

impl IconSource {
    /// Returns whether the image the icon is made from is specified: for the icon from the `.exe`, always
    /// `true`; for a custom image and a SteamGridDB cover, `true` if the path or URL is not empty.
    #[must_use]
    pub fn is_chosen(&self) -> bool {
        match self {
            Self::Executable => true,
            Self::File { path, .. } => !path.trim().is_empty(),
            Self::SteamGridDb { url, .. } => !url.trim().is_empty(),
        }
    }
}

/// NSP metadata visible on the Switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Metadata {
    /// Title for all languages.
    pub title: String,
    /// Publisher for all languages.
    pub publisher: String,
    /// Title and publisher overrides for individual languages.
    pub localized: Vec<LocalizedTitle>,
    /// Version for display; `None` means the build number.
    pub display_version: Option<String>,
    /// Icon source.
    pub icon: IconSource,
}

impl Metadata {
    /// Creates metadata for a new game: the given title, publisher "Carafe", icon from the `.exe`.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            publisher: DEFAULT_PUBLISHER.to_owned(),
            localized: Vec::new(),
            display_version: None,
            icon: IconSource::Executable,
        }
    }

    /// Returns the version for display: the one entered manually or `1.<build number>`.
    #[must_use]
    pub fn version_for(&self, build_number: u32) -> String {
        match &self.display_version {
            Some(version) if !version.trim().is_empty() => version.trim().to_owned(),
            _ => format!("1.{build_number}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_is_published_by_carafe() {
        let metadata = Metadata::new("OpenTTD");
        assert_eq!(metadata.publisher, "Carafe");
        assert_eq!(metadata.icon, IconSource::Executable);
    }

    #[test]
    fn icon_source_needs_a_picked_image() {
        let crop = Crop {
            x: 0,
            y: 0,
            size: 0,
        };
        assert!(IconSource::Executable.is_chosen());
        assert!(
            !IconSource::File {
                path: String::new(),
                crop,
            }
            .is_chosen()
        );
        assert!(
            IconSource::File {
                path: "D:\\cover.png".to_owned(),
                crop,
            }
            .is_chosen()
        );
        assert!(
            !IconSource::SteamGridDb {
                game_id: 0,
                image_id: 0,
                url: String::new(),
                crop,
            }
            .is_chosen()
        );
        assert!(
            IconSource::SteamGridDb {
                game_id: 1,
                image_id: 2,
                url: "https://cdn2.steamgriddb.com/grid/a.png".to_owned(),
                crop,
            }
            .is_chosen()
        );
    }

    #[test]
    fn languages_follow_nacp_order() {
        for (index, language) in NacpLanguage::ALL.into_iter().enumerate() {
            assert_eq!(language.index(), index);
        }
        assert_eq!(NacpLanguage::Russian.index(), 11);
        assert_eq!(NacpLanguage::Russian.name(), "Russian");
    }

    #[test]
    fn version_falls_back_to_build_counter() {
        let mut metadata = Metadata::new("OpenTTD");
        assert_eq!(metadata.version_for(3), "1.3");
        metadata.display_version = Some("  ".to_owned());
        assert_eq!(metadata.version_for(3), "1.3");
        metadata.display_version = Some("15.3".to_owned());
        assert_eq!(metadata.version_for(3), "15.3");
    }
}
