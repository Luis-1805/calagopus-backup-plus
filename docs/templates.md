# 🧩 Templates

[← Guide overview](README.md)

A template is a ready-made set of routines. Applying one is the fastest way to set up a server — you can still adjust
every routine afterwards.

- [Built-in templates](#built-in-templates)
- [Applying a template](#applying-a-template)
- [Custom templates](#custom-templates)

## Built-in templates

Template names and the routine names they create follow your panel language (e.g. *Hourly* / *Stündlich*).

### GFS compact — 5 backups *(default)*

> The last 3 hours, plus a weekly backup (renewed every Monday) and a monthly backup (renewed on the 1st).

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Hourly | create | `0 0 * * * *` — every hour | 3 |
| Weekly | take over from *Hourly* | `0 0 0 * * 1` — Monday 00:00 | 1 |
| Monthly | take over from *Hourly* | `0 0 0 1 * *` — 1st, 00:00 | 1 |

**Backup limit needed:** 5 (recommended 6).

### GFS extended — 41 backups

> 24 hourly, 7 daily, 4 weekly and 6 monthly backups — about half a year of history.

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Hourly | create | `0 0 * * * *` | 24 |
| Daily | take over from *Hourly* | `0 0 0 * * *` — daily 00:00 | 7 |
| Weekly | take over from *Daily* | `0 0 0 * * 1` — Monday 00:00 | 4 |
| Monthly | take over from *Weekly* | `0 0 0 1 * *` — 1st, 00:00 | 6 |

**Backup limit needed:** 41 (recommended 42). The admin's *max. backups kept per server* must be at least 41
(default 50).

### Daily, 7 days — 7 backups

> A backup every night at 04:00, the last 7 are kept.

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Daily | create | `0 0 4 * * *` — daily 04:00 | 7 |

### Every 6 hours, 2 days — up to 8 backups

> A backup every 6 hours, at most 2 days old.

| Routine | Type | Schedule | Keep | Max. age |
| --- | --- | --- | :---: | :---: |
| Every 6 hours | create | `0 0 */6 * * *` — 00:00, 06:00, 12:00, 18:00 | 8 | 48 h |

## Applying a template

On the server: **Auto-Backups → Setup → Template → Apply template**.

What happens:

1. The template is checked against the server — e.g. whether the backup limit is high enough. If not, you get a message
   and nothing changes.
2. All existing routines of the server are **replaced** by the template's routines (you're asked to confirm if the server
   already had routines).
3. **Automatic backups** are switched **on**.
4. The new routines are due immediately: the first backup is created within a minute.

Existing backups are never deleted by applying a template. Backups that belonged to the old routines are shown as
*Not managed*; use **Assign routine** in their menu to hand them to a new routine, or delete them on the panel's
**Backups** page.

> [!NOTE]
> Templates can be applied even when the admin has disabled custom routines — that's exactly what they are for.

Admins can also apply a template to **many servers at once** — see [Administration](admin.md#apply-a-template-to-servers).

## Custom templates

Admins can create their own templates on the admin page (**Admin → Extensions → Backups Plus → Configure → Templates**):

- **New template** starts with a single hourly routine.
- **Copy as custom template** on a built-in template creates an editable copy.
- Each template has an **ID** (lowercase letters, digits, hyphens; used internally), a **name** and a **description**,
  and routines configured with the same editor as on the server page.
- **Save templates** checks every template with the same rules as the server editor (except the server's backup limit,
  which is checked when the template is applied).

Custom templates appear for all servers in the template list, marked *(custom)*. The admin can make one of them the
**default template** that is preselected on servers that aren't set up yet.

Changing a template later does **not** change servers that already applied it — templates are copied, not linked.
Re-apply the template (or use the bulk action) to roll out changes.

---

[← The Auto-Backups tab](server-tab.md) · Next: [Recipes →](recipes.md)
