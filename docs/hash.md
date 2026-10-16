# Tree Hash in `skm`

`skm` uses deterministic cryptographic tree hashing to detect changes between the global skill store and local project installations without requiring network access, git histories, or tracking files in the target repositories.

---

## Hash Format

Hashes are prefixed with `blake3:` followed by the lowercase hex digest of BLAKE3 over canonical manifest lines, sorted lexicographically so the final digest does not depend on the filesystem traversal order:

```text
blake3:<64-character-hex-digest>
```

---

## Manifest Line Format

Each entry (file or directory) in the skill tree produces one canonical manifest line:

```text
<kind> <mode> <relative/path> <content-hash>
```

### Fields

- **`kind`**:
  - `f`: regular file
  - `d`: directory
- **`mode`**:
  - `x`: executable (Unix permissions contain any execute bit `0o111 != 0`)
  - `-`: non-executable
- **`relative/path`**:
  - Path relative to the root of the skill.
  - Normalized with forward slashes (`/`) regardless of operating system.
- **`content-hash`**:
  - For files (`f`): BLAKE3 hex digest of the raw file content bytes.
  - For directories (`d`): `-`

### Example Manifest

```text
d - templates -
f - SKILL.md 8f1e63a8244f7d4b...
f x bin/run 2a3c7198f02bc91e...
```

---

## Determinism & Exclusion Rules

1. **Deterministic Sorting**: All canonical manifest lines are sorted lexicographically before computing the BLAKE3 digest with `\n` line delimiters.
2. **Symlinks**: Symlinks are followed during traversal (`follow_links(true)`); target contents and permissions are evaluated.
3. **Empty Directories & Modes**: Empty directories and executable bits are preserved and directly affect the tree hash.
4. **Exclusions**: The following entries are excluded from both hashing and filesystem copying:
   - Directories: `.git/` and `node_modules/`
   - Files: `.DS_Store`
   - Suffixes: `*.tmp`

Because tree hashing and `copy_tree` share identical exclusion rules, a freshly installed skill always satisfies:
```text
written_hash == source_hash
```

---

## Usage in State Tracking

The tree hash is recorded in `~/.local/share/skills-manager/projects.toml`:
- **`source_hash`**: The hash of the skill in the central store at the time of installation or sync.
- **`written_hash`**: The hash of the skill written into the target project.

This enables `skm status` and `skm sync` to reliably categorize skill states:
- `ok`: `current_project_hash == written_hash == source_hash`
- `modified`: `current_project_hash != written_hash` (local edits detected; prevents accidental overwrites)
- `behind`: `current_project_hash == written_hash && source_hash != current_store_hash` (upstream update available in store)
- `detached`: A locally modified skill detached as a local fork (`detached = true`) to preserve user edits.
