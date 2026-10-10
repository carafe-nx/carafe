---
name: release-notes
description: Release notes for a new Carafe version on GitHub. Use when preparing a release, its description or changelog, or choosing the next version tag.
---

# Release notes

A Carafe release is a draft that `.github/workflows/release.yml` creates from a tag; the notes are pasted into
that draft by hand. Notes speak to players, not developers: what changed for them and what they have to do.

## Steps

1. **Collect.** Take the last tag (`git describe --tags --abbrev=0`) and the commits after it
   (`git log <tag>..HEAD --oneline`), with their diffs where the subject alone does not say what a player sees.
   Done when every commit is either placed in a section or set aside as invisible to players
   (`ci`, `chore`, `docs`, `build`, refactoring, tests).
2. **Version.** Only fixes → bump the patch (`v0.1.0` → `v0.1.1`); anything new → bump the minor.
   The user's tag wins when given.
3. **Write** the notes in the format below, in English. Each item is one sentence on what changed for the
   player and, when it helps, one on how it was before. Link the issue or discussion that reported it;
   ask the user when the commits do not name one.
4. **Runtime check.** If any placed commit touches `runtime/` or `external/`, the change lives inside each
   game's NSP: end that item with "Rebuild games made with <previous version> to get the fix."
5. **Hand over.** Put the body on the Windows clipboard with `Set-Clipboard` and a single-quoted here-string.
   Show the user the title, the body and a Russian translation of both; the title goes into its own field
   of the release form, so it stays out of the clipboard.

## Format

Title: `Carafe <version>` (no `v`).

```markdown
<One line: "A bug-fix release." for fixes only, otherwise what the release is about.>

## New

- **<Short name>.** <What the player can do now.> ([#<n>](<link>))

## Fixed

- **<Short name>.** <What works now.> <How it was before.> ([#<n>](<link>))

Tell others how a game ran in [Compatibility](https://github.com/carafe-nx/carafe/discussions/categories/compatibility), and report bugs in [Issues](https://github.com/carafe-nx/carafe/issues).

**Full Changelog**: https://github.com/carafe-nx/carafe/compare/<previous tag>...<new tag>
```

- A section with no items is left out.
- Requirements, download files and installer warnings live in the first release's notes; a later release
  repeats them only when they change.
- Console setup is described through DBI and custom firmware alone; key files stay out of public texts.

## Publishing

The user runs **Actions → Release → Run workflow** on `main` with the tag, then pastes the notes into the
draft and publishes it. Offer these steps; the run itself is theirs.
