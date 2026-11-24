# 🔌 API

[← Guide overview](README.md)

Everything the Auto-Backups tab and the admin page do is available through the panel API — useful for scripts,
provisioning or billing systems. Authenticate like any other panel API call (session or API key). The routes also
appear in the panel's OpenAPI documentation.

- [Conventions](#conventions)
- [Server endpoints](#server-endpoints)
- [Admin endpoints](#admin-endpoints)
- [Examples](#examples)

## Conventions

- JSON with `snake_case` keys.
- Errors use the panel's standard error format (`{"errors": ["…"]}`); messages are in the language of the authenticated
  user.
- Times are ISO 8601 in UTC.
- `{server}` is the server UUID, `{backup}` a backup UUID.

## Server endpoints

Base path: `/api/client/servers/{server}/backup-plus`

| Method | Path | Permission | Description |
| --- | --- | --- | --- |
| `GET` | `/` | `backup-plus.read` | Everything the tab shows: settings, routines, backups with planned action, log, limits, templates, time zone |
| `PUT` | `/` | `backup-plus.manage` | Save settings and routines |
| `POST` | `/apply-template` | `backup-plus.manage` | Replace the routines with a template |
| `POST` | `/run` | `backup-plus.manage` | Create a backup now (rate limit: 3/min per server) |
| `POST` | `/backups/{backup}/pin` | `backup-plus.manage` | Pin or unpin a managed backup |
| `PUT` | `/backups/{backup}/routine` | `backup-plus.manage` | Assign a managed backup to a routine, or to none |

### `PUT /` — save settings and routines

```json
{
  "enabled": true,
  "delete_failed": true,
  "routines": [
    {
      "key": "hourly",
      "uuid": null,
      "name": "Hourly",
      "enabled": true,
      "cron": "0 0 * * * *",
      "source": "create",
      "source_key": null,
      "keep_count": 3,
      "max_age_hours": 0,
      "name_template": "Auto {routine} {date}",
      "rename_template": null,
      "ignored_files": ["logs/"]
    },
    {
      "key": "weekly",
      "uuid": null,
      "name": "Weekly",
      "enabled": true,
      "cron": "0 0 0 * * 1",
      "source": "promote",
      "source_key": "hourly",
      "keep_count": 1,
      "max_age_hours": 0,
      "name_template": "Auto {routine} {date}",
      "rename_template": "Weekly {date}",
      "ignored_files": []
    }
  ]
}
```

- The list **replaces** all routines of the server, in this order.
- `key` is any string you choose; take-over routines reference their source via `source_key`.
- To update an existing routine (and keep its schedule state), pass its `uuid` — the `GET` response contains it. Without
  a `uuid` a new routine is created. Routines missing from the list are removed (their backups are kept).
- `source` is `create` or `promote` (= take over).
- `rename_template` (optional, take-over routines only): new name for backups this routine takes over; `null` keeps
  the name. Placeholders as in the [routine reference](routines.md#rename-taken-over-backups).
- The response is the same as `GET /`.

If the admin has disabled custom routines and the caller isn't an extension admin, `routines` is ignored and only
`enabled` / `delete_failed` are saved.

### `POST /apply-template`

```json
{ "template_id": "gfs-kompakt", "enabled": true }
```

Template IDs are listed in `GET /` (`templates[].id`). Built-in IDs: `gfs-kompakt` (GFS compact), `gfs-erweitert`
(GFS extended), `taeglich-7` (Daily, 7 days), `alle-6-stunden` (Every 6 hours, 2 days). The response is the same as
`GET /`.

### `POST /run`

```json
{ "routine_uuid": null }
```

`null` runs the first active create routine. Response: `{ "backup_uuid": "…" }`.

### `POST /backups/{backup}/pin`

```json
{ "locked": true }
```

### `PUT /backups/{backup}/routine`

```json
{ "routine_uuid": "…" }
```

`null` removes the backup from rotation.

### `GET /` response (shortened)

```json
{
  "policy": { "exists": true, "enabled": true, "delete_failed": true, "last_run": "…", "last_error": null },
  "routines": [
    {
      "uuid": "…", "name": "Hourly", "enabled": true, "position": 0, "cron": "0 0 * * * *",
      "source": "create", "source_routine_uuid": null, "keep_count": 3, "max_age_hours": 0,
      "name_template": "Auto {routine} {date}", "rename_template": null, "ignored_files": [],
      "next_run": "…", "last_run": "…", "last_error": null, "backup_count": 3
    }
  ],
  "backups": [
    {
      "uuid": "…", "name": "Auto Hourly 2026-11-24 13:00", "routine_uuid": "…",
      "successful": true, "locked": false, "bytes": 123456789,
      "completed": "…", "created": "…",
      "planned": { "type": "delete", "reason": { "kind": "over_keep", "routine": "Hourly", "keep": 3 } }
    }
  ],
  "log": [ { "level": "info", "message": "…", "created": "…" } ],
  "limits": {
    "globally_enabled": true, "min_interval_hours": 1, "max_keep_total": 50, "max_routines": 10,
    "allow_custom_routines": true, "can_customize": true, "backup_limit": 6, "backups_total": 5
  },
  "templates": [ { "id": "gfs-kompakt", "name": "GFS compact (5 backups)", "builtin": true, "routines": [] } ],
  "default_template_id": "gfs-kompakt",
  "timezone": "Europe/Berlin",
  "timezone_source": "server"
}
```

`timezone_source` is `server`, `node`, `extension` (admin default) or `fallback` (UTC). `planned.type` is one of
`keep`, `move` (`from`, `to`), `delete` (`reason.kind`: `failed`, `too_old`, `over_keep`),
`pinned`, `running`, `failed_kept`, `unassigned`.

## Admin endpoints

Base path: `/api/admin/extensions/dev.backupplus.gfsbackups`

| Method | Path | Permission | Description |
| --- | --- | --- | --- |
| `GET` | `/settings` | `extensions.read` | Global settings |
| `PUT` | `/settings` | `extensions.manage` | Save global settings |
| `GET` | `/templates` | `extensions.read` | Built-in and custom templates |
| `PUT` | `/templates` | `extensions.manage` | Replace the custom templates |
| `POST` | `/apply-template` | `extensions.manage` | Apply a template to many servers |
| `GET` | `/overview` | `extensions.read` | Statistics |

`PUT /settings`:

```json
{
  "enabled": true,
  "min_interval_hours": 1,
  "max_keep_total": 50,
  "default_template_id": "gfs-kompakt",
  "default_timezone": "Europe/Berlin",
  "allow_custom_routines": true,
  "default_template_id": "gfs-kompakt",
  "max_backups_per_tick": 10,
  "log_retention": 100
}
```

`POST /apply-template`:

```json
{ "template_id": "gfs-kompakt", "only_unconfigured": true, "enable": true }
```

Response: `{ "applied": 12, "skipped": ["Server name: reason", "…"] }`.

`PUT /templates` takes `{ "templates": [ … ] }` with templates in the same shape as `GET /templates` returns them; in
template routines, sources are referenced by name via `source_name`.

## Examples

Set up a new server with the default template (e.g. from a provisioning script):

```bash
curl -X POST "https://panel.example.com/api/client/servers/$SERVER/backup-plus/apply-template" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"template_id": "gfs-kompakt", "enabled": true}'
```

Trigger a backup before a deployment:

```bash
curl -X POST "https://panel.example.com/api/client/servers/$SERVER/backup-plus/run" \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{}'
```

---

[← Troubleshooting & FAQ](troubleshooting.md) · Next: [How it works →](how-it-works.md)
