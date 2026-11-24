# 🛠️ Troubleshooting & FAQ

[← Guide overview](README.md)

- [A new feature doesn't show up after an update](#a-new-feature-doesnt-show-up-after-an-update)
- [No backups are being created](#no-backups-are-being-created)
- [Backup limit reached](#backup-limit-reached)
- [Backup was skipped](#backup-was-skipped)
- [My weekly / monthly backup doesn't appear](#my-weekly--monthly-backup-doesnt-appear)
- [A backup could not be deleted](#a-backup-could-not-be-deleted)
- [Error messages when saving](#error-messages-when-saving)
- [FAQ](#faq)

## A new feature doesn't show up after an update

At the bottom of the Auto-Backups tab you see two versions: the **interface** your browser is running and the
**installed** version. If they differ, an orange *Outdated interface* alert appears at the top.

1. Reload the page with **Ctrl + Shift + R** (or open it in a private window).
2. If the alert stays, something between you and the panel still serves old files — typically a proxy or CDN such as
   Cloudflare. Purge its cache.
3. If both versions show the old number, the update itself didn't go through: check the build log under
   **Admin → Extensions**.

## No backups are being created

Work through this list:

1. **Is Backups Plus active globally?** A yellow *Disabled globally* alert on the server page means an admin switched it
   off ([Global settings](admin.md#global-settings)).
2. **Is the server active?** The *Status* card must say **Active**. If not, switch on *Automatic backups active* and
   click **Save**.
3. **Is there an active *create* routine?** Take-over routines alone never create backups.
4. **Is the next run in the past?** The *Routines* card shows the next run. Remember it's evaluated in the server's time
   zone.
5. **Check the log** at the bottom of the page — skipped runs and errors are recorded there.
6. **Can the panel back up this server at all?** Try a manual backup on the panel's **Backups** page. If that fails
   (no backup configuration, node offline, configuration in maintenance mode), Backups Plus can't succeed either.
7. **Multiple panel instances?** Automatic runs happen on the panel's *primary* instance only. Make sure one instance has
   `APP_PRIMARY` enabled.

## Backup limit reached

> *The server's backup limit has been reached (6/6). Raise the limit to at least 7 or keep fewer backups.*

Before every backup, Backups Plus checks the server's backup limit. All backups count — including manual ones and pinned
ones. If the limit is reached, Backups Plus first tries to free one slot by rotating out the oldest backup of the routine
that is about to run. If that isn't possible (e.g. all other backups are manual or pinned), the run fails with this
message.

**Fix:** raise the server's backup limit to the suggested value, delete manual backups you don't need, unpin backups, or
reduce *Keep* in your routines.

## Backup was skipped

A scheduled backup is skipped — not failed — in these situations. The log shows which one, and the routine simply runs
again at its next scheduled time.

| Log entry | Reason |
| --- | --- |
| *skipped: the server is suspended* | Suspended servers aren't backed up |
| *skipped: the server is being installed or restored* | The server's files are changing right now |
| *skipped: the server is being transferred* | The server is moving to another node |
| *skipped: a previous automatic backup is still running* | Only one automatic backup per server runs at a time. A due routine first **waits** for the running backup (log: *"…" waits for the running backup to finish*) and starts as soon as it's done. It is only skipped if the running backup is still not finished when the routine's next scheduled time arrives — then your backups take longer than the schedule interval; choose a longer interval. |

A backup that has been running for more than **6 hours** is treated as stuck and no longer blocks new backups.

When you click **Back up now** in one of these situations, you get the message *The backup was skipped – see the log
for details.*

## My weekly / monthly backup doesn't appear

Take-over routines only receive a backup when **both** are true:

1. the routine is **due** (its scheduled time has passed since it last received one), and
2. a backup **falls out** of its source routine — which happens only after a new backup of the source has **finished**
   and the source has more backups than its *Keep*.

So right after setting up the default template it takes a few hours until the hourly routine has more than 3 backups and
the first ones fall out. After that, the weekly and monthly routines fill up. Also check that the take-over routine is
**active** and that its **source** is correct.

## A backup could not be deleted

> *"…" could not be deleted: …*

The panel couldn't delete the backup — usually because the node is offline or the backup configuration is in
maintenance mode. Nothing is lost: the backup stays and the deletion is retried on the next rotation (every 5 minutes).
Once the panel has accepted a deletion, it is processed in the background by the panel itself, including the panel's
own retries.

## Error messages when saving

| Message | Fix |
| --- | --- |
| *At most N routines per server are allowed.* | Remove routines or ask the admin to raise *Max. routines per server* |
| *Every routine needs a name (1–64 characters).* | Give every routine a name |
| *The routine name "…" is used twice.* | Names must be unique (case-insensitive) |
| *"…": invalid schedule "…"* | Fix the cron expression — see [Schedule](routines.md#schedule) |
| *"…": the schedule has no upcoming runs.* | The expression never matches again (e.g. 31 February) |
| *"…": backups may be created at most every N hour(s).* | Use a less frequent schedule for create routines |
| *"…": please choose a source routine …* | Take-over routines need a source |
| *"…": the source routines form a cycle.* | A → B → A is not allowed; one routine in the chain must create backups |
| *At most N backups per server may be kept …* | Reduce *Keep* or ask the admin to raise the limit |
| *This server's backup limit is N, but the routines would need M slots.* | Raise the server's backup limit or reduce *Keep* |
| *Please add at least one routine or apply a template first.* | You switched automatic backups on without routines |
| *The template doesn't fit: …* | Applying a template failed one of the checks above for this server |

## FAQ

**Does Backups Plus work with S3 / restic / Kopia / …?**
Yes. It uses the panel's own backup functions, so every storage driver the panel supports works.

**Does it back up databases?**
It creates normal server backups (the server's files). Database instance backups are not part of Backups Plus.

**Are my manual backups safe?**
Yes. Backups Plus only manages backups in its own *Backups Plus (auto)* group.

**What happens when I uninstall Backups Plus?**
Nothing is deleted. The backups stay in the *Backups Plus (auto)* group and can be managed manually.

**Can I restore an automatic backup?**
Yes — on the panel's normal **Backups** page, like any other backup.

**Why is a backup called "Auto Hourly …" although it's my monthly backup?**
Names are given when a backup is created and, by default, don't change when it moves to another routine. The
**Routine** column shows where it belongs now. Switch on [Rename taken-over backups](routines.md#rename-taken-over-backups)
in the take-over routine to give such backups a fitting name automatically.

**I deleted a routine. Where are its backups?**
Still there, shown as *Not managed*. Assign them to another routine or delete them manually.

**Can I move a specific backup into my monthly routine?**
Yes: **⋮ → Assign routine → Monthly**. Note that the monthly routine then applies its *Keep* rule, so its previous backup
may be deleted on the next rotation if *Keep* is 1.

**Why are there 6 backups when my routines keep 5?**
A new backup is being created or has just finished; the next rotation (within 5 minutes) removes the one that fell out.

**In which time zone do schedules run?**
The server's time zone; if it has none, the node's time zone, then the admin's Backups Plus default, then UTC (see
[Time zones](concepts.md#time-zones)). The *Status* card shows which one is used. Times in the UI are shown in your
browser's local time.

**My schedules run an hour off / the time zone shows UTC.**
The server has no time zone of its own and its node reports none. Set it under the server's **Settings → Time zone**
(e.g. `Europe/Berlin`), or ask the admin to set a *Default time zone* for Backups Plus.

**Which languages are supported?**
English and German. Everything follows the language you selected in your panel account settings.

---

[← Administration](admin.md) · Next: [API →](api.md)
