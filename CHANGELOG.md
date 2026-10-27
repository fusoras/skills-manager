# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added `--to-store` flag to `skm sync` allowing local skill modifications to be pushed directly back to the registered global store.
- Added selective skill argument support (`skm sync [SKILL]...`) to reconcile or push specific skills rather than all project skills (e.g. `skm sync <skill> --to-store`).
- Added automatic re-attachment: running `skm sync --to-store` on a locally modified or previously detached skill removes the detached tombstone and updates hashes in the central registry (`~/.local/state/skm/projects.toml`), returning the skill state to `ok`.
- Added safety divergence guard: prevents accidental overwrite when both the local copy and the store copy changed independently, requiring `--force` to overwrite the store.
