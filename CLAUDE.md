# CLAUDE.md – Backups Plus

> **Keep this file current.** Whenever you change something that matters here (architecture, names, build steps,
> conventions, release state, open items), update this file in the same change. It must always be enough to continue
> working on the project without any other context.

## What this is

**Backups Plus** is an extension for the [Calagopus](https://github.com/calagopus/panel) game server panel
(≥ 1.2.11, `:heavy` image). Each server gets freely configurable backup **routines** with rotation and cleanup.

- Repo: <https://github.com/Luis-1805/calagopus-backup-plus> (branch `main`). A rename to `calagopus-backups-plus` is
  open. Ask the owner before doing it.
- The owner runs it on **one own panel only**, so breaking migrations are acceptable when they come with a migration
  path.
- Distributed as `dev_backupsplus_backups.c7s.zip` (GitHub release asset), uploaded under **Admin → Extensions**.
- License: MIT. The full text is also in `Metadata.toml` (`license_text`), which the panel shows on install.

## Working with the owner

- Talk to the owner in **German**. The repo (README, docs, code identifiers, release notes, commit messages) is in **English**.
  Code comments are mostly German, which is fine.
- **Always ask before** committing, pushing, creating a GitHub release or renaming the repo.
- Don't add backward compatibility in API responses (e.g. keeping old field names) unless asked; the owner rejected it
  explicitly ("Das sollte ohne gehen").
- Prefer small, complete changes. Say what was not verified.

## Names and identifiers (since 2.4.0)

| What | Value |
| --- | --- |
| Display name | Backups Plus |
| Package ID (`Metadata.toml`) | `dev.backupsplus.backups` |
| Rust crate | `dev_backupsplus_backups` (`backend/Cargo.toml`) |
| Frontend class | `DevBackupsplusBackupsExtension` (`frontend/src/index.ts`) |
| Tables | `backups_plus_policies`, `backups_plus_routines`, `backups_plus_backups`, `backups_plus_log` |
| Server permissions | `backups-plus.read`, `backups-plus.manage` (group `backups-plus`) |
| Client API | `/api/client/servers/{server}/backups-plus` |
| Admin API | `/api/admin/extensions/dev.backupsplus.backups` (admin perms `extensions.read` / `extensions.manage`) |
| Cron task | `backups_plus_tick`, every **5 s** (`*/5 * * * * *`, UTC, primary instance only) |
| Panel backup group | `Backups Plus (auto)` |
| Settings keys | `dev.backupsplus.backups::<field>` in the panel's `settings` table |

Up to 2.3.0 it was **"Backup Plus"**, `dev.backupplus.gfsbackups`, tables `backup_plus_*`, perms `backup-plus.*`.
Old names still exist on purpose in: migrations 1–3 (`backup_plus_*`), the rename migration and
`backend/src/legacy.rs`.

## Layout

```text
Metadata.toml                 package ID, name, panel_version, license text
backend/                      Rust crate (built inside the panel workspace)
  src/lib.rs                  extension entry: routes, cron task, permissions, settings deserializer, version test
  src/engine.rs               tick, run_server, create/rename/delete backups, throttling, reschedule()
  src/retention.rs            pure rotation planner (also drives the UI's "Next action" preview)
  src/schedule.rs             cron parsing (croner, 6 fields, 5 also ok), time zones, min-interval helpers
  src/validation.rs           routine validation (names, keep, cron, min interval, source chains, backup limit)
  src/settings.rs             global admin settings (stored via panel extension settings)
  src/models.rs               SQL for own tables (sqlx, runtime queries; SQLX_OFFLINE builds)
  src/templates.rs            built-in templates (gfs-kompakt, gfs-erweitert, taeglich-7, alle-6-stunden)
  src/i18n.rs                 backend messages EN/DE (Msg with args, stored as JSON in log/last_error)
  src/legacy.rs               pauses ticks while the old extension is still loaded – REMOVE in 2.5
  src/routes/{server,admin}.rs
  src/simulation_tests.rs     long-running rotation simulations
frontend/                     React + Mantine, compiled into the panel's frontend
  src/index.ts                registers server route `/backups-plus` and admin page
  src/ServerPage.tsx          "Auto-Backups" tab
  src/AdminPage.tsx           admin settings, templates, bulk apply
  src/components/RoutineListEditor.tsx   routine cards (collapsible, state in localStorage)
  src/components/DurationInput.tsx       number + unit (s/min/h/d) → seconds
  src/lib/schemas.ts          zod schemas (API is snake_case, transformed to camelCase), cron presets, formatters
  src/translations.ts         English source strings
  public/translations/de/dev.backupsplus.backups.json   German strings (same keys/placeholders as EN)
  src/version.ts              FRONTEND_VERSION – must equal backend/Cargo.toml version (backend test checks it)
migrations/<timestamp>_<name>/{up,down}.sql
docs/                         English user docs (README index, admin, api, concepts, routines, server-tab, …)
.github/                      issue templates, banner.svg
```

## Core concepts

- **Policy** (per server): enabled, delete_failed, backup group, last run/error, optional `min_interval_seconds`
  override (admin only).
- **Routine**: `create` (makes new backups on its cron) or `promote`/"take over" (adopts a backup that falls out of its
  source routine when it is due, and can rename it with `rename_template`). It keeps `keep_count` newest and has an
  optional `max_age_hours`. Chains must end at a create routine (no cycles).
- **Rotation** (`retention::plan`) is a pure function from routines + backups to an action per backup
  (keep/move/delete/pinned/running/failed_kept/unassigned). It runs on every backup run and every 5 minutes for all
  enabled servers.
- **Name placeholders**: `{routine}`, `{date}`, `{day}`, `{time}`, `{server}` (server name), and for renames
  `{original}`. Dates are in the server time zone (`engine::render_template`).
- **Time zone**: server → node → admin default → UTC (`schedule::resolve_timezone`).
- **Minimum interval** (global `min_interval_seconds`, 5 s – 7 d, default 3600; per-server override in the policy):
  - Validation rejects create routines whose cron gap is below it (tolerance min/24, max 1 h, for DST).
  - It applies immediately to existing routines. If a cron is `too_frequent`, scheduled runs before
    `last_run + min - 10s` are skipped and `next_run` is moved (`schedule::next_allowed`). The API flags these
    routines as `throttled`.
  - Lowering the value calls `engine::reschedule` (admin settings PUT, override PUT) to pull held-back runs forward.
- **Tick** every 5 s. "Max. servers per minute" is enforced with a per-minute budget (`take_minute_budget`).
  Rotation for all servers and log trim run only when `minute % 5 == 0 && second < 5`.
- A running backup makes a scheduled routine **wait** (it stays due) until its following slot. Only the first waiting
  tick is logged.

## Migrations – rules

- The panel tracks applied migrations in its `migrations` table. An extension migration's ID is a uuid made from xxh3_128 of
  `"{identifier}:{name}:{date_millis}"`, where the identifier is the package ID with `.` → `_`. **Changing the package ID re-runs
  all migrations**, so every migration must be **idempotent** (IF EXISTS / IF NOT EXISTS, column-existence guards
  in DO blocks).
- Never delete old migrations. They are the chain for fresh installs. Edits to applied migrations are only OK as
  idempotency guards.
- `20261125200000_server_interval_override` uses the old table name, because it runs before the rename.
- `20261125210000_rename_to_backups_plus` renames tables/indexes and moves the settings keys,
  `::extension_permissions`, `::disabled_extensions` and all `*permissions` ARRAY columns. It also renames the
  backup group and deletes the old package's migration records (hard-coded IDs). It must stay permanently.
- Upgrade from ≤ 2.3 on a panel:
  1. Delete the old extension with **"remove migrations" OFF**. ON runs the down.sql files and drops all data.
  2. Upload the new zip and rebuild once.
- Panel migrations run before settings load. The UI "remove migrations" switch runs `down.sql`.

## Build, test, release

Builds don't work natively on Windows. Use **WSL Ubuntu** (`wsl -d Ubuntu -u root`, Rust in `/root/.cargo`).

- **Backend:**
  1. Copy the repo into a panel checkout (`/root/panel`, a clone of calagopus/panel) at
     `backend-extensions/dev_backupsplus_backups/`. Copy `backend/.`, `Metadata.toml`, `migrations` and `frontend` there.
  2. Run:
     ```bash
     SQLX_OFFLINE=true cargo clippy -p dev_backupsplus_backups --tests
     SQLX_OFFLINE=true cargo test   -p dev_backupsplus_backups
     ```
  3. To check against the released panel, use a clone of tag `release-1.2.11`.
- **Migrations:** test with the local PostgreSQL in WSL.
  1. Create stub panel tables (`servers`, `server_backup_groups`, `server_backups`, `settings`, `migrations`, a
     table with a `permissions` array).
  2. Apply the old migrations from `git archive <old tag>`, then the new ones **once**.
  3. Check the data, the down chain and a fresh install.
- **Frontend:** use a panel 1.2.11 source tree with `frontend/node_modules` installed.
  1. Copy `frontend/{src,public,package.json}` to `frontend/extensions/dev_backupsplus_backups/`.
  2. Run `npx tsc -p tsconfig.json`, `npx oxfmt <ext>/src` (panel formatter, copy the result back),
     `npx oxlint <ext>/src` and `npx vite build`.
  3. Check that EN and DE have identical keys and placeholders. The panel's `diff-translations.mts` breaks on Windows
     paths, so use a small tsx script.
- **Translations:** avoid literal `{…}` in UI strings unless it is a real parameter, because the panel interpolates
  them.
- **Version bump:** `backend/Cargo.toml` `version` and `frontend/src/version.ts` together. The server page compares the
  frontend and installed versions and warns about stale cached UIs (Ctrl+Shift+R / purge the Cloudflare cache).
- **Pack:** zip `Metadata.toml backend/Cargo.toml backend/src frontend/package.json frontend/src frontend/public
  migrations` (no node_modules) as `dev_backupsplus_backups.c7s.zip`. `*.c7s.zip` is gitignored.
- **Release:**
  1. Commit, then tag `vX.Y.Z`.
  2. Run `gh release create` with notes in the style of earlier releases (New / Changed / Installation / Tested) and
     the zip as the asset.
  3. Only do this after the owner says yes.

## Current state (update this!)

- Released: up to **v2.3.0** (as "Backup Plus", old package ID).
- **The next release (planned 2.4.0) is committed on `main` but not pushed or released.** It includes the rename and
  package-ID migration, min interval in seconds with a per-server override, immediate throttling, collapsible
  routines, the full license text and the 5 s tick. The version in the repo is still 2.3.0. The owner's push workflow
  bumps it, so don't bump it in feature commits. Release notes are drafted outside the repo.
- Open items:
  - `{server}` placeholder reported as "not working" by the owner. Code and test show it works, so it was not reproduced.
    Ask for the exact template and resulting name.
  - Repo rename to `calagopus-backups-plus` is pending. The docs and README still link to `calagopus-backup-plus`.
  - Remove `backend/src/legacy.rs` (and `mod legacy` plus its call in `engine::tick`) in 2.5. Keep the SQL migrations.
  - 2.4.0 has not been tested on a live panel yet.
