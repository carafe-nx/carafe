# Roadmap

What's planned and what's done, without dates. Open items are in priority order; bold ones are in progress.

- `bug` — Carafe itself is broken
- `game` — a specific game misbehaves
- `feature` — something new
- `research` — a question without an answer yet

## Planned

- [ ] `bug` Saves and game settings are lost after a restart when the game keeps a file open the whole session
- [ ] `game` OpenTTD: rare start-up hang is worked around, but its root cause is still unknown
- [ ] `feature` Buttons mapped by their labels, as on the Switch (A → A), with a choice to map them by position
- [ ] `feature` Back up and restore game saves over USB
- [ ] `research` Smaller save data on the console: measure what games really write
- [ ] `feature` Move saves from the PC version of a game to the Switch
- [ ] `research` 64-bit games that draw with OpenGL hang
- [ ] `research` Faster game start-up
- [ ] `bug` The logo shifts slightly between the boot splash and the loading screen
- [ ] `feature` Sort and search in the library
- [ ] `feature` Import and export Autorun settings files
- [ ] `feature` Game name and publisher in several languages
- [ ] `feature` Signed installer, without the unknown publisher warning
- [ ] `feature` Releases for macOS and Linux
- [ ] `research` Steam client inside the NSP for games that need Steam
- [ ] `research` Proton on Horizon OS

<details>
<summary>Done</summary>

- [x] `feature` Carafe updates itself from GitHub releases and offers to rebuild games made with an older runtime — v0.2.0
- [x] `feature` Launch bitness in the build wizard: Auto from the chosen `.exe`, or 32/64 bit by hand under Advanced
- [x] `bug` Blurry taskbar icon at high display scaling
- [x] `bug` The mouse cursor doesn't move with the stick — v0.1.1
- [x] `feature` First public release: Windows installers for x64 and ARM64, in English and Russian — v0.1.0
- [x] `feature` About section in the settings, with the version and the license
- [x] `feature` The loading screen stays until the game's first frame, without black pauses
- [x] `feature` The boot splash and the loading screen share one look
- [x] `game` Need for Speed Underground: saves survive a restart
- [x] `feature` Carafe's own logo, app icons and boot splash
- [x] `feature` Loading screen with the game's icon and start-up stages
- [x] `feature` Install to the Switch over USB through DBI, and fetch the game's logs back
- [x] `feature` The runtime ships as an archive: 223 MB instead of 1.1 GB
- [x] `bug` Library folders and game folders with non-Latin letters in the path
- [x] `feature` Paths that are too long are caught before the build, not halfway through it
- [x] `feature` Free space is checked before the build; leftovers of a failed build are cleaned up
- [x] `game` Need for Speed Underground runs: games that need a fixed load address
- [x] `feature` Library of built games, read from the NSP files in its folder
- [x] `feature` The game folder is read on its own: `.exe`, 32 or 64 bit, name and publisher
- [x] `feature` Game icon from the `.exe`, from your own image or from SteamGridDB
- [x] `feature` Controls tab with a Switch controller diagram
- [x] `feature` NSP built right in the app, with hacBrewPack for Windows x64 and ARM64
- [x] `feature` The app: first-run setup, library, six-step build wizard, English and Russian, light and dark theme
- [x] `game` OpenTTD reaches the main menu every time: start-up hangs are worked around
- [x] `game` OpenTTD: Quit returns to the HOME Menu without an error
- [x] `feature` The game starts straight away, without the Autorun launcher
- [x] `feature` The runtime runs from the NSP alone, with no files on the SD card
- [x] `game` OpenTTD: the first Windows game started from a standalone NSP
- [x] `research` Can a Windows runtime live inside an NSP? Yes: JIT works in application mode

</details>

Missing something? Suggest it in [Ideas](https://github.com/carafe-nx/carafe/discussions/categories/ideas)
or report a bug in [Issues](https://github.com/carafe-nx/carafe/issues).
