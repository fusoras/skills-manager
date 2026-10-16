# Packs in `skm`

A **pack** is a named collection of skills that install together. Packs live exclusively in the registered store under `<store>/packs/<name>.toml`. They are **never copied into target projects** as files, maintaining the zero-pollution guarantee.

---

## Pack Definition (`<store>/packs/<name>.toml`)

```toml
# <store>/packs/frontend.toml
name = "frontend"                     # Optional: defaults to the file stem ("frontend")
description = "Web development tools" # Optional: human-readable description
skills = [
    "react",
    "tailwind",
    "seo",
]
```

---

## Creating a Pack

1. Create a `.toml` file inside the `packs/` directory of your store (e.g. `<store>/packs/web.toml`):
   ```toml
   # <store>/packs/web.toml
   name = "web"
   description = "Web development tools"
   skills = [
       "git-commit",
       "react",
   ]
   ```

2. Alternatively, create it directly using `skm pack edit`:
   ```sh
   skm pack edit web --add git-commit react
   ```

3. Verify that `skm` detects it:
   ```sh
   skm pack list
   skm pack show web
   ```

---

## Editing Packs (`skm pack edit`)

`skm pack edit` provides two convenient workflows:

### 1. Direct Command-Line Modification (no editor needed)

Add one or more skills to a pack:
```sh
skm pack edit web --add tailwind seo
```
- Adds the skills to the `skills = [...]` array in the TOML file.
- Preserves existing comments and descriptions.
- Validates that the skills exist in the store before saving.
- Creates the pack file automatically if it did not exist yet.

Remove skills from a pack:
```sh
skm pack edit web --remove seo
```

### 2. Interactive Editing in Text Editor

Run without flags to open `<store>/packs/<name>.toml` in your editor (`$VISUAL` or `$EDITOR`, fallback to `nano` or `vim`):
```sh
skm pack edit web
```
If the pack does not exist yet, an initial template is created automatically before launching the editor.

---

## Installing Packs

Install all skills in a pack into the default target (`.agents/skills`):
```sh
skm add --pack web
# or:
skm pack add web
```

### Multi-Agent Targets

You can install packs into multiple agent environments simultaneously using target flags:
```sh
skm pack add web --opencode --pi --claude
```
Available flags:
- `--agent` (or `--agents`): `.agents/skills` (**default**)
- `--opencode`: `.opencode/skills`
- `--claude`: `.claude/skills`
- `--codex`: `.codex/skills`
- `--pi`: `.pi/skills`
- `--cursor`: `.cursor/skills`
- `--dir <path>`: custom target path

Updating a project after adding skills to a pack in the store:
```sh
skm add --pack web
# or simply run:
skm pack add web
```

---

## Reference Counting & Safe Removal

`skm` protects skills from accidental deletion when multiple packs or direct installations overlap:

```
Pack A: [alpha, beta]
Pack B: [beta, gamma]
```

- When removing Pack A:
  ```sh
  skm pack remove A
  ```
  - `alpha` is deleted from disk and unregistered.
  - `beta` is **retained** because Pack B still requires it:
    ```text
    Removed alpha (.agents/skills/alpha)
    Retained beta (.agents/skills/beta; still required by pack 'B')
    ```
- When Pack B is subsequently removed, `beta` and `gamma` are deleted.
- If a skill was installed directly (`skm add beta`) and also belongs to a pack, removing the pack retains the skill (`still required by direct install`).
- Trying to directly remove a member skill (`skm remove beta`) without `--force` fails to prevent accidental deletion of pack dependencies.

---

## Local Detach Safety

If you edit a skill installed via a pack locally inside your project:
1. Running `skm sync` detects the local modifications and detaches the skill as a local fork (`detached = true`).
2. The skill is preserved untouched and never overwritten by subsequent syncs.
3. The pack association is **not lost** (`skm pack status` still tracks the pack).
4. Running `skm pack remove` will refuse to delete the locally modified copy unless `--force` is explicitly provided.

---

## Commands Summary

| Command | Description |
| --- | --- |
| `skm pack [list]` / `skm list --packs` | List all available packs in the store |
| `skm pack show <name>` | Inspect pack description and member skills |
| `skm pack edit <name>` | Open pack in `$EDITOR` (or `$VISUAL`) |
| `skm pack edit <name> --add <skills>...` | Add skills directly from CLI |
| `skm pack edit <name> --remove <skills>...` | Remove skills directly from CLI |
| `skm pack status` / `skm status --packs` | Show installed packs in the current project |
| `skm pack add <pack>... [flags]` | Install packs into target project |
| `skm pack remove <pack>... [flags]` | Remove packs with reference counting |
