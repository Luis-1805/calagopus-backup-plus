# 🎓 Tutorial: build your own backup strategy

[← Guide overview](README.md)

This tutorial is for beginners. You won't apply a template — instead you'll build a backup strategy **yourself**, routine
by routine, and understand every setting along the way. At the end you'll be able to design a strategy for any server.

**What you'll build:** a strategy that keeps the last few hours in detail, one backup per day for the past week, and one
older backup per week and per month — with automatic cleanup of everything else.

**Time needed:** about 30 minutes of reading and clicking. Some results only show up after a few hours, because backups
need time to accumulate — you can stop in between and come back.

**What you need:**

- Backup Plus installed ([Getting started](getting-started.md#2-install-the-extension))
- A server where the panel can already create backups (try one manual backup on the **Backups** page first)
- Access to the server's **Auto-Backups** tab with the `backup-plus.manage` permission

---

- [Part 1 — Plan before you click](#part-1--plan-before-you-click)
- [Part 2 — Your first routine](#part-2--your-first-routine)
- [Part 3 — Reading and writing schedules](#part-3--reading-and-writing-schedules)
- [Part 4 — A daily history with "take over"](#part-4--a-daily-history-with-take-over)
- [Part 5 — Weekly and monthly backups](#part-5--weekly-and-monthly-backups)
- [Part 6 — The backup limit](#part-6--the-backup-limit)
- [Part 7 — Polishing: names, renaming, ignored files, maximum age](#part-7--polishing-names-renaming-ignored-files-maximum-age)
- [Part 8 — Check that everything works](#part-8--check-that-everything-works)
- [Part 9 — Common beginner mistakes](#part-9--common-beginner-mistakes)
- [Exercises](#exercises)
- [Where to go next](#where-to-go-next)

> [!NOTE]
> **Panel in German?** The interface follows your language. The most important labels:
>
> | English | German |
> | --- | --- |
> | Auto-Backups | Auto-Backups |
> | Add routine | Routine hinzufügen |
> | Type: *Create a new backup* / *Take over a backup from another routine* | Art: *Neues Backup erstellen* / *Backup aus anderer Routine übernehmen* |
> | Source | Quelle |
> | Schedule · Quick select | Zeitplan · Schnellauswahl |
> | Keep · Maximum age | Behalten · Höchstalter |
> | Name template · Ignored files | Namensvorlage · Ignorierte Dateien |
> | Automatic backups active | Automatische Backups aktiv |
> | Save · Run now | Speichern · Jetzt ausführen |
> | Managed backups · Next action · Log | Verwaltete Backups · Nächste Aktion · Protokoll |

---

## Part 1 — Plan before you click

A good backup strategy answers three questions. Take a minute and write down your answers.

**1. How much work can you afford to lose?**
If the server breaks right now, how old may the newest backup be? For an active game server, losing an hour is
annoying but OK; losing a day is not. → This decides **how often** you create backups.

**2. How far back do you need to go?**
Sometimes a problem is noticed late — a griefer, a broken plugin, a corrupted world that only shows up days later. →
This decides **how far into the past** your oldest backup should reach.

**3. How much storage do you have?**
Every backup costs space and counts towards the server's **backup limit**. → This decides **how many** backups you
can keep.

The trick of a good strategy: **many recent backups, few old ones.** You need fine steps for the last hours (to undo a
recent mistake precisely) but only rough steps for last month (you just need *something* from back then).

For this tutorial we'll use this plan:

| Time range | Granularity | Number of backups |
| --- | --- | :---: |
| last 3 hours | every hour | 3 |
| last 7 days | one per day | 7 |
| last 4 weeks | one per week | 4 |
| last 3 months | one per month | 3 |
| **Total** | | **17** |

> [!TIP]
> Your answers might be different — that's the point. Keep your own numbers in mind; you'll plug them in at each step.

## Part 2 — Your first routine

Every strategy starts with **one routine that creates backups**. Everything else builds on it.

1. Open your server and click **Auto-Backups** in the sidebar.
2. Scroll down to the **Routines** card and click **Add routine**.

A new card appears. Fill it in like this:

| Field | Value | Why |
| --- | --- | --- |
| **Name** | `Hourly` | Shown everywhere — in the backup list, the log and in backup names. Pick something you'll understand in a month. |
| **Type** | *Create a new backup* | This routine is the one that actually makes backups. |
| **Schedule** | `0 0 * * * *` (Quick select: *Hourly (on the hour)*) | A backup at the start of every hour. We'll look at schedules properly in Part 3. |
| **Keep** | `3` | Keep the 3 newest hourly backups — answer to *"how much work can I lose"*. |
| **Maximum age** | `0` | 0 means "no age limit". Not needed here. |
| **Active** | on | |
| **Name template** | leave as is (`Auto {routine} {date}`) | Backups will be called e.g. *Auto Hourly 2026-11-24 13:00*. |
| **Ignored files** | leave empty for now | We'll come back to this in Part 7. |

3. Scroll up to the **Setup** card and switch on **Automatic backups active**.
4. Click **Save** (below the routines).

### What you should see now

**Within about a minute:**

- The **Managed backups** table shows a new backup *Auto Hourly …* with the next action **⏳ Running**.
- The **Log** says *Started "Auto Hourly …" for routine "Hourly"*.
- Once the backup is finished (depending on the server size, from seconds to minutes), its action changes to
  **🟢 Stays**.

> [!NOTE]
> Why does it start right away instead of at the full hour? New routines are **due immediately**, so you get a first
> backup without waiting. After that, the routine follows its schedule.

**After about 4 hours:** there are 4 hourly backups. The newest 3 are *Stays*; the oldest one shows
**🔴 Will be deleted** — hover over it and you'll read *older than the 3 newest backups of routine "Hourly"*. A few
minutes later it's gone. From now on: one new backup per hour, one old one deleted. **That's rotation.**

✅ **Checkpoint:** you have a working, self-cleaning backup routine. Many servers need nothing more than this. But it
only reaches back 3 hours — let's fix that.

## Part 3 — Reading and writing schedules

Before adding more routines, let's understand the **Schedule** field, so you're never stuck with the presets.

A schedule is a *cron expression*: six fields separated by spaces. Read it from left to right:

```text
0   0   *   *   *   *
│   │   │   │   │   └─ day of week   (0–6, 0 = Sunday, 1 = Monday)
│   │   │   │   └───── month         (1–12)
│   │   │   └───────── day of month  (1–31)
│   │   └───────────── hour          (0–23)
│   └───────────────── minute        (0–59)
└───────────────────── second        (0–59) — almost always 0
```

The symbols:

| Symbol | Meaning | Example | Reads as |
| --- | --- | --- | --- |
| a number | exactly this value | `0 30 4 * * *` | at 04:30 |
| `*` | every value | `0 0 * * * *` | every hour at minute 0 |
| `*/n` | every *n*-th value | `0 0 */6 * * *` | every 6 hours (00, 06, 12, 18) |
| `a,b` | a list | `0 0 3,15 * * *` | at 03:00 and 15:00 |
| `a-b` | a range | `0 0 1-6 * * *` | every hour from 01:00 to 06:00 |

**How to read one:** go through the fields and say what's *fixed*.
`0 0 0 * * 1` → second 0, minute 0, hour 0, any day, any month, **day of week 1** → *every Monday at 00:00*.

You don't have to trust yourself: below the field the editor shows the schedule in plain words and the **next run**.
Always glance at it after typing.

> [!IMPORTANT]
> Schedules run in the **server's time zone** (shown in the *Status* card and below the field). If your server is set
> to UTC but you live in Germany, `0 0 4 * * *` means 04:00 UTC = 05:00 or 06:00 German time.

🧪 **Try it:** what do these mean? (Answers at the end in [Exercises](#exercises).)

1. `0 0 12 * * *`
2. `0 0 0 1 * *`
3. `0 0 */2 * * *`
4. `0 0 22 * * 5`

## Part 4 — A daily history with "take over"

Your plan says: *one backup per day for the last 7 days*. You could add a second routine that **creates** a backup
every night. But think about it: your hourly routine already makes a backup every hour — including at midnight. Why
make another one?

This is what the second routine type is for: **take over**.

> A *take-over* routine doesn't create backups. Instead, when a backup **falls out** of another routine (the
> *source*) and the take-over routine is **due**, it adopts that backup instead of letting it be deleted.

So a *Daily* routine that takes over from *Hourly* once per day turns one of the hourly backups — which would be
deleted anyway — into a daily backup. **No extra backup, no extra load.**

### Add it

1. Click **Add routine** again. The new card is already pre-filled as a take-over routine — adjust it:

| Field | Value | Why |
| --- | --- | --- |
| **Name** | `Daily` | |
| **Type** | *Take over a backup from another routine* | |
| **Source** | `Hourly` | Daily adopts backups that fall out of Hourly. |
| **Schedule** | `0 0 0 * * *` (Quick select: *Daily at 00:00*) | From midnight on, Daily is *ready* to adopt one backup. |
| **Keep** | `7` | One per day for a week. |
| **Maximum age** | `0` | |

2. Click **Save**.

### What happens now, step by step

| When | What happens |
| --- | --- |
| right after saving | Daily is new, so it's **due immediately**. |
| as soon as Hourly has more than 3 backups | The oldest hourly backup **falls out** → Daily is due → **Daily takes it over**. In the log: *"Auto Hourly …" moved from "Hourly" to "Daily"*. |
| after that | Daily is done until its next scheduled time (tonight 00:00). Backups that fall out of Hourly in the meantime are deleted, as before. |
| tonight 00:00 | Daily is due again. |
| shortly after 00:00 | The next backup that falls out of Hourly becomes Daily's second backup. |
| after 7 nights | Daily has 7 backups. On the 8th night, Daily adopts a new one, and its **oldest** daily backup falls out. Nobody takes it over → **deleted**. |

In the **Managed backups** table you can see this coming: before a backup moves, its next action shows
**🔵 Becomes "Daily"**.

> [!NOTE]
> The backup keeps its name *Auto Hourly …* — names are given when a backup is created. The **Routine** column tells
> you where it belongs now. Want a fitting name? Switch on **Rename taken-over backups** in the *Daily* card — more in
> [Part 7](#rename-taken-over-backups).

✅ **Checkpoint:** your history now reaches back a week, and you still only create one backup per hour.

## Part 5 — Weekly and monthly backups

Now the same idea once more — but this time the **source** is *Daily*, not *Hourly*. That builds a **chain**:

```text
Hourly ──falls out──▶ Daily ──falls out──▶ Weekly ──falls out──▶ Monthly ──falls out──▶ deleted
```

Each step keeps fewer, older backups. Add two more routines:

| Name | Type | Source | Schedule | Keep |
| --- | --- | --- | --- | :---: |
| `Weekly` | Take over | `Daily` | `0 0 0 * * 1` — every Monday | 4 |
| `Monthly` | Take over | `Weekly` | `0 0 0 1 * *` — 1st of the month | 3 |

Click **Save**.

How it plays out: every Monday, the daily backup that falls out (the one from a week ago) moves to *Weekly* instead of
being deleted. On the 1st of each month, the weekly backup that falls out (about a month old) moves to *Monthly*.

> [!TIP]
> **Why not take Weekly directly from Hourly?** You can — that's what the built-in *GFS compact* template does. Then
> the weekly backup is only a few hours old when it's taken over, and it is replaced every week. Taking it from
> *Daily* instead means your weekly backups are already a week old when they arrive, so the chain reaches **further
> back** with the same number of backups. Both are valid; it depends on your answer to *"how far back do I need to
> go"*.

How far back does your strategy reach now?

| Routine | Covers roughly |
| --- | --- |
| Hourly (3) | the last 3 hours |
| Daily (7) | the last week |
| Weekly (4) | about 1–5 weeks back |
| Monthly (3) | about 1–4 months back |

**17 backups for roughly four months of history.** Without rotation you'd need almost 3,000 hourly backups for the same
span.

> [!IMPORTANT]
> A chain fills up **gradually**: Daily fills over a week, Weekly over a month, Monthly over a few months. That's
> normal — the *Routines* status card shows `backups/keep` for each routine, e.g. `2/7`.

## Part 6 — The backup limit

Every server has a **backup limit**: the maximum number of backups it may have. Backup Plus never exceeds it, and
**all** backups count — including manual ones.

Do the math for your strategy:

```text
  Keep of all routines:   3 + 7 + 4 + 3  = 17
+ room for a new backup:                 +  1
+ manual backups you want to keep:       +  2   (your own number)
= backup limit you need:                   20
```

Why "+1"? A new backup exists for a few minutes next to the old ones until the next rotation removes the one that fell
out.

Check the **Backup limit** card on the Auto-Backups page (`all backups / limit`). If the limit is too low, a red alert
appears in the *Setup* card and saving shows *This server's backup limit is …*. Only an administrator can raise it.

> [!TIP]
> If you can't get a higher limit, reduce *Keep* where it hurts least — usually *Daily* (e.g. 5 instead of 7) or
> *Weekly*.

## Part 7 — Polishing: names, renaming, ignored files, maximum age

Your strategy works. These options make it better.

### Better names

Open the *Hourly* routine and change the **Name template**, for example:

```text
{server} – {day} {time}
```

→ *Survival – 2026-11-24 13:00*. Available placeholders: `{routine}`, `{date}`, `{day}`, `{time}`, `{server}`.
Only *create* routines have a name template, because only they create backups.

### Rename taken-over backups

When *Daily* takes over an hourly backup, it's still called *Survival – 2026-11-24 13:00* (or *Auto Hourly …*). To
make the backup list easier to read, give each take-over routine its own name:

1. Open the *Daily* card and switch on **Rename taken-over backups**.
2. The **New name** field appears with `{routine} {date}`. Change it to e.g. `Daily {day}`.
3. Do the same for *Weekly* (`Weekly {day}`) and *Monthly* (`Monthly {day}`), then **Save**.

From now on a backup is renamed whenever it moves along the chain: *Survival – 2026-11-24 13:00* → *Daily 2026-11-24* →
*Weekly 2026-11-24* → *Monthly 2026-11-24*. `{date}`, `{day}` and `{time}` always refer to when the backup was
**originally made**, and `{original}` inserts the previous name. Backups that were taken over before you switched
this on keep their names.

### Leave out what you don't need

Logs, caches and temporary files make backups bigger and slower without helping a restore. In *Hourly* →
**Ignored files**, one rule per line, for example:

```text
logs/
cache/
*.tmp
```

> [!WARNING]
> Be careful not to exclude something you need for a restore. If unsure, make a manual backup, look at its contents in
> the panel and decide then.

### Limit the age (optional)

**Maximum age** removes backups older than a given number of hours, even if *Keep* would allow more. Example: if a rule
says data must not be kept longer than 90 days, set *Monthly* to `2160` (= 90 × 24). Old backups then fall out of the
chain at 90 days at the latest.

### Keep one backup forever

Before a big change (a game update, a new modpack), make a backup with **Run now** and then **⋮ → Pin (never delete)**
in the backup list. Pinned backups are never removed by rotation — until you unpin them.

## Part 8 — Check that everything works

Use this checklist after setting up any strategy:

- [ ] **Status card** says *Active*.
- [ ] **Routines card** lists all routines with a sensible *next run*.
- [ ] **Backup limit card** isn't red.
- [ ] Every take-over routine has the right **source** (open its card and check).
- [ ] **Run now** on your create routine starts a backup (*Running* in the list, *Started …* in the log).
- [ ] After a few hours, the **Next action** column shows *Stays*, *Becomes "…"* and *Will be deleted* the way you
      expect.
- [ ] The **Log** shows no red *error* entries.

**Reading the backup list is your best tool.** Before any backup changes, the *Next action* column tells you what
will happen and — on hover — why. If something looks wrong, change the routine and save; the preview updates
immediately.

## Part 9 — Common beginner mistakes

| Mistake | What happens | Fix |
| --- | --- | --- |
| Forgot to switch on **Automatic backups active** | Nothing happens at all | Switch it on and **Save** |
| Only take-over routines, no create routine | No backups are ever made | Add a *Create* routine as the start of the chain |
| Take-over routine with the wrong source | It never receives backups, or the wrong ones | Check the **Source** field |
| Expecting the weekly backup immediately | *Weekly* stays at `0/4` for days | Normal — chains fill up over time (Part 5) |
| Backup limit too low | Runs fail with *backup limit reached* | Do the math from Part 6 and ask an admin |
| Schedule in the wrong time zone | Backups run at unexpected hours | Check the time zone in the *Status* card |
| Two create routines at the same time | The second waits until the first is done | Fine — but usually one create routine plus take-over routines is simpler |
| Deleted a routine and "lost" its backups | They're still there, shown as *Not managed* | **⋮ → Assign routine** in the backup list |

## Exercises

Try to design these yourself before opening the solutions.

<details>
<summary><b>Answers to the schedule quiz in Part 3</b></summary>

1. `0 0 12 * * *` — every day at 12:00
2. `0 0 0 1 * *` — on the 1st of every month at 00:00
3. `0 0 */2 * * *` — every 2 hours (00:00, 02:00, 04:00, …)
4. `0 0 22 * * 5` — every Friday at 22:00

</details>

**Exercise 1 — Small server, little storage.** A small survival server with a backup limit of 6. The owner wants to undo
mistakes from today and have something from last week.

<details>
<summary>Possible solution</summary>

| Name | Type | Source | Schedule | Keep |
| --- | --- | --- | --- | :---: |
| Every 4 hours | Create | – | `0 0 */4 * * *` | 3 |
| Weekly | Take over | Every 4 hours | `0 0 0 * * 1` | 2 |

3 backups cover the last 12 hours, 2 weekly backups reach back up to two weeks. 5 backups + 1 = limit 6. ✅

</details>

**Exercise 2 — No lag during play time.** Players are online from 14:00 to 01:00. Backups cause short lag spikes, so
they should only run between 03:00 and 08:00. The owner wants a week of history.

<details>
<summary>Possible solution</summary>

| Name | Type | Source | Schedule | Keep |
| --- | --- | --- | --- | :---: |
| Morning | Create | – | `0 0 3-8 * * *` (every hour 03:00–08:00) | 6 |
| Daily | Take over | Morning | `0 0 0 * * *` | 7 |

Each morning creates 6 backups; one per day is handed to *Daily*. Make sure the server's time zone matches the players'.

</details>

**Exercise 3 — Data must not be older than 30 days.** A server whose backups contain personal data that must not be kept
for more than 30 days.

<details>
<summary>Possible solution</summary>

| Name | Type | Source | Schedule | Keep | Max. age |
| --- | --- | --- | --- | :---: | :---: |
| Daily | Create | – | `0 0 4 * * *` | 7 | 0 |
| Weekly | Take over | Daily | `0 0 0 * * 1` | 4 | **720** |

720 hours = 30 days. *Weekly* never holds a backup older than that, and *Daily* only reaches back a week.

</details>

**Exercise 4 — Find the bug.** This setup never produces weekly backups. Why?

| Name | Type | Source | Schedule | Keep |
| --- | --- | --- | --- | :---: |
| Hourly | Create | – | `0 0 * * * *` | 3 |
| Weekly | Take over | **Daily** | `0 0 0 * * 1` | 2 |
| Daily | Take over | Hourly | `0 0 0 * * *` | 7 |

<details>
<summary>Answer</summary>

Trick question — it works! The order of the cards doesn't matter for chains: Backup Plus always processes a source
before the routines that take over from it. *Weekly* fills up once *Daily* has 7 backups and starts passing them on —
that takes a week. If after two weeks *Weekly* is still empty, check that *Daily* is **Active**.

</details>

## Where to go next

- [Routine reference](routines.md) — every field in detail
- [Core concepts](concepts.md) — the rotation algorithm explained precisely
- [Recipes](recipes.md) — more ready-made strategies to learn from
- [Troubleshooting & FAQ](troubleshooting.md) — when something doesn't behave as expected

Once you've built a strategy you like, an administrator can save it as a
[custom template](templates.md#custom-templates), so you can apply it to other servers with one click.

---

[← Guide overview](README.md)
