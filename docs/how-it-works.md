# 🔍 How it works

[← Guide overview](README.md)

A look under the hood — for admins who want to know exactly what runs when, and for contributors.

- [The background task](#the-background-task)
- [A scheduled run, step by step](#a-scheduled-run-step-by-step)
- [The rotation algorithm](#the-rotation-algorithm)
- [Data model](#data-model)
- [Translations](#translations)
- [Project layout](#project-layout)
- [Building and testing](#building-and-testing)

## The background task

Backups Plus registers one cron task, `backups_plus_tick`, that runs **every minute** (UTC) on the panel's **primary**
instance (`APP_PRIMARY=true`).

Every tick:

1. If Backups Plus is disabled globally, stop.
2. Find all **active** servers that have an active *create* routine whose next run has passed, oldest last run first.
3. Process up to *Max. servers per minute* of them (see [below](#a-scheduled-run-step-by-step)). The rest wait for the
   next minute.

Every **5 minutes** additionally:

4. Run a rotation for every active server that wasn't processed in this tick. This is what sorts in backups that have
   finished since the last run, and what lets due take-over routines adopt them.
5. Trim each server's log to *Log entries per server*.

Errors in one server never stop the others; they are written to that server's log and shown on its page.

## A scheduled run, step by step

1. Load the server, its settings and routines. Create the *Backups Plus (auto)* group if it's missing.
2. **Rotate** (see below).
3. Pick the first due, active *create* routine.
4. **Safety checks** — skip (and log) if the server is suspended, installing, restoring or transferring. If an
   automatic backup of this server (started within the last 6 hours) is still running, the routine **waits**: it stays
   due and is tried again in the next tick, until the routine's following scheduled time — only then is the slot
   skipped. The first waiting tick writes a log entry.
5. Take the panel's per-server backup lock (the same one the panel uses for manual backups).
6. **Backup limit** — if the server is at its limit, rotate again while reserving one slot of this routine; if it's
   still full, fail with *backup limit reached*.
7. Create the backup through the panel (`ServerBackup::create`) with the routine's name template and ignored files. The
   panel then asks Wings to create it.
8. Assign the backup to the routine, write the activity log entry and the log line.
9. Set the routine's next run from its schedule — also if the run failed or was skipped, so a broken server never retries
   every minute. (A *waiting* routine is the exception: it keeps its due time, see step 4.)

Manual runs (**Back up now**, **Run now**) do steps 1–8 but leave the schedule untouched.

## The rotation algorithm

The rotation is a pure function: it takes routines and backups and returns an action for every backup. The same function
produces the *Next action* preview in the UI, so what you see is exactly what will happen.

```text
for each backup:
    still running                 → Running
    pinned                        → Pinned
    failed                        → Delete (or FailedKept if "delete failed" is off)
    routine missing or paused     → Unassigned
    otherwise                     → add to its routine's list

for each active routine, sources before the routines that take over from them:
    sort its list newest first
    keep = Keep (minus 1 if a slot is reserved for an upcoming backup)
    the first `keep` backups that are not older than Max. age  → Keep / Move (if they came from another routine)
    every other backup, newest first:
        if a take-over routine with this routine as source is due
           and hasn't received a backup in this pass        → move it to that routine's list
        else                                                 → Delete
```

Routines that received a backup are marked as done until their next scheduled time.

Deletion uses the panel's own backup deletion (`ServerBackup::delete`), so files on the node or in object storage are
removed the same way as when you delete a backup by hand. Locked backups are checked again right before deleting.

## Data model

| Table | Content |
| --- | --- |
| `backups_plus_policies` | One row per server: on/off, *delete failed*, the managed backup group, last run and last error |
| `backups_plus_routines` | Routines: name, schedule, type, source, keep, max. age, name template, ignored files, next/last run, last error |
| `backups_plus_backups` | Which routine a backup belongs to (references the panel's `server_backups`) |
| `backups_plus_log` | Per-server log; messages are stored as translation keys with parameters |

All tables reference the panel's `servers` table and are cleaned up automatically when a server is deleted. Global
settings and custom templates are stored with the panel's extension settings.

Migrations live in `migrations/` and run automatically on install and update. The 2.0 migration converts the 1.x
settings (fixed hourly/daily/weekly/monthly tiers) into equivalent routines.

## Translations

| Part | Where | How it's rendered |
| --- | --- | --- |
| Interface | `frontend/src/translations.ts` (English base) and `frontend/public/translations/<lang>/dev.backupsplus.backups.json` | The panel's translation system, in the viewer's language |
| Errors, validation, delete reasons, log, built-in templates | `backend/src/i18n.rs` | On the server, in the language of the user making the request |

The log stores keys and parameters instead of finished sentences, so the same entry is shown in English to one user and
in German to another. Entries from before 2.1 are shown as stored.

**Adding a language:** copy the German JSON file into a new language folder and translate the values (keep the
`{placeholders}`), then add the matching cases to the `template()` function in `i18n.rs` and to `Lang::from_code`.

## Project layout

```text
Metadata.toml                     extension metadata (package name, panel version)
backend/
  Cargo.toml
  src/
    lib.rs                        registers routes, background task, permissions, settings
    engine.rs                     scheduled/manual runs, creating and deleting backups
    retention.rs                  the rotation algorithm (pure, unit-tested)
    schedule.rs                   cron evaluation in the server's time zone
    templates.rs                  built-in templates, template → routines
    validation.rs                 rules checked when saving
    models.rs                     database access
    settings.rs                   global settings
    i18n.rs                       backend translations
    simulation_tests.rs           90-day simulations of the templates
    routes/server.rs, admin.rs    API endpoints
frontend/
  src/index.ts                    registers the server route, admin page and permission icon
  src/ServerPage.tsx              the Auto-Backups tab
  src/AdminPage.tsx               the admin configuration page
  src/components/RoutineListEditor.tsx
  src/translations.ts             English UI strings
  public/translations/de/…        German UI strings
migrations/                       SQL migrations
```

## Building and testing

Backups Plus is built like any Calagopus extension (see the
[panel's development guide](https://calagopus.com/docs/panel/extensions/dev-environment)). Place the repository
contents in `backend-extensions/dev_backupsplus_backups/` of a panel checkout, then:

```bash
# backend: compile, lint, test
SQLX_OFFLINE=true cargo clippy -p dev_backupsplus_backups --tests
SQLX_OFFLINE=true cargo test -p dev_backupsplus_backups

# frontend: type-check (from the panel's frontend/ directory)
pnpm typecheck

# package
panel-rs extensions export dev.backupsplus.backups
```

The test suite includes unit tests for rotation, schedules and translations, plus simulations that run the built-in
templates minute by minute over 90 and 220 days with real cron schedules and time zones.

---

[← API](api.md) · [Guide overview](README.md)
