//! Searches SteamGridDB for a game and its square covers — for checking the key and the network.
//!
//! ```text
//! cargo run -p carafe-steamgriddb --example search -- <API key> <title>
//! ```

use std::process::ExitCode;

use carafe_core::ports::ArtSearch;
use carafe_steamgriddb::SteamGridDb;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [key, term] = args.as_slice() else {
        eprintln!("usage: search <api key> <game name>");
        return ExitCode::FAILURE;
    };
    let client = SteamGridDb::new();
    let games = match client.search_games(key, term) {
        Ok(games) => games,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for game in &games {
        println!("{} {} {:?}", game.id, game.name, game.year);
    }
    let Some(first) = games.first() else {
        return ExitCode::SUCCESS;
    };
    match client.square_images(key, first.id) {
        Ok(images) => {
            for image in images.iter().take(5) {
                println!(
                    "{} {:?}x{:?} {}",
                    image.id, image.width, image.height, image.url
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
