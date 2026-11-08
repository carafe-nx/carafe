# Carafe agent instructions

These instructions apply to the whole repository. For work under `app/`, also read
[app/AGENTS.md](app/AGENTS.md).

Before making changes, read [docs/development.md](docs/development.md) through the repository's
layout, building and rules sections. They define the runtime, container builds, third-party patches,
line endings and repository conventions.

When changing documentation, read [docs/README.md](docs/README.md), including "Keeping it".
Read the relevant decisions and research before changing the behavior they describe.

Project skills are maintained in [.agents/skills/](.agents/skills/):

- `feature-kickoff` applies when starting a feature, researching a ROADMAP item or preparing its mockup.
- `release-notes` applies when preparing a release, its notes or its next version tag.

Keep personal instructions, machine-specific configuration and credentials outside tracked project files.
