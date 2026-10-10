---
name: feature-kickoff
description: Starting a new Carafe feature with deep research and live mockups before any code. Use when the user asks to begin a ROADMAP item, to research how a feature is usually done, or to show how it will look.
---

# Feature kickoff

A feature starts as a **picture the user can click**, not as code. The user decides the product questions
looking at live mockups in Carafe's own look; the code follows the decisions written into `docs/`.

## Steps

1. **Ground.** Read the ROADMAP item, the decisions and research in `docs/` that touch it, and the code it
   will change. Done when you can name every file the feature touches, what each one does today and what
   the feature would change for the player.
2. **Size it.** Tell the user in a few lines what the feature changes for the player and how big it is.
   - **Large or risky** — new dependency, secrets, installer, data format, anything that can break every
     user — take every step below.
   - **Small** — one control or one screen — skip step 3, mock up only the one screen in step 4, and record
     the decision in step 6 only when it has a reason worth keeping.
   - **Changes nothing for the player** — say so and propose what would help instead, before building it.
3. **Research in parallel.** In one message, launch background agents, one per independent question:
   - a codebase survey — the end-to-end data flow, exact `file:line` places to change, what breaks on a
     wrong value;
   - how real apps solve it — Wine front-ends, launchers, Tauri apps — with links, controls and wording;
   - platform facts — Windows, Wine/Autorun, Horizon OS — when the feature depends on them.
   Done when every agent has reported and each finding is either used or set aside with a reason.
4. **Mock up.** Run `Artifact` quickstart, load `artifact-design`, take colours, fonts and spacing from the
   app's theme tokens in `app/ui`, and build one page: the real screen of Carafe with the feature in it,
   every state clickable (default, changed, warning, error), each variant side by side, plus a short
   "how it works" part in plain words. The page is in Russian. Done when every product question in step 5
   has its variants visible on the page.
5. **Decide with the user.** Publish the page, then ask the product questions through `AskUserQuestion`,
   recommended option first, each option described by what the player gets. Technical internals are
   decided by you from the research, not asked.
6. **Record.** Write `docs/research/<topic>/REPORT.md` (findings with sources) and a new numbered
   `docs/decisions/` file (what was chosen and why), in English; link them from `docs/README.md`; make the
   ROADMAP item bold.
7. **Build.** Implement by the decision. Every new rule in the Rust crates gets a test beside it, including
   the wrong-input and edge cases found in step 3.
8. **Test.** Done when all three pass, each shown to the user:
   - `npm run check` from `app/` is green;
   - the feature works in the running app, every state from the mockup, shown with screenshots;
   - behaviour that lives only on the Switch is checked on the console in one batched run, with the exact
     steps and what to look for handed to the user beforehand.
