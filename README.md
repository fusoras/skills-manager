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
anything. `skm store add` normalizes the path before saving it: a directory
inside your home is stored as `~/...` (so `~/skills-store` stays readable and
portable), anything else as an absolute path — a relative path is resolved from
the working directory first. `~` is expanded whenever the store is used. Updates
preserve your comments and formatting.

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
| `skm list [--packs]`                                                                                | Skills or packs available in the store                              |
| `skm pack [list]`                                                                                   | List available packs in the store                                   |
| `skm pack show <name>`                                                                              | Details of a pack (skills, description)                             |
| `skm pack edit <name> [--add <skill>...] [--remove <skill>...]`                                     | Open pack in $EDITOR or add/remove skills directly via CLI          |
| `skm pack status`                                                                                   | State of packs installed in this project                            |
| `skm pack add <pack>... [agent flags] [--pin] [--force] [--dry-run]`                                | Install packs into this project                                     |
| `skm pack remove <pack>... [agent flags] [--force] [--dry-run]`                                      | Remove packs with reference counting                                |
| `skm projects`                                                                                      | List all managed projects on this machine                           |
| `skm add <skill>... [--pack] [agent flags] [--pin] [--force] [--dry-run]`                           | Copy skills or packs into project agent targets                     |
| `skm remove <skill>... [--pack] [agent flags] [--force] [--dry-run]`                                 | Delete copies or unlink packs with reference counting               |
| `skm status [--packs]`                                                                              | Health of skills or packs (read only)                               |
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

`skm` uses a **zero-pollution architecture**: target projects contain **no metadata files
and no configuration dotfiles**. There is no `skills.toml`, no `.skills-manager/`
directory, and no need to touch `.gitignore`. The project holds only the actual
skill directories installed into your agent target folders (e.g. `.agents/skills/<skill>/`).

All project registrations, target paths, installed skill records, BLAKE3 hashes, pins,
and detached tombstones are stored centrally in the user state directory:
`<user state dir>/skm/projects.toml` (on Linux: `~/.local/state/skm/projects.toml`,
or `$XDG_STATE_HOME/skm/projects.toml`).

```toml
# ~/.local/state/skm/projects.toml — central registry
[projects."~/my-project"]
targets = [".agents/skills"]

[[projects."~/my-project".skill]]
name = "git-commit"
source_rel = "git-commit"          # path of the skill inside the store
target = ".agents/skills"          # where the copy lives, project relative
written_hash = "blake3:..."        # content `skm` wrote
source_hash = "blake3:..."         # store content at install time
pinned = false
installed_at = 1760536800          # epoch seconds
```

The store is global: neither projects nor the central registry ever hardcode a store
path into the skills. When a locally modified copy is detached by `skm sync` (see below),
it is marked with `detached = true`, `content_hash`, and `detached_at`. `skm` never touches
a tombstoned directory again, keeping local forks safe.

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

### Installing

`skm add` resolves each skill in the registered store, resolves the target
directories, and copies the whole skill directory as real files through a temporary
directory next to the destination plus an atomic `rename`, preserving
permissions.

Targets are chosen via agent flags:

- `--agent` (or alias `--agents`): standard agents directory (`.agents/skills`) — **default** if no flag is given
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

### Packs

Packs are named collections of skills that install together. They are defined in
`<store>/packs/<name>.toml` in the store root:

```toml
# <store>/packs/frontend.toml
name = "frontend"
description = "Web development tools"
skills = ["react", "tailwind", "seo"]
```

Packs live in the store only: they are **never copied into target projects** as files
and maintain the zero-pollution guarantee.

#### Creating a pack

1. Create a `.toml` file inside the `packs/` directory of your store (e.g. `<store>/packs/web.toml`):
   ```toml
   # <store>/packs/web.toml
   name = "web"                         # Optional: defaults to file stem ("web")
   description = "Web development tools" # Optional
   skills = [
       "git-commit",
       "react",
   ]
   ```
2. Verify that `skm` detects it:
   ```sh
   skm pack list
   skm pack show web
   ```

#### Adding or removing skills with `skm pack edit`

You can modify pack skills directly from the command line without opening a text editor:

```sh
# Add one or more skills to a pack (creates the pack if it does not exist)
skm pack edit web --add tailwind seo

# Remove skills from a pack
skm pack edit web --remove seo
```

Or open the pack file in your preferred text editor (`$EDITOR` / `$VISUAL`, fallback to `nano`):

```sh
skm pack edit web
```

To install newly added skills into projects that already use the pack:

```sh
skm add --pack web
# or: skm pack add web
```

- **List & Show**: `skm pack` or `skm list --packs` displays all available packs;
  `skm pack show <name>` inspects pack members and metadata.
- **Install**: `skm add --pack frontend` or `skm pack add frontend` installs all
  member skills into the chosen agent target directories (`--agent`, `--opencode`, etc.).
- **Reference Counting**: Skills shared across multiple packs or installed directly are
  safely reference-counted. Removing a pack (`skm remove --pack frontend` or
  `skm pack remove frontend`) only deletes skills that are not required by another
  pack or installed directly.
- **Local Detach Safety**: If a skill installed via a pack is modified locally,
  `skm sync` automatically detaches it as a local fork (`detached = true`) without
  dropping its pack association.

### Checking

`skm status` is read only and prints one row per recorded skill in `projects.toml`
plus one row per unmanaged directory found in known target folders. States, from
the most to the least urgent:

| State       | Meaning                                                               |
| ----------- | --------------------------------------------------------------------- |
| `missing`   | recorded in central registry, but the directory is gone               |
| `detached`  | a local fork: `skm` never touches it                                  |
| `modified`  | the copy differs from `written_hash` (local edits)                    |
| `behind`    | the store content differs from `source_hash` (an update is available) |
| `pinned`    | in sync and pinned                                                    |
| `ok`        | in sync                                                               |
| `skipped`   | no store is registered, so the copy cannot be compared with one       |
| `unmanaged` | a directory in a target that central registry does not know           |

A detached fork is never compared with the store again, and without a registered
store the comparison is `skipped (no store registered)` while the local verdicts
(`missing`, `modified`, `detached`, `unmanaged`) keep being reported.

### Syncing

`skm sync` reconciles what the project has recorded in `projects.toml` with the store
and prints one line per entry plus a counter summary. Only the copies `skm` itself
wrote are ever refreshed: everything else is reported and left exactly as it is.

| Action      | Meaning                                                             |
| ----------- | ------------------------------------------------------------------- |
| `ok`        | the copy and the store are the ones recorded: nothing to do         |
| `updated`   | the store changed and the copy was refreshed from it                |
| `installed` | the copy was missing and has been written                           |
| `restored`  | a pinned copy was missing and has been written back                 |
| `adopted`   | a copy that already matched the store is now recorded in registry   |
| `replaced`  | `--force`: a locally modified copy was overwritten                  |
| `pinned`    | the copy is pinned, so it is never updated                          |
| `detached`  | a locally modified copy: detached tombstone recorded, fork kept     |
| `skipped`   | the store cannot provide the skill                                  |
| `unmanaged` | a directory in a target that nothing records                        |

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
