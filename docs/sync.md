# Synchronization in `skm` (`skm sync`)

`skm sync` reconciles skill installations with the registered global store. It operates deterministically using canonical BLAKE3 tree hashes without git history, network calls, or modifying repository metadata (zero pollution).

---

## Architecture & Directionality

`skm` supports bidirectional synchronization workflows between the **Store** (central source of truth) and **Target Projects / Global Agent Environments**:

```text
       Downstream: skm sync (default)
  ┌─────────────────────────────────────┐
  │                                     ▼
Store (Master)                   Project / Agent Copies
  ▲                                     │
  └─────────────────────────────────────┘
       Upstream: skm sync --to-store
```

1. **Downstream Reconciliation (`Store -> Project/Agent`)**:
   Refreshes untouched local copies with store updates, restores missing copies, and protects local modifications from being overwritten by auto-detaching them.
2. **Upstream Synchronization (`Project/Agent -> Store`)**:
   Pushes local modifications back into the store with `--to-store`, updates store content atomically, and restores local status back to `✔ ok`.

---

## Downstream Reconciliation (`skm sync`)

Running `skm sync` without flags checks all recorded skills against the store and updates them when needed:

```sh
skm sync
```

### Action Reference

| Action      | Visual State | Meaning                                                              |
| ----------- | ------------ | -------------------------------------------------------------------- |
| `ok`        | `✔ ok`       | Local copy and store match the recorded state; nothing to do.        |
| `updated`   | `✔ ok`       | Store content changed and untouched local copy was refreshed from it.|
| `installed` | `✔ ok`       | Copy was missing on disk and has been written from the store.        |
| `restored`  | `⚑ pinned`   | A pinned copy was missing and has been restored from the store.      |
| `adopted`   | `✔ ok`       | An unrecorded directory matching the store was adopted into registry.|
| `replaced`  | `✔ ok`       | `--force`: a locally modified copy was overwritten from the store.   |
| `pinned`    | `⚑ pinned`   | Copy is marked as pinned; store changes are ignored.                 |
| `detached`  | `⤹ detached` | Local modifications detected; fork kept untouched and auto-detached. |
| `skipped`   | `○ skipped`  | Store is missing, or store no longer provides the skill.             |
| `unmanaged` | `? unmanaged`| Directory in a target directory not declared in central registry.    |

### Local Modification Protection (Auto-Detach)

When a skill copy is modified locally:
- `skm status` reports `✎ modified`.
- Running `skm sync` **never overwrites your local edits**. Instead, it records a detached tombstone in `~/.local/state/skm/projects.toml` (`detached = true`).
- A detached copy becomes an independent local fork: future regular `skm sync` runs ignore it completely.
- Pass `--force` to intentionally discard local modifications and re-align with the store (`replaced`).

---

## Upstream Synchronization (`skm sync --to-store`)

When you develop or improve a skill inside a project or global agent directory, you can push those changes back to the registered store using `--to-store`.

### Syntax

```sh
# Push all modified or detached skills in this project to the store
skm sync --to-store

# Push a specific skill only
skm sync <skill> --to-store

# Push a global agent skill to the store
skm sync <skill> --global --to-store
skm sync --global --to-store

# Preview actions without writing to store or registry
skm sync --to-store --dry-run

# Force push when the store copy has diverged independently
skm sync <skill> --to-store --force
```

### What Happens During `--to-store`

1. **Tree Verification**:
   Validates that the local directory contains a valid `SKILL.md` and computes its canonical BLAKE3 tree hash (including `templates/`, `scripts/`, permissions, etc.).
2. **Atomic Store Update**:
   Copies the local tree into the store atomically via temporary directory swap (`install_tree`), preserving executable permissions and directory structure while skipping `.git`, `node_modules`, and system temporary files.
3. **Automatic Re-attachment**:
   If the skill was previously marked as detached (`⤹ detached`), `--to-store` automatically clears the detached tombstone (`detached = false`, `detached_at = None`).
4. **Registry Hash Alignment**:
   Updates both `written_hash` and `source_hash` in `~/.local/state/skm/projects.toml` to match the new content hash.
5. **Clean Status**:
   `skm status` immediately reports `✔ ok` for the skill.

### Divergence Guard

If the store copy was edited independently (e.g. in another project or manually) since the skill was installed, pushing local modifications could accidentally overwrite external changes.

`skm sync --to-store` detects this divergence:
```text
error: store copy of '<skill>' has changed since it was installed; pass --force to overwrite the store
```
Passing `--force` explicitly confirms that the local version should overwrite the diverged store copy.

---

## Selective Synchronization (`skm sync [SKILL]...`)

You can supply one or more skill names to selectively sync or push specific skills without touching others:

```sh
# Downstream: sync only git-commit from store to project
skm sync git-commit

# Upstream: push only rust-best-practices to store
skm sync rust-best-practices --to-store

# Multiple skills
skm sync git-commit rust-best-practices --to-store
```

---

## Common Flags

| Flag         | Description                                                          |
| ------------ | -------------------------------------------------------------------- |
| `--to-store` | Push local modifications upstream to the global store                |
| `-g, --global` | Operate on user-level global agent skills instead of project skills |
| `--force`    | Force overwrite (local edits on pull, or diverged store on push)     |
| `--dry-run`  | Output planned actions and verification checks without writing files |
