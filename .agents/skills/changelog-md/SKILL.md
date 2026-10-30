---
name: changelog-md
description: Maintain CHANGELOG.md following Keep a Changelog + SemVer. Use when asked to
  "add changelog", "agregar CHANGELOG", "update the changelog", "release notes", "cut a release",
  or before tagging a release. Never writes entries without explicit approval; unaccepted
  candidates are parked in `.agents/PLAN.md` under `## Pending CHANGELOG`.
---

# CHANGELOG.md

## Hard rules

1. **Alpha rule** — no published version or release tag yet → write NOTHING. Pre-first-release
   development is alpha and is not logged.
2. **Approval rule** — write only after the user explicitly accepts ("add changelog",
   "agregar CHANGELOG", "sí, agrégalo"). Never add entries proactively.
3. **Unreleased rule** — accepted changes go under `## [Unreleased]` until a version ships.
4. **Release rule** — when a version ships, move `[Unreleased]` entries into
   `## [x.y.z] - YYYY-MM-DD` and leave a fresh empty `## [Unreleased]`.
5. **Version rule** — the user always provides the version. Never propose or bump it.
6. **Pending rule** — post-alpha, a change the user does not accept (or never explicitly asked
   for) is parked in `.agents/PLAN.md` under the exclusive section `## Pending CHANGELOG`.
   It leaves the plan when the user accepts it (moves into `CHANGELOG.md`) or drops it entirely.
   Alpha → park nothing.

## Skeleton

Template only — in a real file, drop the empty sections.

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- <one bullet per change>

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.1.0] - 2026-10-29
```

- `## [Unreleased]` is always on top, never dated.
- Released versions go below it, newest first, same section order.
- Pre-releases are written as `## [0.1.0-alpha.1] - YYYY-MM-DD`.

## Sections

| Section      | Use for                                              | Example                                   |
| ------------ | ---------------------------------------------------- | ----------------------------------------- |
| `Added`      | New features, components, capability                 | `- add ColorPicker component`             |
| `Changed`    | Existing behavior, output, or format changed         | `- change deploy target to staging`       |
| `Deprecated` | Still works, scheduled for removal                   | `- deprecate legacy theme prop`           |
| `Removed`    | Deleted features or files                            | `- remove unused build step`              |
| `Fixed`      | Bug fixes                                            | `- fix crash on missing changelog`        |
| `Security`   | Vulnerabilities or hardening                         | `- update yaml parser`                    |

Order is fixed; empty sections are deleted, never left as headers. Fix that is also a
vulnerability goes in `Security`.

Bullet style: one line per change, English, imperative mood, specific, no trailing period,
no commit hash, no author. Breaking change: append ` **BREAKING**`.

## Detect state (run first)

| Check                                | Result                    |
| ------------------------------------ | ------------------------- |
| `git tag --list` / `gh release list` | no tags → alpha → stop    |
| tags exist, file missing             | create skeleton only      |
| `[Unreleased]` has entries           | pending, not yet released |

## Procedure — add a change

1. Detect state. No release yet → stop and explain why.
2. Read `CHANGELOG.md`.
3. Take the change from the user's message (or `git diff` / `git log` since the last tag).
4. Pick the section from the table; ask when ambiguous.
5. Insert one bullet under `## [Unreleased]` → section. Touch nothing else.
6. Report the exact line added.

## Procedure — release version

1. Ask for the version (`0.1.0-alpha.1` for pre-releases). Never guess.
2. Version already present in the file → stop.
3. Move `[Unreleased]` content into `## [version] - <today>`.
4. Recreate an empty `## [Unreleased]` on top.
5. Move only accepted entries.

## Suggesting (without writing)

When a feature is finished and the project is past alpha, close the reply with **one** line:

`CHANGELOG candidate: Added — <entry in English>`

Nothing else. The user accepts with "add changelog"; only then write it. Never suggest in alpha,
never more than one line, never repeat a suggestion already pending.

## Pending changes — `.agents/PLAN.md`

Anything the user does not accept is never lost: it waits in the project plan, away from
`CHANGELOG.md`.

- Exclusive section, exact title `## Pending CHANGELOG` (English), one bullet per change.
- Create `.agents/PLAN.md` (and the section) if missing; never touch other sections of the file.
- Entry format, same bullet style as the changelog:

  `- <Section> — <one line, English, imperative, no trailing period>`

  e.g. `- Added — add ColorPicker component`.

- Add when: the suggestion was rejected, ignored, or never explicitly accepted (post-alpha only).
- Remove when: the user accepts it → move the bullet into `CHANGELOG.md` (Unreleased → section)
  and delete the line from the plan; or the user drops the change/feature entirely → delete it.
- No duplicates: if the same change is already listed, do not add it again.
- Alpha → park nothing; skip this section entirely.

## Anti-patterns

- Do NOT write entries in alpha (no released version yet).
- Do NOT create empty sections or empty version headers.
- Do NOT bump or propose versions, or invent dates.
- Do NOT rewrite, reorder, or reformat existing entries.
- Do NOT derive entries from commits without user approval.
- Do NOT write outside `CHANGELOG.md` and the `## Pending CHANGELOG` section of `.agents/PLAN.md`.
