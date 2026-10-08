//! Icon sources: an image from disk, the `.exe` icon, SteamGridDB covers.

use std::path::Path;

use carafe_core::icon::data_url;
use carafe_core::ports::{AdapterError, ArtGame, ArtImage, ImageFile};
use tauri::{AppHandle, Manager, State};

use crate::error::CommandError;
use crate::services::Services;

/// Reads an image from disk and returns it as a `data:` URL.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound`, `io` or `unsupported` if the file cannot be read or is not an image.
#[tauri::command]
pub async fn read_image(app: AppHandle, path: String) -> Result<String, CommandError> {
    blocking(app, move |services| services.images.read_image(&path))
        .await
        .map(|image| to_url(&image))
}

/// Returns the largest icon of the game's `.exe` as a `data:` URL; `None` if there are no icons.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound`, `io` or `unsupported` if the `.exe` cannot be read.
#[tauri::command]
pub async fn executable_icon(
    app: AppHandle,
    folder: String,
    executable: String,
) -> Result<Option<String>, CommandError> {
    let path = Path::new(&folder).join(&executable);
    let path = path.to_string_lossy().into_owned();
    blocking(app, move |services| services.images.executable_icon(&path))
        .await
        .map(|icon| icon.map(|image| to_url(&image)))
}

/// Searches SteamGridDB for games by title.
///
/// # Errors
///
/// [`CommandError`] of kind `apiKey` if the key is not set or was rejected; `network` on a network error.
#[tauri::command]
pub async fn art_games(
    app: AppHandle,
    services: State<'_, Services>,
    term: String,
) -> Result<Vec<ArtGame>, CommandError> {
    let key = api_key(&services)?;
    blocking(app, move |services| services.art.search_games(&key, &term)).await
}

/// Returns the game's square covers from SteamGridDB.
///
/// # Errors
///
/// [`CommandError`] of kind `apiKey` if the key is not set or was rejected; `network` on a network error.
#[tauri::command]
pub async fn art_images(
    app: AppHandle,
    services: State<'_, Services>,
    game_id: u32,
) -> Result<Vec<ArtImage>, CommandError> {
    let key = api_key(&services)?;
    blocking(app, move |services| {
        services.art.square_images(&key, game_id)
    })
    .await
}

/// Downloads a SteamGridDB cover and returns it as a `data:` URL.
///
/// # Errors
///
/// [`CommandError`] of kind `unsupported` if the address is not on the SteamGridDB image server; `network`
/// on a network error.
#[tauri::command]
pub async fn art_download(app: AppHandle, url: String) -> Result<String, CommandError> {
    blocking(app, move |services| services.art.download(&url))
        .await
        .map(|image| to_url(&image))
}

fn api_key(services: &Services) -> Result<String, CommandError> {
    services
        .preferences()
        .steam_grid_db_key
        .clone()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            AdapterError::Unauthorized("no SteamGridDB API key in settings".to_owned()).into()
        })
}

fn to_url(image: &ImageFile) -> String {
    data_url(image.mime, &image.bytes)
}

async fn blocking<T, F>(app: AppHandle, work: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&Services) -> Result<T, AdapterError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<Services>()))
        .await
        .map_err(|error| CommandError::from(AdapterError::Io(error.to_string())))?
        .map_err(CommandError::from)
}
