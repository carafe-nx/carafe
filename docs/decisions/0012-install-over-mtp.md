# 12. Install and fetch logs over MTP with DBI

Date: 2026-10-05 – 2026-10-07

## Context

A built NSP has to reach the console, and a game's logs have to come back for bug reports.

## Decision

- When a Switch running DBI in MTP mode is connected, the game card offers **To the SD card** and **To console memory**:
  the NSP is copied into DBI's `SD Card install` or `NAND install` storage, with progress in the card.
- **Fetch logs** copies `switch/carafe/<Title ID>/` from the card to `<library>\.carafe\logs\<Title ID>\`, replacing the
  previous copy, and opens the folder. Wine's 2 MB `.shm` session file is skipped.
- On Windows this is Windows Portable Devices through the `windows` crate (`carafe-mtp`); all `unsafe` code lives in
  `wpd.rs`. The Switch is recognised by Nintendo's vendor id `VID_057E`.

## Rejected

- DBI's USB backend protocol: needs a libusb or WinUSB driver.
- Copying through `Shell.Application`: no progress, Explorer windows pop up.
- Logs in a zip: logs are read by eye.
