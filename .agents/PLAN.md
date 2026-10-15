# Plan — skills-manager (`skm`)

Roadmap, pending work and known notes. The [README](../README.md) documents only
what exists today; this file keeps what is still missing.

## Status

**Implemented: M0 – M2** (193 tests green).
- The store is **global and unique**: one directory per machine, registered by path.
- **Zero-pollution architecture**: target projects contain NO `skills.toml`, NO `.skills-manager/` directory, and require NO `.gitignore` changes. All project metadata, targets, pins, tree hashes, and detached tombstones live centrally in `~/.local/state/skm/projects.toml`.
- Added `skm projects` command to list all managed projects on the machine.

| Phase | Scope | State |
| --- | --- | --- |
| M0 | Scaffold, global configuration, store commands (`config`, `store add/show/remove/scan`, `list`) | done |
| M1 | `add` / `remove` / `status` / `projects`: real file copies, zero-pollution architecture, central registry (`~/.local/state/skm/projects.toml`), tree hashes | done |
| M2 | `sync`: reconciliation with the store, auto-detach of locally modified copies, pins, adoption in central registry | done |
| M3 | Packs: named collections of skills that install together | pending |
| M4 | Hybrid watcher: auto-sync before commands, `skm watch`, systemd / System V service | pending |
| M5 | `doctor`, `--json`, CI `--check`, shell completions, docs polish | pending |

## Pending

### M3 — packs

* A pack is `<store>/packs/<name>.toml`: a named list of skills. Packs live in
  the store only — they are **never copied into projects** and never exist as a
  file in them; they just tell `skm` which skills to import together. They are
  also the answer to "categories": with a single global store there are no named
  stores to group skills by.
* Support packs in `add`, `remove`, `status` and `sync`.
* Reference counting: removing a pack must not delete skills still used by
  another pack or recorded individually.
* A pack member edited locally: detach that member (a state-level exclusion)
  without dropping the pack association.

### M4 — watcher (hybrid)

* Auto-sync before running commands (pull), configurable, with `--no-sync`.
* `skm watch` in the foreground: watch the registered stores (debounced) and
  sync every affected copy of the project.
* Service mode: `skm service install` → systemd user unit, System V init script
  as fallback; the daemon watches stores and syncs every registered project.
* The central project registry `~/.local/state/skm/projects.toml` already tracks
  all managed projects and targets.

### M5 — polish

* `skm doctor`: validate stores (missing `SKILL.md`), packs and state integrity.
* `--json` output; `skm sync --check` (exit code) for CI.
* Shell completions (`clap_complete`).
* Migration helper from tabernáculo: detect existing symlinks and turn them
  into managed copies.

## Known notes

* The store is **global and unique**: a single directory per machine, registered
  by path with `skm store add <path>` — no names, no `[stores]` table. The same
  path (after `~` expansion) is a no-op; another path replaces the current one.
* Without a registered store, `status` and `sync` report every entry as
  `skipped (no store registered)`, write nothing and exit 0; `add`, `remove`,
  `list`, `store`/`store show` and `store scan` fail with
  ``no store registered; run `skm store add <path>` ``.
* Old configuration formats are rejected with a hint: a config with `[stores]`
  or `default_store` fails with `old config format`.
* Zero-pollution: projects are never dirtied with metadata files. All project
  state is tracked in the central registry at `$XDG_STATE_HOME/skm/projects.toml`.
* `status` reports `behind` for a pinned skill whose store advanced — pin
  precedence is intentional.
* `skm remove` on a detached fork says "delete by hand" without naming the fork.
* A store that no longer holds a skill is never compared: `status` shows
  `ok`/`pinned`, `sync` reports `skipped (store unavailable)`.

## Dev

```sh
cargo build --release      # binary at target/release/skm
cargo test                 # unit tests + end-to-end tests
cargo clippy --all-targets -- -D warnings
cargo fmt
```
