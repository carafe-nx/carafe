# External dependencies

Everything in this folder is third-party code. Each project is a git submodule pinned to an exact
commit and is used without changes. Carafe's own changes to Autorun live in `runtime/patches/autorun/`
as separate patches.

| Folder | Upstream | Pinned commit | Version | License |
| --- | --- | --- | --- | --- |
| `autorun/` | [autorunhq/autorun](https://github.com/autorunhq/autorun) | `636913ca46a3638018b3ea56995d6852db9f0165` | — | LGPL-2.1 |
| `hacbrewpack/` | [rlaphoenix/hacBrewPack](https://github.com/rlaphoenix/hacBrewPack) | `745b16ecfc9ce055743067d200572204cb2aac6c` | v3.05 | GPL-2.0 |
| `nx-hbloader/` | [switchbrew/nx-hbloader](https://github.com/switchbrew/nx-hbloader) | `82b95122c5ae8dc059bf23893ba7623c72c86773` | — | ISC |

The original hacBrewPack repository by The-4n is no longer available; `rlaphoenix/hacBrewPack` is a
mirror of the same history.

## Pulled in by Autorun

These are pinned by Autorun itself, not by Carafe:

- `horizon-dlls`: Autorun's own submodule ([autorunhq/autorun-horizon-dlls](https://github.com/autorunhq/autorun-horizon-dlls)),
  commit `5d6eccb82c266f037dd4305037c986b6303cd59a` at the Autorun commit above;
- Box64 and FFmpeg: fetched by Autorun's `horizon-wine/tools/bootstrap-*.sh` scripts at the commits
  written in those scripts.

## Pinned by Carafe's build

These are not submodules: the runtime build fetches them itself.

- FEX: [FEX-Emu/FEX](https://github.com/FEX-Emu/FEX) at `395b132f346b1a45def246d10c52245edba1ef02` (FEX-2609),
  in `runtime/fex/build-fex.sh`, with the patches in `runtime/patches/fex/`;
- DXVK: [doitsujin/dxvk](https://github.com/doitsujin/dxvk) at `b1a1c99ab52b687cf950d62c88bc2fa316b41663`, in
  `runtime/dxvk/build-dxvk.sh`, with the patches in `runtime/patches/dxvk/`;
- Wine Mono 11.3.0, the arm64 release archive, in `runtime/Makefile` (`MONO_VERSION`).

## Fetching

Do not run `git submodule update` with Git for Windows: it rewrites line endings and the build breaks.
The build scripts fetch the submodules inside the `switch-dev` container.

## Updating

The pinned commit is stored by git, not in `.gitmodules`. `git submodule status` prints it.

1. Check out the new commit inside the submodule.
2. Rebuild and test the runtime on a console.
3. Commit the submodule change together with any patch updates.
4. Update the table above.
