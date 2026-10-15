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

The global configuration lives at `<user config dir>/skm/config.toml`
(on Linux: `~/.config/skm/config.toml`, or `$XDG_CONFIG_HOME/skm/config.toml`).
`skm config path` prints it and `skm config show` prints the current TOML — when
the file does not exist, `show` prints the in-memory defaults without writing
anything. The store path is saved exactly as typed (so `~/skills-store` stays
readable) and expanded only when used. Updates preserve your comments and
formatting.

```toml
store = "~/skills-store"           # optional until `skm store add` registers one
default_target = ".agents/skills"

[agents]
opencode = ".opencode/skills"

[watch]
debounce_ms = 500
```

## Commands

| Command                                                                                             | Description                                                         |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `skm config path`                                                                                   | Print the path of the global configuration file                     |
| `skm config show`                                                                                   | Print the current configuration as TOML (never writes)              |
| `skm store add <path> [--create]`                                                                   | Register the store; `--create` creates a missing directory          |
| `skm store` / `skm store show`                                                                      | The registered store: path, state, layout and skill count           |
| `skm store remove`                                                                                  | Unregister the store (the directory is never touched)               |
| `skm store scan`                                                                                    | Full scan of the store: skills, packs and warnings                  |
| `skm list`                                                                                          | Skills available in the store                                       |
| `skm add <skill>... [--agent <name>] [--dir <path>] [--pin] [--force] [--dry-run]`                   | Copy skills into this project and declare them                      |
| `skm remove <skill>... [--force] [--dry-run]`                                                       | Delete the copies and the declarations                              |
| `skm status`                                                                                        | Health of every declared skill (read only)                          |
| `skm sync [--force] [--dry-run]`                                                                    | Reconcile the project with the store                                |

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

## Projects

Commands that install skills work on the project found from the working
directory: the nearest directory at or above it containing `skills.toml` or
`.skills-manager/`, else the working directory itself. A project holds two
files:

```toml
# skills.toml — committed, the declaration
[defaults]
target = ".agents/skills"          # optional; falls back to default_target

[[install]]
skill = "git-commit"
pin = true                         # optional: later syncs leave it alone
```

```toml
# .skills-manager/state.toml — generated, never commit it
[[skill]]
name = "git-commit"
source_rel = "git-commit"          # path of the skill inside the store
target = ".agents/skills"          # where the copy lives, project relative
written_hash = "blake3:..."        # content `skm` wrote
source_hash = "blake3:..."         # store content at install time
pinned = false
installed_at = 1760536800          # epoch seconds
```

The store is global: the manifest and the state never mention it. A copy that
`skm sync` had to detach (see below) is recorded as a _tombstone_: the same entry
plus `content_hash`, `detached_at` and `detached = true`. `skm` never touches a
tombstoned directory again, and it never adopts it back, so a local fork is safe
for good.

**Managed projects should ignore `.skills-manager/`** (add it to their
`.gitignore`); `skills.toml` is meant to be committed.

### Installing

`skm add` resolves each skill in the registered store, resolves the target
directory, and copies the whole skill directory as real files through a temporary
directory next to the destination plus an atomic `rename`, preserving
permissions. The target is chosen in this order:

1. `--dir <path>`
2. `--agent <name>` (looked up in the `[agents]` table of the global config)
3. `[defaults].target` of `skills.toml`
4. `default_target` of the global config (`.agents/skills`)

Each skill lands in `<target>/<skill name>`. `skm add` appends the `[[install]]`
declaration and writes the state file.

Guards, all reported as errors with the exact flag that gets past them:

- the skill was edited in the project → `--force` (or `skm remove`)
- the target directory is not managed by `skm` → `--force`
- `skm remove` on a locally edited copy → `--force`

`--dry-run` prints what would happen and writes nothing. Re-running `skm add`
for an untouched skill is a no-op: it re-syncs the copy without touching the
declaration or the state. `--pin` marks the copy as pinned in both files, and a
later `skm add` without `--pin` never unpins it.

### Checking

`skm status` is read only and prints one row per declared skill plus one row per
directory in a target that nothing declares. States, from the most to the least
urgent:

| State       | Meaning                                                               |
| ----------- | --------------------------------------------------------------------- |
| `missing`   | declared or recorded, but the directory is gone                       |
| `detached`  | a local fork: the declaration was dropped and `skm` never touches it  |
| `modified`  | the copy differs from `written_hash` (local edits)                    |
| `behind`    | the store content differs from `source_hash` (an update is available) |
| `pinned`    | in sync and pinned                                                    |
| `ok`        | in sync                                                               |
| `skipped`   | no store is registered, so the copy cannot be compared with one       |
| `unmanaged` | a directory in a target that neither file knows                       |

A detached fork is never compared with the store again, and without a registered
store the comparison is `skipped (no store registered)` while the local verdicts
(`missing`, `modified`, `detached`, `unmanaged`) keep being reported.

### Syncing

`skm sync` reconciles what the project declares with the store and prints one
line per entry plus a counter summary. Only the copies `skm` itself wrote are
ever refreshed: everything else is reported and left exactly as it is.

| Action      | Meaning                                                             |
| ----------- | ------------------------------------------------------------------- |
| `ok`        | the copy and the store are the ones recorded: nothing to do         |
| `updated`   | the store changed and the copy was refreshed from it                |
| `installed` | the copy was missing and has been written                           |
| `restored`  | a pinned copy was missing and has been written back                 |
| `adopted`   | a copy that already matched the store is now recorded               |
| `replaced`  | `--force`: a locally modified copy was overwritten                  |
| `pinned`    | the copy is pinned, so it is never updated                          |
| `detached`  | a locally modified copy: the declaration was dropped, the fork kept |
| `skipped`   | the store cannot provide the skill                                  |
| `unmanaged` | a directory in a target that nothing declares                       |

```text
$ skm sync
updated    deploy -> .agents/skills/deploy
detached   draft -> .agents/skills/draft (local modifications)
ok         git-commit -> .agents/skills/git-commit
3 entries: 1 ok, 1 updated, 1 detached
```

- **Pinned** copies are never updated; when their directory is gone they are
  restored from the store.
- **Locally modified** copies are never overwritten. The `[[install]]`
  declaration is dropped from `skills.toml` (comments and formatting survive)
  and a tombstone goes into the state file: the files stay exactly as they are
  and later syncs ignore them for good. `--force` overwrites them instead, and
  keeps the declaration.
- A copy with no recorded state (a fresh clone) is **adopted** when it matches
  the store, and detached when it does not.
- A store that no longer holds the skill is reported as
  `skipped (store unavailable)`, and with no store registered everything is
  reported as `skipped (no store registered)`: nothing is touched either way.
- `--dry-run` prints the plan and writes nothing at all.
- Sync is idempotent: running it again reports everything as `ok`.

### The tree hash

Hashes are `blake3:` followed by the hex digest of one canonical line per entry,
sorted so the digest does not depend on the filesystem order:

```text
<kind> <mode> <relative/path> <content-hash>
d - templates -
f - SKILL.md 8f1e...
f x bin/run 2a3c...
```

`kind` is `f` for files and `d` for directories, `mode` is `x` when the entry
has any execute bit and `-` otherwise, `content-hash` is the BLAKE3 of the file
bytes (`-` for directories), and paths are relative to the skill with `/`
separators. Symlinks are followed (their content is what gets copied), and the
entries listed below are ignored both by the hash and by the copy, so a freshly
installed skill has `written_hash == source_hash`:

- directories `.git/` and `node_modules/`
- files named `.DS_Store`
- files ending in `.tmp`
