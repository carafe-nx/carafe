//! Rebuilding games made with a runtime older than Carafe's.

use serde::Serialize;
use ts_rs::TS;

use crate::disk_space::required_space;
use crate::library::GameSummary;
use crate::title_id::TitleId;

/// The library's offer to rebuild games made with an older runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebuildOffer {
    /// The runtime version in Carafe.
    pub runtime_version: String,
    /// Games made with an older runtime, in library order.
    pub games: Vec<TitleId>,
    /// The player chose not to be offered a rebuild for this runtime.
    pub dismissed: bool,
}

/// Returns the offer to rebuild the library's outdated games; `dismissed` is the runtime version for
/// which the player last chose "not now".
#[must_use]
pub fn rebuild_offer(
    games: &[GameSummary],
    current_runtime: &str,
    dismissed: Option<&str>,
) -> RebuildOffer {
    RebuildOffer {
        runtime_version: current_runtime.to_owned(),
        games: games
            .iter()
            .filter(|game| game.runtime_outdated)
            .map(|game| game.title_id)
            .collect(),
        dismissed: dismissed == Some(current_runtime),
    }
}

/// Returns how much free disk space rebuilding NSPs of `sizes` bytes needs.
///
/// The games are built one after another, so the largest one decides. Zero for no games.
#[must_use]
pub fn rebuild_space(sizes: &[u64]) -> u64 {
    sizes
        .iter()
        .max()
        .map_or(0, |&largest| required_space(largest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Arch;

    fn summary(entropy: u64, outdated: bool) -> GameSummary {
        GameSummary {
            title_id: TitleId::from_entropy(entropy),
            title: format!("Game {entropy}"),
            publisher: String::new(),
            version: "1.1".to_owned(),
            arch: Arch::X86,
            size_bytes: 1,
            icon: None,
            runtime_version: "0.1.0".to_owned(),
            runtime_outdated: outdated,
        }
    }

    #[test]
    fn offers_only_outdated_games() {
        let games = [summary(1, true), summary(2, false), summary(3, true)];
        let offer = rebuild_offer(&games, "0.2.0", None);
        assert_eq!(offer.games, [games[0].title_id, games[2].title_id]);
        assert!(!offer.dismissed);
    }

    #[test]
    fn dismissed_for_the_current_runtime() {
        let offer = rebuild_offer(&[summary(1, true)], "0.2.0", Some("0.2.0"));
        assert!(offer.dismissed);
    }

    #[test]
    fn a_newer_runtime_offers_again() {
        let offer = rebuild_offer(&[summary(1, true)], "0.3.0", Some("0.2.0"));
        assert!(!offer.dismissed);
    }

    #[test]
    fn the_largest_game_decides_the_space() {
        let gib = 1 << 30;
        assert_eq!(
            rebuild_space(&[gib, 3 * gib, 2 * gib]),
            required_space(3 * gib)
        );
    }

    #[test]
    fn no_games_need_no_space() {
        assert_eq!(rebuild_space(&[]), 0);
    }
}
