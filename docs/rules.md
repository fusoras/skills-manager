# Agent Rules & Instructions Management in `skm` (`skm rule`)

`skm rule` manages instructions, rules, and system prompts for AI agents (such as `AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, etc.) across local projects and global environments. It operates using non-destructive comment markers, allowing team members and developers to maintain their own project notes while synchronizing shared instructions from a centralized store.

---

## Quick Start

### 1. View Available Rules

List all rules available in your registered store:

```sh
skm rule list
```

### 2. Apply Instructions to Your Project

Inject your store's default instructions into `./AGENTS.md`:

```sh
skm rule apply
```

The instructions are placed between comment markers:

```markdown
<!-- BEGIN SKM INSTRUCTIONS -->
# Team Conventions
- Follow standard conventions
<!-- END SKM INSTRUCTIONS -->
```

> [!NOTE]
> You can add any notes, instructions, or comments above or below these markers. `skm` will never touch or overwrite anything outside the markers!

### 3. Apply to a Specific Agent Globally

Configure your global agent instructions (e.g. Antigravity CLI / Gemini, Claude, Cursor, Pi):

```sh
skm rule apply --global --agy       # Configures ~/.gemini/GEMINI.md
skm rule apply --global --claude    # Configures ~/.claude/CLAUDE.md
skm rule apply --global --cursor    # Configures ~/.cursor/AGENTS.md
skm rule apply --global --pi        # Configures ~/.pi/agent/AGENTS.md
```

### 4. Check Status & Sync Updates

When shared rules change in the store, inspect drift and sync them:

```sh
skm rule status                     # Check whether rules are ok, modified, or behind
skm rule sync                       # Pull latest store updates into your marked blocks
```

---

## Comment Marker Protocol

When `skm rule apply` writes to a destination file, it wraps the store content inside standard comment markers.

### Default Rule Marker

Used for root instructions (`AGENTS.md`):

```markdown
# Developer Custom Header (stays untouched)

<!-- BEGIN SKM INSTRUCTIONS -->
# Team Conventions
- Always run cargo fmt before committing
- Follow conventional commits
<!-- END SKM INSTRUCTIONS -->

# Developer Custom Notes (stays untouched)
```

### Tagged Sub-Rule Markers

Used for named or modular rules from `<store>/rules/<name>.md`:

```markdown
<!-- BEGIN SKM INSTRUCTIONS: review -->
# Code Review Checklist
- Check for security vulnerabilities
- Ensure comprehensive test coverage
<!-- END SKM INSTRUCTIONS: review -->
```

Multiple tagged rules and the default rule can safely co-exist within the same file.

### Modification & Replacement Rules

- **Existing File without Markers**: By default, `skm` appends the marked block to the bottom of the file with separating newlines.
- **Existing File with Markers**: Only the lines between the matching `BEGIN` and `END` markers are replaced. Content above and below the block is 100% preserved.
- **Whole-File Replacement (`--overwrite`)**: Overwrites the entire file with the marked block, wiping preexisting text outside markers.
- **Clean Removal (`skm rule remove`)**: When removing a rule, if the file only contains the marked block, the file is deleted. If other user text exists, the block is excised, surrounding blank lines are collapsed cleanly, and the remaining user content is preserved.

---

## Store Layout for Rules

Rules live in the registered store root:

```text
<store>/
├── AGENTS.md          # Default canonical rule
├── rules/             # Sub-rules directory (ignored during skill scanning)
│   ├── review.md      # Rule named "review"
│   ├── rust.md        # Rule named "rust"
│   └── security.md    # Rule named "security"
└── skills/            # Skills directory (or root skills in flat layout)
```

- `<store>/AGENTS.md`: The canonical default rule applied when no rule name is specified.
- `<store>/rules/*.md`: Modular rules identified by their file stem (e.g. `review` for `rules/review.md`).
- Files in `rules/` are never treated as skills by `skm list` or `skm sync`.

---

## Command Reference

### `skm rule list`

List all rules available in the registered store:

```sh
skm rule list
```

Example output:

```text
RULE     SOURCE           KIND
default  AGENTS.md        default
review   rules/review.md  rule
rust     rules/rust.md    rule
```

### `skm rule show <name>`

Print the raw content of a store rule to stdout:

```sh
skm rule show default
skm rule show review
```

### `skm rule apply [<name>] [flags]`

Apply a rule to the resolved target files. If `<name>` is omitted, defaults to `default` (`AGENTS.md`).

```sh
# Local project: applies default rule to ./AGENTS.md
skm rule apply

# Local project: applies rules/review.md into ./AGENTS.md with tagged markers
skm rule apply review

# Target a specific agent in project mode
skm rule apply --agy        # Writes to .gemini/GEMINI.md
skm rule apply --claude     # Writes to CLAUDE.md
skm rule apply --opencode   # Writes to AGENTS.md
skm rule apply --cursor     # Writes to .cursor/AGENTS.md

# Target an arbitrary custom file
skm rule apply --file docs/PROMPT.md

# Replace entire target file instead of non-destructively updating/appending
skm rule apply --overwrite

# Dry run: view what would change without modifying files
skm rule apply --dry-run
```

### `skm rule sync [-g|--global] [--dry-run]`

Reconcile all recorded rules in the current project or global environment with the latest store content:

```sh
skm rule sync              # Sync project rules
skm rule sync --global     # Sync global rules
skm rule sync --dry-run    # Preview planned changes
```

If store rules changed, `sync` replaces the inner block while preserving any external user notes.

### `skm rule status [-p|--project] [-g|--global]`

Display the status of recorded rules across project and global targets:

```sh
skm rule status
```

Example output:

```text
Project Rules:
RULE     TARGET     STATE
default  AGENTS.md  ok
review   AGENTS.md  behind

Global Rules:
RULE     TARGET              STATE
default  ~/.gemini/GEMINI.md ok
```

#### Status Values

| State               | Meaning                                                                 |
| ------------------- | ----------------------------------------------------------------------- |
| `ok`                | Injected block matches the store and recorded hash.                     |
| `behind`            | Store rule has updated; run `skm rule sync` to update the marked block. |
| `modified`          | Content inside the markers was edited manually in the target file.      |
| `modified (no markers)` | Target file exists but does not contain the required comment markers. |
| `missing`           | Target file does not exist on disk.                                     |
| `store unavailable` | The rule no longer exists in the store.                                 |

### `skm rule remove <name> [flags]`

Remove an applied rule block from the resolved targets:

```sh
skm rule remove default
skm rule remove review
skm rule remove default --global --agy
```

If no other content remains in the target file, `skm` removes the file entirely. If custom user content exists, only the marked block is removed.

---

## Agent Target Matrix & Global Safety

### Global Mode Safety Constraint

In `--global` mode, **`skm` strictly refuses to default to `~/.agents/AGENTS.md`**. No standard AI coding assistant or agent reads instructions from that path.

In `--global` mode, you must provide:
1. An explicit agent flag (`--agy`, `--claude`, `--opencode`, `--cursor`, `--pi`), or
2. An explicit file path (`--file <path>`), or
3. Run in an interactive terminal, where an interactive multi-select menu (`inquire`) will prompt you to select targets or enter a custom path.

Running `skm rule apply --global` without flags in non-interactive mode aborts immediately with an error:

```text
error: --global requires specifying a target agent (--agy, --claude, --opencode, --cursor, --pi) or --file <path>
```

### Agent Path Matrix

| Agent Assistant       | Flag                  | Local Project Target        | Global Target (Home)             |
| --------------------- | --------------------- | --------------------------- | -------------------------------- |
| **Antigravity / Gemini** | `--agy` / `--antigravity` | `.gemini/GEMINI.md`     | `~/.gemini/GEMINI.md`            |
| **Claude Code**       | `--claude`            | `CLAUDE.md`                 | `~/.claude/CLAUDE.md`            |
| **OpenCode**          | `--opencode`          | `AGENTS.md`                 | `~/.config/opencode/AGENTS.md`   |
| **Cursor**            | `--cursor`            | `.cursor/AGENTS.md`         | `~/.cursor/AGENTS.md`            |
| **Pi**                | `--pi`                | `.pi/AGENTS.md`             | `~/.pi/agent/AGENTS.md`          |
| **Custom Path**       | `--file <path>`       | `<path>`                    | `<path>`                         |
| **Default (no flag)** | *(none)*              | `AGENTS.md` (project root)  | *(Forbidden without flags/menu)* |

---

## Central Registry Structure

Applied rules are recorded in `$XDG_STATE_HOME/skm/projects.toml` (typically `~/.local/state/skm/projects.toml`) under each project record or the global record:

```toml
[projects."/path/to/project"]
[[projects."/path/to/project".rule]]
name = "default"
source_rel = "AGENTS.md"
target_file = "AGENTS.md"
applied_hash = "blake3:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
source_hash = "blake3:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
applied_at = 1714300000

[global]
[[global.rule]]
name = "default"
source_rel = "AGENTS.md"
target_file = "~/.gemini/GEMINI.md"
applied_hash = "blake3:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
source_hash = "blake3:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
applied_at = 1714300000
```
