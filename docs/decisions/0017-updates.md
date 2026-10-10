# 17. Carafe updates itself from GitHub releases

Date: 2026-10-10

Replaces the MSI installers of [0016](0016-releases-and-licenses.md), "the runtime version is the app version" in
[0010](0010-library-and-carafe-json.md) and [0016](0016-releases-and-licenses.md), and the "reqwest" line of
[0013](0013-desktop-app.md). Research: [research/updates/REPORT.md](../research/updates/REPORT.md).

## Context

Players install Carafe by hand from GitHub and learn about new versions only by going back there. Fixes in the runtime
reach a game only after it is rebuilt, and today every new Carafe marks every game as outdated, because the runtime
version is the app version. Installing an update closes the app, and Carafe can be in the middle of a build or a USB
install.

## Decision

- **Installer.** NSIS for the current user, one per architecture with English and Russian, installed to
  `%LOCALAPPDATA%\Carafe`. Updates need no administrator rights. The first NSIS release is installed by hand; its
  installer removes the old MSI install (one UAC prompt). The product name `Carafe` and the publisher `carafe` stay as
  they are: the NSIS installer finds the MSI by them.
- **Updater.** `tauri-plugin-updater` with `default-features = false` and `native-tls`, `system-proxy`: schannel is
  already in the app through ureq, and ring would need clang on the ARM64 runner. The flow lives in Rust
  (`carafe-desktop`); the window talks to it through Carafe's own commands and `update://…` events and gets no
  `updater:*` permission.
- **Signing.** Every installer is signed with a minisign key (`tauri signer generate`). The public key is in
  `tauri.conf.json`; the private key and its password are secrets of the `release` environment on GitHub and stay with
  the owner. `requireSignedVersion` is on: a signature must name the version it was made for. Updater artifacts are
  made only by the release config, so local builds need no key.
- **Manifest.** The release job writes `latest.json` once, after both architectures are built, with the final download
  URLs and both keys per architecture (`windows-x86_64`, `windows-x86_64-nsis`, the same for `aarch64`) and puts it
  into the draft. Carafe reads `releases/latest/download/latest.json`, so drafts and prereleases are never offered.
  `latest.json` also carries the release's runtime version.
- **Runtime version.** The runtime gets a version of its own: the oldest release tag whose runtime inputs (`runtime/`,
  `external/`, `docker/`, `compose.yaml`, `assets/switch/`) are identical to the commit being released, or the new tag
  if they changed. The release workflow works it out and passes it to the build. A game is outdated when its runtime
  version is older than Carafe's.
- **When.** The first check is a minute after start, then every 12 hours while Carafe is open. A failed automatic check
  stays silent and is retried next time.
- **Modes** in the settings: automatic (default) — download in the background, install on request; notify only —
  show that a version is out, download on request; off — check only on request. Checking talks to GitHub only.
- **What the player sees.** A button in the library's top bar, only when there is news: "available", "downloading",
  "restart to update", "failed". Its popover shows the version and, if the runtime changed, that games are worth
  rebuilding; "What's new" opens the release page. Nothing opens on its own.
- **Installing** happens only when the player asks for it: now, or when Carafe closes. While a build or a USB install
  is running, installing is refused; the player can ask to install when it finishes. After the update Carafe shows once
  that it was updated.
- **Failures.** Before installing Carafe notes the version it is moving to; if it starts again on the old version, the
  button says the update did not finish. Old installer copies in `%TEMP%` are removed at start.
- **Rebuilding games.** When games are outdated and the runtime has changed since the player last dismissed it, the
  library offers to rebuild them. The player picks games; they are built one by one with the same settings, Title ID
  and icon; the old NSP is replaced only after a successful build. The free space check covers the largest game.

## Rejected

- Keeping the per-machine MSI: a UAC prompt on every update, and standard users cannot update at all.
- A per-user MSI through a forked WiX template: the template has to follow Tauri by hand, and MSI cannot move an
  existing per-machine install to per-user.
- The bundler's elevated update task: the v2 updater does not use it, and it runs any MSI in `%TEMP%` as administrator.
- The updater's JavaScript API: the window would hold the permission to install.
- An updater of Carafe's own on ureq: the signature check, the installer launch and the platform keys would all be
  repeated by hand.
- Release notes inside the app: they are written in English after the draft is made, and would need the GitHub API and
  a Markdown renderer.
- Downloading the runtime separately to keep updates small: a second download to keep in step, no offline install,
  and releases are rare.
- Installing silently on close: the version changes unnoticed, and a failure on close is seen by no one.
- tauri-action: one job writes the manifest after both architectures are built, without merging and retries.
