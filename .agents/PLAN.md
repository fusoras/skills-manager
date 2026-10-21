# Plan — skills-manager (`skm`)

Roadmap, pending work and known notes. The [README](../README.md) documents only
what exists today; this file keeps what is still missing.

## Status

**Implemented: M0 – M3 + Global Agent Skills + Interactive UX & Unified Status** (215 tests green).
- The store is **global and unique**: one directory per machine, registered by path.
- **Zero-pollution architecture**: target projects contain NO `skills.toml`, NO `.skills-manager/` directory, and require NO `.gitignore` changes. All project metadata, targets, pins, tree hashes, and detached tombstones live centrally in `~/.local/state/skm/projects.toml`.
- Added `skm projects` command to list all managed projects on the machine.
- **Packs (M3)**: named collections in `<store>/packs/<name>.toml` installing together with reference counting and automatic detach of local modifications. Supported via `skm pack [list|show|edit|status|add|remove]` and flags `--pack` / `--packs`.
- **Global Agent Skills**: `--global` (`-g`) flag to install, remove, inspect (`status`), and `sync` user-level skills across agents: Antigravity CLI (`~/.gemini/skills`, `--agy`), OpenCode (`~/.config/opencode/skills`), Pi (`~/.pi/agent/skills`), Claude (`~/.claude/skills`), Codex, and Cursor. Agent-targeted removal (`skm remove <skill> --global --<agent>`) safely deletes skills per agent without touching others.
- **Interactive UX & Unified Status**:
  - Interactive multi-select picker (`inquire`) launched when `skm add` or `skm pack add` is run without skill arguments in a terminal or with `-i` / `--interactive`.
  - Unified visual status view (`skm status`): displays both current project and global agent skills, scoped via `-p` / `--project` and `-g` / `--global`, with clear unicode icons (`✔ ok`, `✎ modified`, `↓ behind`, `⚑ pinned`, `✖ missing`, `? unmanaged`, `⤹ detached`, `○ skipped`).

| Phase | Scope | State |
| --- | --- | --- |
| M0 | Scaffold, global configuration, store commands (`config`, `store add/show/remove/scan`, `list`) | done |
| M1 | `add` / `remove` / `status` / `projects`: real file copies, zero-pollution architecture, central registry (`~/.local/state/skm/projects.toml`), tree hashes | done |
| M2 | `sync`: reconciliation with the store, auto-detach of locally modified copies, pins, adoption in central registry | done |
| M3 | Packs: named collections of skills that install together | done |
| M4 | Hybrid watcher: auto-sync before commands, `skm watch`, systemd / System V service | pending |
| M5 | `doctor`, `--json`, CI `--check`, shell completions, docs polish | pending |

## Pending

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
