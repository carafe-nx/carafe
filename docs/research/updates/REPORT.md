# Carafe updates itself

How a Tauri 2 app on Windows can update itself from GitHub releases, what it costs with Carafe's installer, and what
calm update UX looks like in other apps. The analysis was done on 2026-10-10 by reading the sources listed below; no
experiments yet.

**Outcome.** [Decision 0017](../../decisions/0017-updates.md): a per-user NSIS installer, `tauri-plugin-updater`
driven from Rust, a signed `latest.json` in each release, a runtime version of its own and a quiet update button in the
library.

**Versions behind every reference:** tauri `dev` @ `eeeaa0f`, tauri-plugin-updater `v2` @ `818d2e0` (released as
2.13.2), tauri-action `dev` @ `a6e90dd`. Carafe at the time: tauri 2.11.6, `@tauri-apps/cli` ^2.12.1, MSI only.

## 1. How the updater works on Windows

Read in `plugins/updater/src/updater.rs` @ `818d2e0`.

- **Check** (L535-699): endpoints are tried in order, HTTP 204 means "no update", an update is offered when the
  announced version is greater than the running one (semver).
- **Download** (L777-846): one `GET` into a `Vec<u8>` in memory, with `on_chunk(len, content_length)`. No resume, no
  `Range`, no delta updates. No timeout by default; a request timeout in reqwest covers the whole body.
- **Verify** (L1797-1863): minisign over the whole file, then the version in the signature's trusted comment is compared
  with the one announced. `requireSignedVersion` makes a signature without a version an error.
- **Install** (L938-1123): the type is detected by content (`.exe` → NSIS, MSI → `msiexec`); the file is written to
  `%TEMP%\<product>-<version>-updater-XXXXXX\`, `on_before_exit` runs, the installer is started with `ShellExecuteW`
  and the app calls `std::process::exit(0)`. The temp folder is kept (`.keep()`), so each update leaves a copy of the
  installer behind (open issue tauri-apps/plugins-workspace#2132).
- **Arguments.** MSI: `/i <file> /passive /promptrestart AUTOLAUNCHAPP=True`. NSIS: `/P /UPDATE` plus `/R` to relaunch
  (installer.nsi L745-755, `RunAsUser`). `restart_after_install(false)` (2.11.0+) drops the relaunch.
- **Errors.** Everything up to `ShellExecuteW` returns `Err` and the app keeps running. Anything after it (UAC
  cancelled, installer blocked) happens with the app already gone; in passive mode nobody sees it.
- **Platform key.** `windows-<arch>-<bundle>`, then `windows-<arch>` (L701-731). `<bundle>` is `msi` or `nsis`, read
  from a marker the bundler patches into the exe (tauri `bundle.rs` L40-60).
- **Rust or JS.** The whole flow is available from Rust: `app.updater_builder().build()?.check()`,
  `Update::download`, `Update::install`. The window then needs no `updater:*` permission. No CSP change: requests go
  from Rust.
- **Dependencies.** 2.13.2 needs tauri ^2.12. Default features pull rustls with ring; ring on `aarch64-pc-windows-msvc`
  needs clang and the ARM64 build tools. With `default-features = false` and `native-tls` it uses schannel, which
  Carafe already has through ureq. reqwest 0.13 is already in Carafe's lockfile, pulled by tauri.

## 2. MSI or NSIS

- **Tauri's MSI is per-machine** (`main.wxs` L28 `InstallScope="perMachine"`, Program Files). There is no option for a
  per-user MSI (tauri-apps/tauri#13792, open); only a forked WiX template.
- **So every MSI update needs elevation.** An admin gets a consent prompt; for an unsigned package it is the yellow
  "unknown publisher" one. A standard user needs an admin password every time. Windows Installer docs: "Using Windows
  Installer with UAC". `quiet` mode cannot elevate at all.
- **Ways round it don't fit.** The bundler's `enableElevatedUpdateTask` is not used by the v2 updater
  (plugins-workspace#3000) and would let any user process run an MSI from `%TEMP%` as admin. UAC patching works only
  with signed `.msp` patches.
- **MSI scope cannot change between versions.** Windows Installer does not do major upgrades across installation
  context (Microsoft Learn, "Major Upgrades"); a per-user MSI after a per-machine one installs side by side.
- **NSIS `currentUser`** (Tauri's default): `RequestExecutionLevel user`, installs to `%LOCALAPPDATA%\<product>`, no
  UAC on install or update. One installer per architecture holds several languages (`languages: ["English",
  "Russian"]`); WiX builds one MSI per language.
- **NSIS removes an old MSI install itself** (installer.nsi L189-216, L314-317): it looks in
  `HKLM\…\Uninstall` for an entry with the same `DisplayName` and `Publisher` and an `msiexec` uninstall string, then
  runs it. The user sees the MSI uninstall and one UAC prompt. This only works while the product name and the
  publisher (`carafe`, taken from the identifier) stay the same. It runs in the installer pages, so not with `/S`.
- **ARM64.** The NSIS installer itself is x86 and runs under emulation; the installed app is native ARM64 (Tauri docs,
  "Windows Installer").
- **Costs of per-user.** AppLocker default rules let standard users run programs only from Windows and Program Files;
  each Windows user installs their own copy. Unsigned NSIS installers attract more antivirus false positives
  (tauri-apps/tauri#11673).
- **Who ships what** (release assets, 2026-10-10): NSIS — Yaak, Clash Verge Rev, Jan, Cap; MSI — GitButler (EV-signed),
  Hoppscotch. Electron's Squirrel and electron-builder NSIS install per user for the same reason: no UAC on updates.

## 3. SmartScreen, Defender, Smart App Control

- The updater writes the installer with plain file I/O: no Mark-of-the-Web, so SmartScreen does not check it
  (Microsoft Learn, "SmartScreen reputation"; `IAttachmentExecute`). Not verified on a machine.
- Smart App Control checks every executable regardless of where it came from; in enforcement mode it blocks unsigned
  code. This already applies to Carafe today; only code signing fixes it.
- Defender scans the file on write and on run wherever it is; the download folder makes no difference, and the
  updater always uses `std::env::temp_dir()`. The runtime is inside Carafe's own archive format, so its DLLs are not
  visible to the scanner as separate files. Not verified on a machine.

## 4. The manifest and GitHub releases

- **Format** (updater.rs L1727-1773): `version`, `notes`, `pub_date` (RFC 3339), `platforms` with `url` and
  `signature` (the text of the `.sig` file). One broken entry fails the whole file for everyone. Extra fields are
  readable through `Update::raw_json`.
- **Endpoint** `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`: GitHub's "latest" is the
  newest release that is neither a draft nor a prerelease (GitHub REST docs, "Get the latest release"). It is a web
  redirect, not an API call.
- **Immutable releases.** Assets cannot change after publishing, so `latest.json` goes into the draft with the final
  download URLs. Notes in it are frozen at that moment; the release body can still be edited.
- **tauri-action** merges `latest.json` job by job, deleting and re-uploading it, with retries
  (`upload-version-json.ts`). With both architectures built before one release job, the file can be written once.
- **Signing.** `tauri signer generate` makes a minisign key pair; CI reads `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. With `createUpdaterArtifacts: true` a build without the private key fails, so
  the setting belongs in the release config only. Since CLI 2.12.0 the trusted comment carries the version. Losing the
  key means installed copies refuse every later update.
- **Rollback.** Marking the previous release as latest stops new installs of a bad version; copies already on it stay
  there, because only newer versions are offered.

## 5. Update UX in other apps

Source code read for the Tauri apps, Zed and the emulators; mainstream apps from their docs.

- **Tauri apps.** GitButler: a card in the corner, check every hour, download after a click. Yaak: check on window
  focus at most every 12 h, optional background download, "What's New" links to the web. Cap: never installs while
  recording or exporting (`with_idle_app`), retries after 5 minutes. Hoppscotch: one state union
  `idle | checking | available | downloading | installing | ready_to_restart | error` driven from Rust.
- **Zed**: downloads silently, then one title-bar button "Restart to Update"; automatic checks that fail stay silent;
  "Updated to X" with release notes once after the update.
- **VS Code, Slack, Chrome**: a badge or a pill, "Restart to update", no modal.
- **Emulators.** Dolphin: "Update after closing", "Never Auto-Update", "Remind Me Later". PCSX2: never shows the
  dialog while a game runs; data-impact warnings first. RPCS3: a "Background" mode with a download button in the
  corner; "Please stop the emulation before trying to update". Ryujinx: off / prompt / background, with Russian strings.
- **Complaints.** Daily popups, a UAC prompt on every update, forced restarts, a broken install with no way back
  (microsoft/vscode#99787, #336131), updates despite "manual" (microsoft/vscode#82433).
- **Microsoft's notification guidance** (Win32 UX guide, "Notifications"): an optional task such as an update is shown
  at most once a day, three times in total; users immersed in work should not see notifications at all.
- **Rebuilding data after an update.** RPCS3 batch cache creation with progress and Cancel; RetroArch "Update Installed
  Cores"; PCSX2 warns about save-state impact before the update. None rebuilds on its own.

## 6. What this means for Carafe

- Per-machine MSI makes every update a UAC prompt; NSIS per user removes it. No released Carafe has an updater, so the
  first release with one is installed by hand anyway and can switch the installer at the same time.
- Installing quits the app, so it must never happen during a build or a USB install; Carafe has no busy state today.
- The runtime version equals the app version today, so every update would mark every game as outdated. A runtime
  version of its own keeps the rebuild offer honest.
- Each update is about 230 MB, mostly the runtime archive. Releases are rare; splitting the runtime out was not worth
  its cost.
