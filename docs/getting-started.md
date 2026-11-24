# 🚀 Getting started

[← Guide overview](README.md)

This page takes you from zero to working automatic backups.

- [1. Requirements](#1-requirements)
- [2. Install the extension](#2-install-the-extension)
- [3. Check the backup limit](#3-check-the-backup-limit)
- [4. Set up your first server](#4-set-up-your-first-server)
- [5. Check that it works](#5-check-that-it-works)
- [6. Give subusers access (optional)](#6-give-subusers-access-optional)
- [Updating and uninstalling](#updating-and-uninstalling)

## 1. Requirements

| Requirement | Why |
| --- | --- |
| Calagopus panel **1.2.11 or newer** | Backup Plus uses panel APIs that were added in that version |
| The **`:heavy`** Docker image (or a development environment) | Extensions are compiled when they are installed; the regular image has no compiler. See [Switching to the heavy image](https://calagopus.com/docs/panel/extensions/switching-to-the-heavy-image) |
| A working **backup configuration** for your servers | Backup Plus creates normal panel backups, so the panel must already be able to back up the server (local, S3, restic, Kopia, …) |

## 2. Install the extension

**Docker (`:heavy` image)**

1. Download `dev_backupplus_gfsbackups.c7s.zip` from the
   [latest release](https://github.com/Luis-1805/calagopus-backup-plus/releases/latest).
2. Open the panel as an administrator and go to **Admin → Extensions**.
3. Drop the file into the upload area. The panel builds and loads the extension; this usually takes one to two minutes.
   You can follow the progress in the build log.

Alternatively, copy the file into the panel's `extensions/` data directory (with the default heavy compose stack that's
`./build/extensions`) and restart the container with `docker compose restart web`.

**Development environment**

```bash
panel-rs extensions add dev_backupplus_gfsbackups.c7s.zip
panel-rs extensions apply --profile balanced
```

After the build, **Backup Plus** appears in the extension list. Its card shows a short summary
(active servers · routines · backups), and **Configure** opens the [admin page](admin.md).

## 3. Check the backup limit

Every Calagopus server has a **backup limit** — the maximum number of backups it may have. Backup Plus respects that
limit, and **all** backups count towards it, including manual ones.

> [!IMPORTANT]
> Set each server's backup limit to at least the total number of backups your routines keep, **plus one**.
>
> For the default template (3 hourly + 1 weekly + 1 monthly) that is **5 + 1 = 6**.

Why plus one? A new backup exists for a few minutes alongside the old ones until the next rotation sorts it in. If the
limit is exactly the number of kept backups, Backup Plus first frees a slot by rotating out the oldest backup of the
routine that is about to run — this works, but it means you briefly have one backup fewer.

You change the limit as an administrator in the server's settings in the admin area (**Admin → Servers**). The
Auto-Backups tab warns you in red when the limit is too low for your routines.

## 4. Set up your first server

1. Open the server and click **Auto-Backups** in the sidebar.
2. In the **Setup** card, choose a template. The default is **GFS compact (5 backups)**:
   - **Hourly** — a new backup every hour, keeps the newest 3
   - **Weekly** — takes over one backup every Monday, keeps 1
   - **Monthly** — takes over one backup on the 1st of each month, keeps 1
3. Click **Apply template**. This creates the routines **and** switches automatic backups on.

That's it. The first backup is created within about a minute.

> [!NOTE]
> Prefer to build your own? Skip the template, click **Add routine** in the **Routines** card, configure it (see the
> [routine reference](routines.md)), switch on **Automatic backups active** and click **Save**.

## 5. Check that it works

- The **Status** card shows **Active**, and the **Routines** card lists your routines with their next run.
- After a minute, the **Managed backups** table shows the new backup with the action **Running**, then **Stays**.
- The **Log** at the bottom of the page records every step, for example *Started "Auto Hourly 2026-11-24 13:00" for
  routine "Hourly"*.
- In the panel's normal **Backups** page, the backups appear in the group **Backup Plus (auto)**.

Over the next few hours you will see the rotation in action: once there are more than 3 hourly backups, the oldest one
either moves to the weekly or monthly routine (if that routine is due) or is deleted. See
[Core concepts](concepts.md) for exactly how that works.

## 6. Give subusers access (optional)

Server owners can always use the tab. For subusers, grant the permissions in the server's **Subusers** page:

| Permission | Allows |
| --- | --- |
| `backup-plus.read` | Seeing the Auto-Backups tab: routines, backups, next actions and the log |
| `backup-plus.manage` | Changing routines, applying templates, **Back up now**, pinning backups and assigning them to routines |

## Updating and uninstalling

**Updating:** upload the new `.c7s.zip` the same way. Database changes are applied automatically. When upgrading from
1.x, your old settings are converted into equivalent routines (Hourly / Daily / Weekly / Monthly) and existing backups
keep their assignment.

**Uninstalling:** remove the extension under **Admin → Extensions**. Your backups are **not** deleted — they stay in the
**Backup Plus (auto)** group and can be managed like any other backup.

---

Next: [Core concepts →](concepts.md)
