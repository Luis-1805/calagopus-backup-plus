# ⚙️ Administration

[← Guide overview](README.md)

Open the admin page via **Admin → Extensions → Backup Plus → Configure**. Viewing requires the panel permission
`extensions.read`, changing anything requires `extensions.manage`.

- [Overview](#overview)
- [Global settings](#global-settings)
- [Templates](#templates)
- [Apply a template to servers](#apply-a-template-to-servers)
- [Permissions](#permissions)
- [Rolling out Backup Plus on a large panel](#rolling-out-backup-plus-on-a-large-panel)
- [Activity log events](#activity-log-events)

## Overview

| Figure | Meaning |
| --- | --- |
| **Servers set up** | Servers that have Backup Plus settings (active or paused) |
| **of which active** | Servers with *Automatic backups active* switched on |
| **Routines** | Routines across all servers |
| **with errors** | Active servers where at least one routine's last run failed |
| **Managed backups** | Backups in the Backup Plus groups of all servers |
| **Storage** | Total size of those backups |

The extension card in the extension list shows a one-line version of this.

## Global settings

| Setting | Default | Range | What it does |
| --- | :---: | :---: | --- |
| **Backup Plus active** | on | | Main switch. Off: no backups are created or deleted on any server. Servers show a *Disabled globally* notice. Settings and routines are kept. |
| **Allow custom routines** | on | | Off: server users can only apply templates; the routine editor becomes read-only. Users with the admin permission `extensions.manage` can always edit routines. |
| **Default template** | GFS compact | | Preselected in the template list of servers that aren't set up yet. |
| **Default time zone** | empty (UTC) | any IANA time zone | Used for servers that have no time zone of their own and whose node doesn't report one. See [Time zones](concepts.md#time-zones). |
| **Minimum interval between new backups** | 60 min | 5–10,080 min | Smallest allowed gap between two runs of a *create* routine, in minutes. Protects nodes from schedules like "every minute". Values from versions before 2.3 (hours) are converted automatically. |
| **Max. backups kept per server** | 50 | 1–500 | Upper limit for the sum of *Keep* over all routines of a server. |
| **Max. routines per server** | 10 | 1–50 | Upper limit for the number of routines per server. |
| **Max. servers per minute** | 10 | 1–1000 | How many servers may start an automatic backup in the same minute. Further due servers are handled in the following minutes. Prevents dozens of simultaneous backups at the top of the hour. |
| **Log entries per server** | 100 | 10–10,000 | Older entries of each server's log are removed every 5 minutes. |

Limits apply when routines are saved or templates are applied. Lowering a limit doesn't change existing routines, but
they can't be saved again until they fit.

## Templates

The **Templates** section lists the [built-in templates](templates.md#built-in-templates) (read-only, expandable to see
their routines) and your **custom templates**.

- **New template** — creates a template with one hourly routine.
- **Copy as custom template** — in a built-in template, creates an editable copy.
- Edit **ID**, **name**, **description** and the routines (same editor as on the server page; schedules are shown in
  UTC here because the template isn't bound to a server yet).
- **Delete template** removes it from the list; servers that already use its routines are not affected.
- **Save templates** — validates all custom templates and saves them. Template IDs must be unique, use only lowercase
  letters, digits and hyphens, and must not clash with a built-in template.

If the default template is deleted, the default falls back to *GFS compact*.

## Apply a template to servers

Roll out a template to many servers in one go.

| Option | Default | Meaning |
| --- | :---: | --- |
| **Template** | | Any built-in or custom template |
| **Only servers without routines** | on | Off: **all** servers get the template and their existing routines are replaced |
| **Enable automatic backups right away** | on | Off: the routines are created but the servers stay paused until someone switches them on |

Every server is validated individually. Servers whose backup limit is too low for the template are **skipped**; the
result message says how many were applied and skipped (the browser console lists the skipped servers with the reason).
Each affected server gets a log entry *Template "…" applied by an administrator*.

> [!CAUTION]
> With *Only servers without routines* switched off, existing routines on **all** servers are replaced. Backups are
> never deleted by this, but they become *Not managed* until they are assigned to a routine again.

> [!NOTE]
> Save pending template changes first — the **Apply** button is disabled while the template editor has unsaved
> changes.

## Permissions

**Server permissions** (in the *Backup Plus* group of the subuser permission picker):

| Permission | Allows |
| --- | --- |
| `backup-plus.read` | Viewing the Auto-Backups tab |
| `backup-plus.manage` | Changing settings and routines, applying templates, manual runs, pinning and assigning backups |

**Admin permissions** (existing panel permissions):

| Permission | Allows |
| --- | --- |
| `extensions.read` | Viewing the admin page, settings, templates and overview |
| `extensions.manage` | Changing settings and templates, applying templates to servers, editing routines on any server even if custom routines are disabled |

API keys follow the same permissions — see [API](api.md).

## Rolling out Backup Plus on a large panel

1. **Raise backup limits first.** Each server needs a backup limit of at least the template's total + 1.
2. **Decide on the default.** Use a built-in template or create a custom one (e.g. *Nightly, 7 days* for low-cost
   plans).
3. **Spread the load.** With many servers, keep *Max. servers per minute* moderate and prefer schedules at different
   minutes (custom templates can use e.g. `0 15 * * * *`).
4. **Apply to unconfigured servers** with *Only servers without routines* on. Repeat later for new servers.
5. **Decide whether users may customise.** Switch off *Allow custom routines* if you want users to choose only from
   your templates.
6. **Watch the overview.** *with errors* tells you when servers need attention (usually backup limits or node problems).

## Activity log events

Backup Plus writes the following events to the panel's activity logs:

| Event | Log | When |
| --- | --- | --- |
| `server:backup.create` | Server | Backup created by a routine (`"source": "backup-plus"`, with routine name) |
| `server:backup.delete` | Server | Backup deleted by rotation (`"source": "backup-plus"`, with reason) |
| `server:backup-plus.update` | Server | Settings or routines saved |
| `server:backup-plus.apply-template` | Server | Template applied on the server page |
| `server:backup-plus.run` | Server | *Back up now* / *Run now* |
| `server:backup-plus.pin` | Server | Backup pinned or unpinned |
| `backup-plus:settings.update` | Admin | Global settings saved |
| `backup-plus:templates.update` | Admin | Custom templates saved |
| `backup-plus:templates.apply` | Admin | Template applied to servers |

---

[← Recipes](recipes.md) · Next: [Troubleshooting & FAQ →](troubleshooting.md)
