# Roadmap

What's planned and what's done, without dates. Open items are in priority order; bold ones are in progress.

- `bug` — Carafe itself is broken
- `game` — a specific game misbehaves
- `feature` — something new
- `research` — a question without an answer yet

## Planned

- [ ] **`feature` The runtime's Direct3D is the system Direct3D: DXVK and dxwrapper in the system folders, no folder in front of system32**
- [ ] `bug` Wine's d3dx9 disassembles the address register as `a` instead of `a0`, so d3d8to9 cannot rebuild skinned shaders (#64)
- [ ] **`game` OpenTTD: start-up stall of 20–110 s while threads wait for a lock**
- [ ] `bug` The + of the menu combination reaches the game: NFSU skips its intro
- [ ] `research` FEX: many threads ending at once wait on its compile locks for tens of seconds
- [ ] `research` FEX: an exception inside block compiling loops in ntdll's unwinder and holds the compile lock
- [ ] **`research` Faster games at the stock CPU clock: FEX tuned for the A57, x87, DXVK calls**
- [ ] **`research` Orchestrator: one part of the runtime that makes translated games use every resource of the Switch**
- [ ] **`feature` One path for every game: Carafe's DXVK, FEX and D3DX, no choice of components**
- [ ] `research` Games limited by the Switch's GPU: less GPU work in DXVK, upscaling
- [ ] `research` Keep translated code between runs and prepare it at the first start
- [ ] `feature` Offer the lock-wait fix to Autorun
- [ ] `feature` Warn in the build wizard when the game folder has mods or fixes (proxy DLLs such as `dinput8.dll`, `.asi`)
- [ ] `feature` A game missing a DLL names it on the console, and the build wizard lets the user add it
- [ ] `feature` Message boxes answered with the controller's buttons, not only by touch
- [ ] `game` Dishonored: stays on the loading screen while GOG Galaxy polls a socket
- [ ] `game` Indiana Jones and the Emperor's Tomb: a green frame before the title screen
- [ ] `game` Rig and Roll: check driving in 3D; the installed copy asks for its CD
- [ ] **`bug` The controller is no HID device: DirectInput 8 joysticks and Windows.Gaming.Input see no pad (Indiana Jones, Megabonk, Cat Mail Co)**
- [ ] `game` Cat Mail Co and Fields of Mistria: the game's memory runs out (Cat Mail Co when a new game loads)
- [ ] `game` Indiana Jones and the Emperor's Tomb: reads the pad, but its title screen answers only its keyboard bindings
- [ ] `research` A 64-bit program that draws only with GDI never hides the loading screen
- [ ] `research` Saves: confirm whether they can still be lost on HOME and sleep, and in games that keep a file open the whole session
- [ ] `bug` Mouse wheel mapped to a button does nothing in the game
- [ ] `research` Dead Space: rare crash while loading a level when the runtime cannot place game memory
- [ ] `research` NFSU2: rare crash at the start of a race in the game's own sound code, two threads racing
- [ ] `game` Undertale: can stop at start-up with every thread waiting; cause not found
- [ ] `game` NFSU2: frame rate drops to 30 for a few seconds in races, CPU and GPU both idle
- [ ] `game` Heroes of Might and Magic IV: rare crash in the Bink video player (`binkw32.dll`) during the intro
- [ ] `feature` A choice to map controller buttons by position instead of by their labels
- [ ] `feature` Back up and restore game saves over USB
- [ ] `research` Smaller save data on the console: measure what games really write
- [ ] `feature` Move saves from the PC version of a game to the Switch
- [ ] `research` 64-bit games that draw with OpenGL hang
- [ ] `research` Frame log with stutter causes: shaders, JIT, file reads, sync
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

- [x] `bug` Heroes of Might and Magic IV starts through system DirectDraw: fullscreen reset preserves the game's window resize
- [x] `research` FEX's TSO emulation slows NFSU2 on the console, so it stays off by default
- [x] `bug` A thread terminated in a lock wait no longer breaks other threads' waits later
- [x] `bug` A thread terminated while it waits in the server ends at once, not 250 ms later
- [x] `bug` 32-bit games no longer lose a wakeup between threads: an x86 locked instruction is a full barrier
- [x] `bug` Suspending a thread works as on Windows, 64-bit and 32-bit: garbage collectors no longer hang a game
- [x] `game` Cat Mail Co starts with the default settings and closes from the HOME Menu
- [x] `bug` Videos drawn with GDI show over a Direct3D game, and the loading screen leaves at their first picture
- [x] `bug` Programs start with Explorer's window hint: a window is no longer maximized and then restored off screen
- [x] `game` Indiana Jones and the Emperor's Tomb: no hang when its video thread opens DirectDraw, quick start
- [x] `feature` DirectDraw and Direct3D 1–7 games draw through dxwrapper and DXVK
- [x] `bug` Logs keep their last lines when a game is closed from the HOME Menu and note when the game was held still
- [x] `bug` The cursor stays on the program's screen: a click no longer lands on the window frame over the black bars
- [x] `bug` Games on DirectInput 7 and older read the mouse and keyboard
- [x] `feature` Videos played through DirectShow and Windows Media (ASF, AVI, MPEG) are decoded with FFmpeg
- [x] `bug` A game no longer hangs when a thread of DirectShow reads its messages while the game waits for it
- [x] `feature` Window hooks built in: MFC games start
- [x] `bug` Texts of programs without Unicode show in the console's language
- [x] `bug` A dialog shown at start-up keeps its own size, in the middle of the screen
- [x] `feature` DXVK-Sarek's settings on a page of their own
- [x] `feature` GOG games get the registry values of their install script, written again when missing
- [x] `bug` Message boxes and dialogs show their text, icon and buttons
- [x] `bug` The loading screen steps aside when a program shows a dialog before its first picture
- [x] `feature` Direct3D 8 games run through DXVK
- [x] `feature` 32-bit Direct3D 10 and 11 games run through DXVK
- [x] `feature` Games that use .NET get Wine Mono
- [x] `bug` Games find the Windows Runtime classes Wine has
- [x] `feature` Experimental: window hooks, waking suspended threads, single-precision x87
- [x] `feature` Logs name the first exceptions, DLLs without their Unix part and the first controller press
- [x] `bug` No hitches while shaders build: DXVK builds them in the background, as DXVK-Sarek does
- [x] `bug` DXVK settings now reach 32-bit games
- [x] `bug` A game that makes a probe Direct3D device on its first start no longer stays on the loading screen
- [x] `bug` Verbose traces no longer make the start take minutes
- [x] `feature` Faster x87 math, CPU flags and repeated D3D9 states built into the runtime
- [x] `feature` Game settings on the console: a menu over the game and on the error screen — v0.4.0
- [x] `feature` Game registry: entries from a .reg file in the app, the whole registry in the console menu — v0.4.0
- [x] `feature` The app and the console screens in every Switch system language — v0.4.0
- [x] `feature` The default controller layout keeps the button labels, as on the Switch (A → A) — v0.4.0
- [x] `bug` 32-bit games no longer run out of address space while FEX compiles on many threads at once (#47) — v0.4.0
- [x] `bug` 32-bit games get addresses up to 4 GB instead of 2 GB (#47) — v0.3.1
- [x] `bug` Saves keep the time they were changed, so games that check it load them (#47) — v0.3.1
- [x] `feature` When a game fails, an error screen shows what failed and offers to restart it or exit — v0.3.0
- [x] `bug` Games get about 80 of the Switch's 96 threads and wait at the limit instead of closing (#31) — v0.3.0
- [x] `bug` NSP builds 2–3 times faster: hacBrewPack copied the game's files a few bytes at a time (#37)
- [x] `bug` A game at another resolution, such as 4:3, in a window or full screen, is fitted and centred with black bars
- [x] `bug` The loading screen stays visible around a game whose picture doesn't fill the screen
- [x] `bug` Sound in games that play audio below 48 kHz (#14)
- [x] `bug` The build banner in the library opens the running build's screen
- [x] `bug` One build at a time, cancellable, with its progress and result in a banner on the library screen
- [x] `bug` Build wizard shows a loader while the folder or the cover loads and waits for it; the cover is downloaded once
- [x] `bug` Saves and game settings survive a restart: committed while files are open, and on closing from the HOME menu
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
