# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `skm rule` command suite for non-destructive management, injection, and synchronization of `AGENTS.md` and agent instructions (`GEMINI.md`, `CLAUDE.md`, etc.).
  - `skm rule apply [NAME]`: Injects instructions from store (`AGENTS.md` or `rules/<name>.md`) into target files enclosed in comment markers (`<!-- BEGIN SKM INSTRUCTIONS --> ... <!-- END SKM INSTRUCTIONS -->`), keeping surrounding user text and comments completely intact.
  - `skm rule sync`: Synchronizes applied instruction blocks with the store across project or global targets without touching user notes.
  - `skm rule status`: Renders a table of applied rules across project and global targets with states (`ok`, `modified`, `behind`, `missing`).
  - `skm rule list` and `skm rule show <NAME>`: Lists available rules in the store and displays full rule text.
  - `skm rule remove <NAME>`: Non-destructively removes the managed instruction block, removing the file if empty or preserving user notes.
  - Strict global target protection: When run with `--global`, strictly forbids falling back to non-standard `~/.agents/AGENTS.md`. Requires an agent flag (`--agy`, `--claude`, `--opencode`, `--cursor`), `--file <path>`, or launches an interactive picker with custom path support.
- Added `--to-store` flag to `skm sync` allowing local skill modifications to be pushed directly back to the registered global store.
- Added selective skill argument support (`skm sync [SKILL]...`) to reconcile or push specific skills rather than all project skills (e.g. `skm sync <skill> --to-store`).
- Added automatic re-attachment: running `skm sync --to-store` on a locally modified or previously detached skill removes the detached tombstone and updates hashes in the central registry (`~/.local/state/skm/projects.toml`), returning the skill state to `ok`.
- Added safety divergence guard: prevents accidental overwrite when both the local copy and the store copy changed independently, requiring `--force` to overwrite the store.
