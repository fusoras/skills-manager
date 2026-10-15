---
name: git-workflow
description: Run git commands using the user's aliases for token savings and follow this project's branch strategy and modern git conventions. Use when asked to "check status", "switch branch", "create branch", "list branches", "commit", "push", "merge", "crear rama", "cambiar de rama", "revisar estado", or for any git operation in this repo. Load BEFORE running git commands.
---

# Git Workflow

Use together with the `git-commit` skill, and only for making commits: `git-workflow` for commands/flow, `git-commit` for the commit message.

## Aliases (always prefer the alias over the full command)

| Alias  | Full         |
| ------ | ------------ |
| `git b` | `git branch` |
| `git s` | `git switch` |
| `git st`| `git status` |

## Rules

- **Never** use `git checkout`. Equivalents: `git s <branch>` (switch branch),
  `git restore <file>` (discard changes), `git restore --staged <file>` (unstage).
- Development branch: `develop` or `feature/<name>`; **never** commit directly
  to `main`, merge only after tests pass. To start: `git s develop` →
  `git b feature/<name>` → `git s feature/<name>`.
- Procedure: `git st` + `git b` → `git diff` → stage only the intended files
  (never `git add .`; no secrets or build artifacts `target/`, `.codegraph/`)
  → commit using the `git-commit` skill.
- Forbidden: force-push/amend, `--no-verify`, modifying git config,
  committing/merging `main` without tests.
