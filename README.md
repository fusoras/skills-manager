# skills-manager (`skm`)

`skm` manages **skills** — directories containing a `SKILL.md` file — that live in
a local _store_ and are meant to be copied into projects. It works entirely
offline: no network, no GitHub, and no symlinks: skills are installed as **real
file copies**.

The store is global and unique: one directory per machine, registered by path
(never by name) in the global configuration.

## Quick start

```sh
skm store add ~/skills-store   # register the folder that holds your skills
skm store                      # where it is, whether it is readable, its layout
skm store scan                 # what the store offers
skm list                       # skills available in the store

skm add git-commit             # copy a skill into this project
skm add seo --pin              # --pin: sync will never update it
skm status                     # ok | behind | modified | pinned | ...
skm sync                       # reconcile the project with the store
```

## Configuration

The global configuration lives at `<user config dir>/skm/config.toml` (e.g. `~/.config/skm/config.toml` on Linux).

- `skm config path`: Print the configuration file location.
- `skm config show`: Print current configuration values and defaults without writing.

For complete details on store normalization, default targets, agent overrides, and watch options, see the [Configuration Documentation](docs/configuration.md).

## Commands

| Command                                                                   | Description                                                |
| ------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `skm config path`                                                         | Print the path of the global configuration file            |
| `skm config show`                                                         | Print the current configuration as TOML (never writes)     |
| `skm store add <path> [--create]`                                         | Register the store; `--create` creates a missing directory |
| `skm store` / `skm store show`                                            | The registered store: path, state, layout and skill count  |
| `skm store remove`                                                        | Unregister the store (the directory is never touched)      |
| `skm store scan`                                                          | Full scan of the store: skills, packs and warnings         |
| `skm list [--packs] [--desc]`                                              | Skills (with optional description) or packs in store        |
| `skm pack [list]`                                                         | List available packs in the store                          |
| `skm pack show <name>`                                                    | Details of a pack (skills, description)                    |
| `skm pack edit <name> [--add <skill>...] [--remove <skill>...]`           | Open pack in $EDITOR or add/remove skills directly via CLI |
| `skm pack status`                                                         | State of packs installed in this project                   |
| `skm pack add [<pack>...] [-i] [agent flags] [--pin] [--force] [--dry-run]`| Install packs into this project (interactive picker if empty) |
| `skm pack remove <pack>... [agent flags] [--force] [--dry-run]`           | Remove packs with reference counting                       |
| `skm projects`                                                            | List all managed projects on this machine                  |
| `skm add [<skill>...] [-i] [--pack] [agent flags] [--pin] [--force] [--dry-run]` | Copy skills into targets (interactive picker if empty)    |
| `skm remove <skill>... [--pack] [agent flags] [--force] [--dry-run]`      | Delete copies or unlink packs with reference counting      |
| `skm status [-p\|--project] [-g\|--global] [--packs]`                      | Unified visual status of project and global skills         |
| `skm sync [--force] [--dry-run]`                                          | Reconcile the project with the store                       |

`skm list` prints the skills available in the store. Passing `--desc` (or `-d` / `--description`) includes the skill description dimmed in gray, truncated dynamically with an ellipsis (`...`) to adapt to the terminal width without hiding the first word.

`skm store add <path>` registers one path: the same path (compared after `~`
expansion, so two spellings of one directory are one store) is a no-op and
prints `store already registered: <path>`; any other path replaces the current
store, reporting the previous one. Every command that needs the store fails
clearly when none is registered:

```text
error: no store registered; run `skm store add <path>`
```

`skm status` and `skm sync` are the exception: they report every entry as
`skipped (no store registered)`, write nothing and exit 0.

### Store layouts

- **catalog** — the store contains a `skills/` directory; skills are its
  subdirectories.
- **flat** — skills are direct subdirectories of the store root. Hidden entries
  and `packs/` are ignored.

A skill's name is the `name:` field of the `SKILL.md` frontmatter, falling back
to the directory name. `skm store scan` reports skills, the number of `*.toml`
pack definitions in `packs/`, and warnings for directories without `SKILL.md`
and for frontmatter names that do not match their directory.

## Project

### Installing

`skm add` resolves each skill in the registered store, resolves the target
directories, and copies the whole skill directory as real files through a temporary
directory next to the destination plus an atomic `rename`, preserving
permissions.

When run without arguments (e.g. `skm add`) in an interactive terminal or with the `-i` / `--interactive` flag, `skm` launches a lightweight terminal multi-select picker:

```text
? Select skills to install:
[x] git-commit
[ ] rust-best-practices
[x] strict-testing

[Press <space> to toggle, <a> to toggle all, <enter> to confirm]
```

If no agent flags were specified on the command line, `skm` then presents a multi-select prompt for the target agent(s):

```text
? Select target agent(s):
[x] opencode         (.opencode/skills)
[ ] antigravity      (.gemini/skills)
[ ] claude           (.claude/skills)
[ ] pi               (.pi/skills)
[ ] cursor           (.cursor/skills)
[ ] codex            (.codex/skills)
[ ] agent            (.agents/skills)

[Press <space> to toggle, <a> to toggle all, <enter> to confirm]
```

(The same interactive selection is available for packs with `skm pack add -i` or `skm add --pack -i`).

Targets are chosen via agent flags:

- `--agent` (or alias `--agents`): standard agents directory (`.agents/skills`) — **default** if no flag is given
- `--antigravity` (or alias `--agy`): Antigravity CLI directory (`.gemini/skills`)
- `--opencode`: OpenCode directory (`.opencode/skills`)
- `--claude`: Claude directory (`.claude/skills`)
- `--codex`: Codex directory (`.codex/skills`)
- `--pi`: Pi directory (`.pi/skills`)
- `--cursor`: Cursor directory (`.cursor/skills`)
- `--dir <path>`: any custom target path

You can combine multiple agent flags in a single command to install into multiple
agent targets simultaneously:

```sh
skm add git-commit --opencode --pi --codex
```

### Global Agent Skills

Pass `--global` (or `-g`) to manage skills across your user environment rather than inside a specific project:

```sh
skm add git-commit --global --opencode      # installs to ~/.config/opencode/skills/git-commit
skm add git-commit -g --agy                 # installs to ~/.gemini/skills/git-commit
skm add git-commit --global --pi            # installs to ~/.pi/agent/skills/git-commit

skm status --global                         # view status of all global skills
skm sync --global                           # reconcile global skills with the store
skm remove git-commit --global --opencode   # deletes only from OpenCode's global directory
```

Global agent targets:
- Antigravity CLI (`--antigravity` / `--agy`): `~/.gemini/skills`
- OpenCode (`--opencode`): `~/.config/opencode/skills`
- Pi (`--pi`): `~/.pi/agent/skills`
- Claude (`--claude`): `~/.claude/skills`
- Codex (`--codex`): `~/.codex/skills`
- Cursor (`--cursor`): `~/.cursor/skills`
- Default Agent (`--agent`): `~/.agents/skills`

Each skill lands in `<target>/<skill name>`. `skm add` records the project and the
skills in `~/.local/state/skm/projects.toml`.

Guards, all reported as errors with the exact flag that gets past them:

- the skill was edited in the project → `--force` (or `skm remove`)
- the target directory is not managed by `skm` → `--force`
- `skm remove` on a locally edited copy → `--force`

`--dry-run` prints what would happen and writes nothing. Re-running `skm add`
for an untouched skill is a no-op: it re-syncs the copy without re-adding entries.
`--pin` marks the copy as pinned in `projects.toml`, and a later `skm add`
without `--pin` never unpins it.

### Discovering the project

Commands that install or manage skills find the project root by inspecting the working
directory and its ancestors:

1. The nearest directory containing a `.git/` folder or recorded in `projects.toml`
2. If neither is found, the current working directory itself

Run `skm projects` to see all projects currently managed on your machine:

```text
$ skm projects
~/work/frontend   (3 skills)
~/work/backend    (5 skills)
```

### Packs

Packs are named collections of skills that install together from `<store>/packs/<name>.toml`. Target projects remain 100% clean (zero metadata files).

- **Install**: `skm add --pack frontend` or `skm pack add frontend`
- **Inspect**: `skm pack list` and `skm pack show <name>`
- **Edit**: `skm pack edit <name> [--add <skill>...] [--remove <skill>...]` or via `$EDITOR`
- **Reference counting**: Shared skills are safely retained until all referencing packs are removed.

For the complete guide on pack definitions, CLI editing, multi-target installation, reference counting, and local detach safety, see the [Packs Documentation](docs/pack.md).

### Checking

`skm status` is read only and displays a **Unified Visual Status View** of skills across both the current project and global agent environments (`~/.config/opencode/skills`, `~/.gemini/skills`, etc.):

- Run `skm status` to view both the current project and global agent sections.
- Pass `-p` / `--project` to filter to project skills only.
- Pass `-g` / `--global` to filter to global agent skills only.

In color-capable terminals, each row features high-visibility status icons:

| Icon & State | Meaning                                                               |
| ------------ | --------------------------------------------------------------------- |
| `✖ missing`   | recorded in central registry, but the directory is gone               |
| `⤹ detached`  | a local fork: `skm` never touches it                                  |
| `✎ modified`  | the copy differs from `written_hash` (local edits)                    |
| `↓ behind`    | the store content differs from `source_hash` (an update is available) |
| `⚑ pinned`    | in sync and pinned                                                    |
| `✔ ok`        | in sync                                                               |
| `○ skipped`   | no store is registered, so the copy cannot be compared with one       |
| `? unmanaged` | a directory in a target that central registry does not know           |

A detached fork is never compared with the store again, and without a registered
store the comparison is `skipped (no store registered)` while the local verdicts
(`missing`, `modified`, `detached`, `unmanaged`) keep being reported.

### Syncing

`skm sync` reconciles what the project has recorded in `projects.toml` with the store
and prints one line per entry plus a counter summary. Only the copies `skm` itself
wrote are ever refreshed: everything else is reported and left exactly as it is.

| Action      | Meaning                                                           |
| ----------- | ----------------------------------------------------------------- |
| `ok`        | the copy and the store are the ones recorded: nothing to do       |
| `updated`   | the store changed and the copy was refreshed from it              |
| `installed` | the copy was missing and has been written                         |
| `restored`  | a pinned copy was missing and has been written back               |
| `adopted`   | a copy that already matched the store is now recorded in registry |
| `replaced`  | `--force`: a locally modified copy was overwritten                |
| `pinned`    | the copy is pinned, so it is never updated                        |
| `detached`  | a locally modified copy: detached tombstone recorded, fork kept   |
| `skipped`   | the store cannot provide the skill                                |
| `unmanaged` | a directory in a target that nothing records                      |

```text
$ skm sync
updated    deploy -> .agents/skills/deploy
detached   draft -> .agents/skills/draft (local modifications)
ok         git-commit -> .agents/skills/git-commit
3 entries: 1 ok, 1 updated, 1 detached
```

- **Pinned** copies are never updated; when their directory is gone they are
  restored from the store.
- **Locally modified** copies are never overwritten. A detached tombstone is
  recorded in `projects.toml`: the files stay exactly as they are and later syncs
  ignore them for good. `--force` overwrites them instead and keeps them active.
- A copy with no recorded state is **adopted** when it matches the store, and
  detached when it does not.
- A store that no longer holds the skill is reported as
  `skipped (store unavailable)`, and with no store registered everything is
  reported as `skipped (no store registered)`: nothing is touched either way.
- `--dry-run` prints the plan and writes nothing at all.
- Sync is idempotent: running it again reports everything as `ok`.

## References

- [Configuration](docs/configuration.md) — Global configuration file structure and settings reference.
- [Tree Hash](docs/hash.md) — BLAKE3 canonical tree hashing, deterministic ordering, and exclusion rules.
- [Packs](docs/pack.md) — Named skill collections, multi-target installation, CLI editing, and reference counting.
