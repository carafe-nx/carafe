//! Web pages the app opens in the system browser.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A web page the window may ask to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExternalLink {
    /// The source code of Carafe.
    Source,
    /// The text of Carafe's license.
    License,
}

impl ExternalLink {
    /// Returns the address of the page.
    pub fn url(self) -> &'static str {
        match self {
            Self::Source => "https://github.com/carafe-nx/carafe",
            Self::License => "https://github.com/carafe-nx/carafe/blob/main/LICENSE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_the_repository() {
        assert_eq!(
            ExternalLink::Source.url(),
            "https://github.com/carafe-nx/carafe"
        );
    }

    #[test]
    fn license_is_the_license_file_in_the_repository() {
        assert_eq!(
            ExternalLink::License.url(),
            "https://github.com/carafe-nx/carafe/blob/main/LICENSE"
        );
    }

    #[test]
    fn every_link_is_https() {
        for link in [ExternalLink::Source, ExternalLink::License] {
            assert!(link.url().starts_with("https://"), "{link:?}");
        }
    }
}
