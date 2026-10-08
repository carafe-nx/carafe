//! Cover art search on SteamGridDB: an adapter for the [`ArtSearch`] port.
//!
//! Requests go to API v2 with the user's key; TLS is provided by the system (`native-tls`), root
//! certificates are the system ones. Pictures are downloaded only from the SteamGridDB image server.

use std::time::Duration;

use carafe_core::icon::image_mime;
use carafe_core::ports::{AdapterError, ArtGame, ArtImage, ArtSearch, ImageFile};
use serde::Deserialize;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// The API address.
pub const API: &str = "https://www.steamgriddb.com/api/v2";

/// The image server; downloads are allowed only from it.
pub const IMAGE_HOST: &str = "https://cdn2.steamgriddb.com/";

const SQUARE_DIMENSIONS: &str = "512x512,1024x1024";
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_JSON_BYTES: u64 = 4 << 20;
const MAX_IMAGE_BYTES: u64 = 32 << 20;
const SECONDS_PER_DAY: i64 = 86_400;

/// A SteamGridDB client.
pub struct SteamGridDb {
    agent: Agent,
}

impl SteamGridDb {
    /// Creates a client with a 20 s timeout per request.
    #[must_use]
    pub fn new() -> Self {
        let tls = TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let config = Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(TIMEOUT))
            .user_agent(concat!("Carafe/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            agent: Agent::new_with_config(config),
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        api_key: &str,
        url: &str,
    ) -> Result<T, AdapterError> {
        let mut response = self
            .agent
            .get(url)
            .header("Authorization", &format!("Bearer {}", api_key.trim()))
            .call()
            .map_err(request_error)?;
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_JSON_BYTES)
            .read_to_vec()
            .map_err(request_error)?;
        serde_json::from_slice(&body)
            .map_err(|error| AdapterError::Network(format!("SteamGridDB answer: {error}")))
    }
}

impl Default for SteamGridDb {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtSearch for SteamGridDb {
    fn search_games(&self, api_key: &str, term: &str) -> Result<Vec<ArtGame>, AdapterError> {
        let term = term.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{API}/search/autocomplete/{}", encode_segment(term));
        let answer: Answer<Vec<GameData>> = self.get_json(api_key, &url)?;
        Ok(answer
            .data
            .into_iter()
            .map(|game| ArtGame {
                id: game.id,
                name: game.name,
                year: game.release_date.map(year_of),
            })
            .collect())
    }

    fn square_images(&self, api_key: &str, game_id: u32) -> Result<Vec<ArtImage>, AdapterError> {
        let url = format!("{API}/grids/game/{game_id}?dimensions={SQUARE_DIMENSIONS}&types=static");
        let answer: Answer<Vec<GridData>> = self.get_json(api_key, &url)?;
        Ok(answer
            .data
            .into_iter()
            .filter(|grid| grid.url.starts_with(IMAGE_HOST) && grid.thumb.starts_with(IMAGE_HOST))
            .map(|grid| ArtImage {
                id: grid.id,
                url: grid.url,
                thumb: grid.thumb,
                width: grid.width,
                height: grid.height,
            })
            .collect())
    }

    fn download(&self, url: &str) -> Result<ImageFile, AdapterError> {
        if !url.starts_with(IMAGE_HOST) {
            return Err(AdapterError::Unsupported(format!(
                "{url}: not a SteamGridDB image"
            )));
        }
        let mut response = self.agent.get(url).call().map_err(request_error)?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_IMAGE_BYTES)
            .read_to_vec()
            .map_err(request_error)?;
        let mime = image_mime(&bytes)
            .ok_or_else(|| AdapterError::Unsupported(format!("{url}: not an image")))?;
        Ok(ImageFile { mime, bytes })
    }
}

#[derive(Deserialize)]
struct Answer<T> {
    data: T,
}

#[derive(Deserialize)]
struct GameData {
    id: u32,
    name: String,
    release_date: Option<i64>,
}

#[derive(Deserialize)]
struct GridData {
    id: u32,
    url: String,
    thumb: String,
    width: Option<u32>,
    height: Option<u32>,
}

fn request_error(error: ureq::Error) -> AdapterError {
    match error {
        ureq::Error::StatusCode(status @ (401 | 403)) => {
            AdapterError::Unauthorized(format!("SteamGridDB: HTTP {status}"))
        }
        ureq::Error::StatusCode(status) => {
            AdapterError::Network(format!("SteamGridDB: HTTP {status}"))
        }
        other => AdapterError::Network(format!("SteamGridDB: {other}")),
    }
}

fn encode_segment(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn year_of(unix_seconds: i64) -> i32 {
    let days = unix_seconds.div_euclid(SECONDS_PER_DAY) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let year = year_of_era + era * 400 + i64::from(month_index >= 10);
    i32::try_from(year).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_terms_are_percent_encoded() {
        assert_eq!(encode_segment("Heroes III"), "Heroes%20III");
        assert_eq!(encode_segment("a/b?c"), "a%2Fb%3Fc");
        assert_eq!(encode_segment("Мор"), "%D0%9C%D0%BE%D1%80");
    }

    #[test]
    fn release_dates_give_years() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(946_684_799), 1999);
        assert_eq!(year_of(946_684_800), 2000);
        assert_eq!(year_of(1_791_302_400), 2026);
        assert_eq!(year_of(-1), 1969);
    }

    #[test]
    fn answers_are_parsed_leniently() {
        let json = r#"{"success":true,"data":[{"id":5254,"name":"OpenTTD","types":["steam"],
            "verified":true,"release_date":1396483200}]}"#;
        let answer: Answer<Vec<GameData>> = serde_json::from_str(json).unwrap();
        assert_eq!(answer.data[0].id, 5254);
        assert_eq!(answer.data[0].release_date.map(year_of), Some(2014));
        let grids = r#"{"success":true,"page":0,"total":1,"limit":50,"data":[{"id":1,"score":0,
            "style":"alternate","width":512,"height":512,"url":"https://cdn2.steamgriddb.com/grid/a.png",
            "thumb":"https://cdn2.steamgriddb.com/thumb/a.jpg","tags":[]}]}"#;
        let answer: Answer<Vec<GridData>> = serde_json::from_str(grids).unwrap();
        assert_eq!(answer.data[0].width, Some(512));
    }

    #[test]
    fn downloads_are_limited_to_the_image_server() {
        let result = SteamGridDb::new().download("https://example.com/a.png");
        assert!(matches!(result, Err(AdapterError::Unsupported(_))));
    }
}
