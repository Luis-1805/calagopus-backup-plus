# 🍳 Recipes

[← Guide overview](README.md)

Ready-to-use routine setups. Build them in the routine editor (**Add routine**), or save them as a
[custom template](templates.md#custom-templates) if you need them on many servers.

Notation: **C** = create a new backup, **T(x)** = take over from routine *x*.

- [Last 3 hours + weekly + monthly (default)](#last-3-hours--weekly--monthly-default)
- [Weekly backup that is always at least a week old](#weekly-backup-that-is-always-at-least-a-week-old)
- [Nightly backups for a week](#nightly-backups-for-a-week)
- [Busy server: every 2 hours, plus daily for a week](#busy-server-every-2-hours-plus-daily-for-a-week)
- [Only at night, never during play time](#only-at-night-never-during-play-time)
- [Time-boxed: nothing older than 3 days](#time-boxed-nothing-older-than-3-days)
- [Long-term archive: weekly for a quarter, monthly for a year](#long-term-archive-weekly-for-a-quarter-monthly-for-a-year)
- [Keep one special backup forever](#keep-one-special-backup-forever)
- [Exclude logs and caches](#exclude-logs-and-caches)

## Last 3 hours + weekly + monthly (default)

The **GFS compact** template.

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Hourly | C | `0 0 * * * *` | 3 |
| Weekly | T(Hourly) | `0 0 0 * * 1` | 1 |
| Monthly | T(Hourly) | `0 0 0 1 * *` | 1 |

5 backups · backup limit ≥ 6

## Weekly backup that is always at least a week old

Same as above, but the weekly routine keeps **2**. Right after Monday's rotation you have this week's backup *and* last
week's backup.

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Hourly | C | `0 0 * * * *` | 3 |
| Weekly | T(Hourly) | `0 0 0 * * 1` | **2** |
| Monthly | T(Hourly) | `0 0 0 1 * *` | 1 |

6 backups · backup limit ≥ 7

## Nightly backups for a week

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Nightly | C | `0 0 4 * * *` | 7 |

7 backups · backup limit ≥ 8 — the **Daily, 7 days** template.

## Busy server: every 2 hours, plus daily for a week

Less load than hourly backups, but a week of daily history.

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Every 2 hours | C | `0 0 */2 * * *` | 6 |
| Daily | T(Every 2 hours) | `0 0 0 * * *` | 7 |

13 backups · covers the last 12 hours in 2-hour steps and the last week day by day.

## Only at night, never during play time

Backups can cause lag on some game servers. Back up only between 01:00 and 06:00:

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Night | C | `0 0 1-6 * * *` — every hour from 01:00 to 06:00 | 6 |
| Weekly | T(Night) | `0 0 0 * * 1` | 2 |

8 backups.

## Time-boxed: nothing older than 3 days

Combine *Keep* with **Maximum age** when old backups must never exist, e.g. for privacy reasons:

| Routine | Type | Schedule | Keep | Max. age |
| --- | --- | --- | :---: | :---: |
| Every 6 hours | C | `0 0 */6 * * *` | 12 | 72 h |

At most 12 backups, none older than 3 days.

## Long-term archive: weekly for a quarter, monthly for a year

| Routine | Type | Schedule | Keep |
| --- | --- | --- | :---: |
| Daily | C | `0 0 3 * * *` | 7 |
| Weekly | T(Daily) | `0 0 0 * * 1` | 13 |
| Monthly | T(Weekly) | `0 0 0 1 * *` | 12 |

32 backups · a year of history. Requires the admin's *max. backups kept per server* ≥ 32 (default 50).

## Keep one special backup forever

Before a big update you made a backup you want to keep:

1. Find it in **Managed backups**.
2. **⋮ → Pin (never delete)**.

It stays until you unpin it. Pinned backups still count towards the server's backup limit.

## Exclude logs and caches

In the *create* routine, under **Ignored files**:

```text
logs/
cache/
*.log
```

Smaller, faster backups — and fewer slots wasted on data you never restore.

---

[← Templates](templates.md) · Next: [Administration →](admin.md)
