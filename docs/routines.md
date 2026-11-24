# 🗂️ Routine reference

[← Guide overview](README.md)

Every field of a routine, what it does and which values are allowed.

- [Name](#name)
- [Type](#type)
- [Source](#source)
- [Schedule](#schedule)
- [Keep](#keep)
- [Maximum age](#maximum-age)
- [Active](#active)
- [Name template](#name-template)
- [Rename taken-over backups](#rename-taken-over-backups)
- [Ignored files](#ignored-files)
- [Order of routines](#order-of-routines)
- [Run now](#run-now)
- [Saving and validation rules](#saving-and-validation-rules)

## Name

| | |
| --- | --- |
| Allowed | 1–64 characters, unique per server (case-insensitive) |
| Used in | the editor, the backup list, the log and the `{routine}` placeholder of the name template |

Renaming a routine keeps all its backups and its schedule.

## Type

| Option | Behaviour |
| --- | --- |
| **Create a new backup** | On its schedule, a new panel backup of the server is created and assigned to this routine |
| **Take over a backup from another routine** | Creates nothing. When a backup falls out of the [source](#source) and this routine is due, the backup moves here instead of being deleted |

See [Core concepts](concepts.md#two-kinds-of-routines-create-and-take-over) for the idea behind the two types.

> [!NOTE]
> A server needs at least one *create* routine — otherwise no backups are ever made. Every take-over chain must
> start at a create routine.

## Source

Only for **take over** routines: the routine whose expiring backups this routine adopts. Any other routine of the server
can be the source, including another take-over routine (this is how [chains](concepts.md#chains) are built).

Rules checked when saving:

- a source must be selected,
- following the sources must end at a *create* routine,
- no cycles (A takes over from B, B takes over from A).

When you remove the source routine in the editor, routines that used it lose their source and you have to pick a new
one before saving.

## Schedule

A [cron expression](https://en.wikipedia.org/wiki/Cron) with **six fields** — the panel's format, which starts with
seconds:

```text
┌──────────── second (0–59)
│ ┌────────── minute (0–59)
│ │ ┌──────── hour (0–23)
│ │ │ ┌────── day of month (1–31)
│ │ │ │ ┌──── month (1–12)
│ │ │ │ │ ┌── day of week (0–6, 0 = Sunday)
│ │ │ │ │ │
0 0 * * * *
```

Classic five-field expressions (without seconds) are accepted as well.

| Meaning | Expression |
| --- | --- |
| Every 15 minutes (only if the admin allows it) | `0 */15 * * * *` |
| Every hour, on the hour | `0 0 * * * *` |
| Every hour at minute 15 | `0 15 * * * *` |
| Every 2 / 6 / 12 hours | `0 0 */2 * * *` · `0 0 */6 * * *` · `0 0 */12 * * *` |
| Every day at 04:00 | `0 0 4 * * *` |
| Twice a day (03:00 and 15:00) | `0 0 3,15 * * *` |
| Weekdays at 22:30 | `0 30 22 * * 1-5` |
| Every Monday at 00:00 | `0 0 0 * * 1` |
| Every Sunday at 03:00 | `0 0 3 * * 0` |
| 1st of every month at 00:00 | `0 0 0 1 * *` |
| 1st of January | `0 0 0 1 1 *` |

The editor helps you:

- **Quick select** fills in common schedules.
- Clicking the field opens a small editor with one box per field.
- Below the field you see the schedule in plain language and the **next run**.

**What the schedule means for each type**

- *Create*: when a backup is made.
- *Take over*: from when on the routine is **ready** to adopt a backup. It adopts the next backup that falls out of its
  source after that time, then waits for its next scheduled time.

**Time zone:** the server's time zone (fallback UTC). See [Time zones](concepts.md#time-zones).

**Minimum interval:** for *create* routines, two consecutive runs must be at least the admin's *minimum interval*
apart (default: 60 minutes; the admin can allow down to 5 minutes). This prevents schedules like "every minute" by
accident. Take-over routines are not limited because they don't create backups. Quick select offers *Every 15 minutes*
and *Every 30 minutes* — they're only accepted if the admin allows intervals that short.

## Keep

| | |
| --- | --- |
| Allowed | 1–500 |
| Meaning | How many of the routine's newest successful backups are kept |

The sum of *Keep* over all routines must not exceed:

- the admin's *max. backups kept per server* (default 50), and
- the server's backup limit.

## Maximum age

| | |
| --- | --- |
| Allowed | 0–87,600 hours (10 years); **0 = unlimited** |
| Meaning | Backups older than this fall out of the routine, even if *Keep* would allow more |

Useful when backups should never be older than a certain time, for example "every 6 hours, but nothing older than
2 days". A backup that falls out because of its age can still be taken over by a due take-over routine.

## Active

Pauses or resumes a single routine.

- A paused **create** routine makes no new backups.
- A paused routine's backups are **frozen**: they are shown as *Not managed* and are neither moved nor deleted until you
  resume it.
- Pausing a routine that others take over from simply means nothing new arrives for them.

To pause everything on a server at once, use **Automatic backups active** in the
[Setup card](server-tab.md#setup) instead.

## Name template

Only for **create** routines: the name of new backups. Placeholders:

| Placeholder | Example | Meaning |
| --- | --- | --- |
| `{routine}` | `Hourly` | Name of the routine |
| `{date}` | `2026-11-24 13:00` | Date and time (server time zone) |
| `{day}` | `2026-11-24` | Date only |
| `{time}` | `13:00` | Time only |
| `{server}` | `Survival` | Name of the server |

Default: `Auto {routine} {date}` → *Auto Hourly 2026-11-24 13:00*. Up to 200 characters; the resulting name is cut at
255 characters. If the template results in an empty name, the panel's default backup name is used.

> [!NOTE]
> By default, backups keep their name when they move to another routine — a backup taken over by *Weekly* is still
> called *Auto Hourly …*. The **Routine** column in the backup list always shows where it belongs now. To give it a
> fitting name, use [Rename taken-over backups](#rename-taken-over-backups).

## Rename taken-over backups

Only for **take over** routines, optional. The field **Name template for taken-over backups** sits at the bottom of
the routine card — in the same place where create routines have their *Name template*. If you fill it in, a backup
gets a new name the moment this routine takes it over (also when you assign it to the routine by hand).

| | |
| --- | --- |
| Empty (default) | the backup keeps its current name |
| Filled in, e.g. `{routine} {date}` | → *Weekly 2026-11-24 13:00* |
| Allowed | up to 200 characters; the result is cut at 255 |

Placeholders:

| Placeholder | Meaning |
| --- | --- |
| `{routine}` | Name of **this** routine (the one taking the backup over) |
| `{date}`, `{day}`, `{time}` | When the backup was **originally created** — not when it was taken over — in the server's time zone |
| `{server}` | Name of the server |
| `{original}` | The backup's name right before renaming |

Examples:

| Template | Result for a backup created on 24 Nov 13:00 as *Auto Hourly 2026-11-24 13:00* |
| --- | --- |
| `{routine} {date}` | *Weekly 2026-11-24 13:00* |
| `{server} – {routine} ({day})` | *Survival – Weekly (2026-11-24)* |
| `{original} [kept as {routine}]` | *Auto Hourly 2026-11-24 13:00 [kept as Weekly]* |

In a [chain](concepts.md#chains), each routine can rename again: e.g. *Daily* renames to `Daily {day}`, and *Weekly*
later renames the same backup to `Weekly {day}`. `{original}` always refers to the name right before the current
renaming. Renaming uses the panel's normal backup rename, so the new name also shows on the panel's **Backups** page.
The log records every rename (*"…" renamed to "…"*).

> [!TIP]
> If you don't want backups renamed at all, leave `{routine}` out of the create routine's name template (e.g.
> `{server} {date}`). Then no name ever becomes misleading.

## Ignored files

Only for **create** routines: files and folders that are left out of the backup. One rule per line, same syntax as the
panel's normal backup dialog (`.gitignore` style):

```text
logs/
*.tmp
```

Empty lines are ignored. Up to 100 rules, each up to 255 characters.

## Order of routines

Use the ▲/▼ buttons to reorder routines. The order matters in two places:

1. When several **create** routines are due at the same moment, the first one in the list runs first. Only one
   automatic backup per server runs at a time, so the others **wait** until it has finished and then run one after
   another (the log shows *"…" waits for the running backup to finish*). A waiting routine keeps its slot until the
   routine's *next* scheduled time; only if the running backup takes even longer is that slot skipped.

   > [!TIP]
   > Usually you don't need two create routines at all — a single create routine plus take-over routines covers most
   > strategies, and creates fewer backups.
2. When several **take-over** routines with the same source are due, the first one gets the first backup that falls
   out.

## Run now

Saved *create* routines have a **Run now** button next to their status. It creates a backup for that routine
immediately and runs a rotation. The header button **Back up now** does the same for the first active create routine.

- Doesn't change the schedule — the next scheduled run stays as it is.
- Respects the same safety checks as scheduled runs (see [Troubleshooting](troubleshooting.md#backup-was-skipped)).
- Limited to 3 manual runs per minute per server.

## Saving and validation rules

Changes in the editor are only applied when you click **Save**. **Discard changes** reloads the saved state. When saving:

| Rule | Message (example) |
| --- | --- |
| Number of routines ≤ admin limit (default 10) | *At most 10 routines per server are allowed.* |
| Every routine has a unique name | *The routine name "Hourly" is used twice.* |
| Valid schedule | *"Hourly": invalid schedule "…"* |
| Create routines respect the minimum interval | *"Hourly": backups may be created at most every 60 minute(s).* |
| Take-over routines have a valid source, no cycles | *"Weekly": please choose a source routine …* |
| Total *Keep* ≤ admin limit and ≤ backup limit | *This server's backup limit is 5, but the routines would need 6 slots.* |
| At least one routine when automatic backups are switched on | *Please add at least one routine or apply a template first.* |

What happens to the schedule when you save:

- **Unchanged** routines keep their next run.
- Routines whose **schedule** or **active** state changed get a new next run, calculated from now.
- **New** routines are due immediately.
- Switching **Automatic backups active** from off to on makes all routines due immediately.

---

[← Core concepts](concepts.md) · Next: [The Auto-Backups tab →](server-tab.md)
