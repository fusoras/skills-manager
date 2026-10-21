# Configuration (`config.toml`)

`skm` uses a single global configuration file to track your global skill store location, default installation targets, agent mappings, and background sync options.

---

## File Location

The configuration file is located at:
- **Linux / macOS (XDG)**: `~/.config/skm/config.toml` (or `$XDG_CONFIG_HOME/skm/config.toml`)
- **Windows**: `%APPDATA%\skm\config.toml`

### CLI Commands

You can inspect your configuration without opening the file manually:

```sh
# Print the absolute path of the configuration file
skm config path

# Print current configuration as TOML (shows in-memory defaults if file doesn't exist)
skm config show
```

---

## TOML Specification & Defaults

```toml
# Path to the global skills store (optional until registered via `skm store add`)
store = "~/skills-store"

# Default relative directory where skills are installed in target projects
default_target = ".agents/skills"

# Default directory where global skills are installed
default_global_target = "~/.agents/skills"

# Mapping of agent names to custom skill directories in projects
[agents]
antigravity = ".gemini/skills"
opencode = ".opencode/skills"

# Mapping of agent names to user-level global skill directories
[global_agents]
antigravity = "~/.gemini/skills"
claude = "~/.claude/skills"
codex = "~/.codex/skills"
cursor = "~/.cursor/skills"
opencode = "~/.config/opencode/skills"
pi = "~/.pi/agent/skills"

# Watch mode options
[watch]
debounce_ms = 500
```

---

## Configuration Fields

### `store`
- **Type**: `string` (path)
- **Default**: `None`
- **Description**: Path to the unique, global skill store directory.
- **Normalization**:
  - Paths inside `$HOME` are stored as `~/...` for portability and human readability.
  - Paths outside `$HOME` are stored as absolute paths.
  - Relative paths passed to `skm store add` are resolved against the current working directory before saving.
  - The tilde (`~`) is expanded dynamically whenever commands access the store.

### `default_target`
- **Type**: `string`
- **Default**: `".agents/skills"`
- **Description**: The default relative directory inside any target project where `skm add`, `skm pack add`, and `skm sync` install skills when no explicit agent target flags are provided.

### `default_global_target`
- **Type**: `string`
- **Default**: `"~/.agents/skills"`
- **Description**: The default directory where `skm add --global` installs skills when no specific agent target flag is provided.

### `[agents]`
- **Type**: Table / Map (`string -> string`)
- **Default**:
  ```toml
  antigravity = ".gemini/skills"
  opencode = ".opencode/skills"
  ```
- **Description**: Overrides or custom target directory locations for specific agent environments in local projects. If an agent name is not explicitly defined in `[agents]`, `skm` defaults to `.<agent>/skills` (e.g. `--claude` installs into `.claude/skills`).

### `[global_agents]`
- **Type**: Table / Map (`string -> string`)
- **Default**:
  ```toml
  antigravity = "~/.gemini/skills"
  claude = "~/.claude/skills"
  codex = "~/.codex/skills"
  cursor = "~/.cursor/skills"
  opencode = "~/.config/opencode/skills"
  pi = "~/.pi/agent/skills"
  ```
- **Description**: Global directory locations for agent skills across the user's home environment. When invoking commands with `--global` (e.g. `skm add <skill> --global --opencode` or `skm remove <skill> --global --antigravity`), `skm` resolves the agent's target from this table.

### `[watch]`
- **Type**: Table
- **Fields**:
  - `debounce_ms` (integer, default: `500`): Debounce duration in milliseconds for filesystem event coalescing during watch / sync modes.

---

## Comment & Formatting Preservation

When modifying configuration using commands like `skm store add <path>`, `skm` parses the file with `toml_edit`:
- Custom comments (`# ...`) are preserved intact.
- Existing whitespace, indentation, and key order are retained.
- Missing files are created with canonical defaults only when mutations are performed.
