# 🖥️ The Auto-Backups tab

[← Guide overview](README.md)

Each server gets an **Auto-Backups** entry in its sidebar (visible with the `backup-plus.read` permission). The page is
built from top to bottom like this:

```text
┌──────────────────────────────────────────────────────────────── [⚡ Back up now] ┐
│ Alerts (only when something needs attention)                                     │
├────────────┬───────────────────┬──────────────┬──────────────────────────────────┤
│ Status     │ Managed backups   │ Backup limit │ Routines                         │
├────────────┴───────────────────┴──────────────┴──────────────────────────────────┤
│ Setup: template · automatic backups on/off · delete failed backups               │
├───────────────────────────────────────────────────────────────────────────────────┤
│ Routines: editor with one card per routine · Save / Discard changes              │
├───────────────────────────────────────────────────────────────────────────────────┤
│ Managed backups: table with next action and ⋮ menu                               │
├───────────────────────────────────────────────────────────────────────────────────┤
│ Log                                                                               │
└───────────────────────────────────────────────────────────────────────────────────┘
```

The page refreshes itself every 30 seconds. Unsaved changes in the editor are kept while it refreshes.

- [Back up now](#back-up-now)
- [Alerts](#alerts)
- [Status cards](#status-cards)
- [Setup](#setup)
- [Routines](#routines)
- [Managed backups](#managed-backups)
- [Log](#log)
- [Who sees what](#who-sees-what)

## Back up now

The button in the page header creates a backup immediately using the first active *create* routine, followed by a
rotation. It is disabled when there is no create routine. To run a specific routine, use **Run now** on its card.
See [Run now](routines.md#run-now).

## Alerts

| Alert | When it appears | What to do |
| --- | --- | --- |
| 🟡 **Disabled globally** | An admin switched off Backup Plus for the whole panel | Nothing to do on the server; ask the admin |
| 🔴 **Last run failed** | The last automatic or manual run ended with an error | Read the message; see [Troubleshooting](troubleshooting.md) |
| 🟠 **Routine "…"** | The last run of that routine failed | Same as above; the message is specific to the routine |
| 🔴 **Backup limit too low** (in *Setup*) | The server's backup limit is lower than the total *Keep* of your routines | Ask an admin to raise the limit to the recommended value |
| 🔵 **Custom routines disabled** (in *Setup*) | The admin only allows templates | Apply one of the templates |
| 🔵 **No time zone configured** | Neither the server nor its node has a time zone, so schedules run in UTC | Set the server's time zone under **Settings → Time zone** |

Errors are cleared automatically by the next successful run.

## Status cards

| Card | Shows |
| --- | --- |
| **Status** | *Active* or *Paused*, the time of the last run, the time zone in use and where it comes from (server settings, node default, Backup Plus default or fallback) — see [Time zones](concepts.md#time-zones) |
| **Managed backups** | How many backups Backup Plus manages right now (not counting those about to be deleted) and the storage they use |
| **Backup limit** | All backups of the server (including manual ones) / the server's limit — red if the limit is too low |
| **Routines** | One line per routine: `backups/keep`, name and next run (*soon* = due now, *paused* = routine paused) |

## Setup

*(requires `backup-plus.manage`)*

**Template + Apply template** — replaces all routines of the server with the selected template and switches automatic
backups on. If the server already has routines you are asked to confirm. Existing backups are **kept**; backups of
routines that no longer exist are shown as *Not managed*. See [Templates](templates.md).

**Automatic backups active** — the server-wide switch. Off pauses all routines (no new backups, no rotation). Switching
it back on makes every routine due immediately. Changes here are applied with the **Save** button below the editor.

**Delete failed backups automatically** — when a backup fails (e.g. the node was offline), it is deleted on the next
rotation. Switch off to keep failed backups for troubleshooting; they are then shown as *Failed*.

## Routines

The routine editor shows one card per routine. All fields are explained in the
[routine reference](routines.md). On each saved routine card you additionally see:

```text
Backups: 3/3 · last run: 24.11.26, 13:00 · next: 24.11.26, 14:00      [Run now]
```

- **Backups x/y** — how many backups belong to the routine / how many it keeps
- **last run / next** — when the routine last ran and when it is due next (in your browser's local time)
- **Run now** — only for *create* routines

Buttons below the editor:

- **Add routine** — the first routine is a *create* routine ("Hourly"). Further routines start as *take over* routines
  from the first create routine on a weekly schedule, because that's what you usually want next. Change anything you
  like.
- **Save** — validates and applies all changes (including the two switches of the *Setup* card).
- **Discard changes** — reloads the saved state.

When the admin has disabled custom routines, the editor is read-only and you can only apply templates.

## Managed backups

All backups in the server's *Backup Plus (auto)* group, newest first.

| Column | Meaning |
| --- | --- |
| **Name** | Backup name; 📌 = pinned |
| **Routine** | The routine the backup belongs to (colour matches the routine number in the editor); `–` = none |
| **Created** | When the backup was started |
| **Size** | Size once finished |
| **Next action** | What the next rotation will do — see the table below |

| Next action | Meaning |
| --- | --- |
| 🟢 **Stays** | Kept by its routine |
| 🔵 **Becomes "…"** | Will move to the named routine |
| 🔴 **Will be deleted** | Hover to see why: *backup failed*, *older than the N newest backups of routine "…"*, or *older than the maximum age of routine "…"* |
| 📌 **Pinned** | Never deleted automatically |
| ⏳ **Running** | Still being created |
| ⚠️ **Failed** | Failed and kept (because *delete failed backups* is off) |
| ⚪ **Not managed** | No active routine; left alone |

The **⋮ menu** of each backup *(requires `backup-plus.manage`)*:

- **Pin (never delete) / Unpin** — pinned backups are excluded from rotation. Pinning uses the panel's own backup lock,
  so a pinned backup is also protected in the panel's normal backup page.
- **Assign routine** — move a backup to another routine, for example to keep a specific backup as your monthly one.
  The target routine's *Keep* rule applies immediately on the next rotation.
- **None (remove from rotation)** — the backup leaves Backup Plus' control. It stays in the group and is shown as
  *Not managed*.

> [!TIP]
> Restoring, downloading and deleting backups by hand works as usual on the panel's normal **Backups** page.

## Log

The last 50 entries for this server, newest first. Each entry has a level (*info*, *warning*, *error*). Typical entries:

| Entry | Meaning |
| --- | --- |
| *Started "…" for routine "Hourly"* | A backup was started |
| *"…" moved from "Hourly" to "Weekly"* | A take-over routine adopted a backup |
| *"…" renamed to "…"* | A take-over routine renamed the backup it adopted ([Rename taken-over backups](routines.md#rename-taken-over-backups)) |
| *"…" deleted (older than the 3 newest backups of routine "Hourly")* | Rotation removed a backup |
| *"Daily" waits for the running backup to finish* | Another automatic backup is still running; this routine starts right after it |
| *"Hourly" skipped: …* | A scheduled backup was skipped — see [Troubleshooting](troubleshooting.md#backup-was-skipped) |
| *"…" could not be deleted: …* | The panel couldn't delete a backup; it is retried on the next rotation |
| *Template "…" applied* | Someone applied a template |
| *"…" pinned / unpinned / manually assigned …* | Manual actions |

The log is shown in **your** panel language, regardless of who or what created the entry. The admin decides how many
entries are kept per server (default 100). All backup creations and deletions are additionally recorded in the panel's
normal **Activity** log with `"source": "backup-plus"`.

## Who sees what

| | Owner / admin | Subuser with `read` | Subuser with `read` + `manage` |
| --- | :---: | :---: | :---: |
| See the tab, status, routines, backups, log | ✅ | ✅ | ✅ |
| Back up now / Run now | ✅ | – | ✅ |
| Change setup and routines, apply templates | ✅ | – | ✅ ¹ |
| Pin, assign backups | ✅ | – | ✅ |

¹ Custom routines only if the admin allows them; templates can always be applied.

---

[← Routine reference](routines.md) · Next: [Templates →](templates.md)
