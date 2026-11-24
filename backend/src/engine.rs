//! Ausführung: Backups anlegen, zwischen Routinen verschieben und löschen.

use crate::{
    i18n::Msg,
    models::{self, ManagedBackup, Policy, Routine, RoutineSource},
    retention::{self, BackupSpec, PlannedAction, RoutineSpec},
    schedule, settings,
};
use shared::{
    State,
    models::{
        ByUuid, CreatableModel, DeletableModel,
        server::{Server, ServerStatus},
        server_activity::{CreateServerActivityOptions, ServerActivity},
        server_backup::{CreateServerBackupOptions, DeleteServerBackupOptions, ServerBackup},
        server_backup_group::{CreateServerBackupGroupOptions, ServerBackupGroup},
    },
};

pub const GROUP_NAME: &str = "Backups Plus (auto)";

/// Laufende Backups, die älter sind, gelten als hängengeblieben und blockieren nicht mehr.
const STUCK_AFTER_HOURS: i64 = 6;

#[inline]
pub fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Was einen Lauf ausgelöst hat.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Hintergrund-Task (alle 5 s): fällige Routinen ausführen.
    Scheduled,
    /// "Jetzt sichern": bestimmte Routine (oder die erste erstellende) sofort ausführen.
    Manual(Option<uuid::Uuid>),
}

/// (Minute, bereits bearbeitete Server) für die Lastverteilung über mehrere Ticks.
static MINUTE_BUDGET: std::sync::Mutex<(i64, u32)> = std::sync::Mutex::new((0, 0));

fn take_minute_budget(now: chrono::NaiveDateTime, max: u32) -> bool {
    let minute = now.and_utc().timestamp() / 60;
    let mut budget = MINUTE_BUDGET.lock().unwrap_or_else(|e| e.into_inner());
    if budget.0 != minute {
        *budget = (minute, 0);
    }
    if budget.1 >= max {
        return false;
    }
    budget.1 += 1;
    true
}

/// Wird alle 5 Sekunden vom Hintergrund-Task aufgerufen.
pub async fn tick(state: State) -> Result<(), anyhow::Error> {
    let settings = settings::load(&state).await;
    if !settings.enabled || crate::legacy::old_extension_active(&state).await {
        return Ok(());
    }

    let now = now();
    let due = Policy::servers_due(&state.database, now).await?;
    let mut handled = std::collections::HashSet::new();

    for server_uuid in due {
        // Lastverteilung: höchstens N Server pro Minute, Überzählige in der nächsten Minute.
        if !take_minute_budget(now, settings.max_backups_per_tick) {
            break;
        }
        handled.insert(server_uuid);

        if let Err(err) = run_server(&state, server_uuid, Trigger::Scheduled).await {
            tracing::error!(server = %server_uuid, "backups-plus: run failed: {err:#?}");
            models::write_log(&state.database, server_uuid, "error", Msg::from_error(&err)).await;
        }
    }

    // Alle 5 Minuten: Rotation für alle aktiven Server (fertig gewordene Backups einsortieren).
    use chrono::Timelike;
    if now.minute().is_multiple_of(5) && now.second() < 5 {
        for server_uuid in Policy::servers_enabled(&state.database).await? {
            if handled.contains(&server_uuid) {
                continue;
            }

            if let Err(err) = run_retention(&state, server_uuid).await {
                tracing::warn!(server = %server_uuid, "backups-plus: rotation failed: {err:#?}");
            }
        }

        models::trim_log(&state.database, settings.log_retention as i64).await?;
    }

    Ok(())
}

struct Context {
    policy: Policy,
    server: Server,
    timezone: chrono_tz::Tz,
    group_uuid: uuid::Uuid,
    routines: Vec<Routine>,
    /// Wirksamer Mindestabstand (Sekunden) zwischen zwei neuen Backups einer Routine.
    min_interval: u32,
}

async fn load_context(state: &State, server_uuid: uuid::Uuid) -> Result<Context, anyhow::Error> {
    let mut policy = Policy::by_server_uuid(&state.database, server_uuid)
        .await?
        .unwrap_or_else(|| Policy::default_for(server_uuid));
    let server = Server::by_uuid(&state.database, server_uuid).await?;
    let settings = settings::load(state).await;
    let min_interval = settings.min_interval_for(policy.min_interval_seconds);
    let (timezone, _) = schedule::resolve_timezone(state, &server, &settings.default_timezone).await;
    let group_uuid = ensure_group(state, &mut policy).await?;
    let routines = Routine::all_for_server(&state.database, server_uuid).await?;

    Ok(Context {
        policy,
        server,
        timezone,
        group_uuid,
        routines,
        min_interval,
    })
}

/// Nur Rotation, ohne neue Backups.
pub async fn run_retention(state: &State, server_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let mut ctx = load_context(state, server_uuid).await?;
    apply_retention(state, &mut ctx, None).await?;
    Ok(())
}

/// Rotation und (falls fällig bzw. manuell ausgelöst) ein neues Backup.
pub async fn run_server(
    state: &State,
    server_uuid: uuid::Uuid,
    trigger: Trigger,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let now = now();
    let mut ctx = load_context(state, server_uuid).await?;

    apply_retention(state, &mut ctx, None).await?;

    let target = match trigger {
        Trigger::Scheduled => ctx
            .routines
            .iter()
            .position(|r| r.source == RoutineSource::Create && r.is_due(now)),
        Trigger::Manual(Some(uuid)) => ctx.routines.iter().position(|r| r.uuid == uuid),
        Trigger::Manual(None) => ctx
            .routines
            .iter()
            .position(|r| r.source == RoutineSource::Create && r.enabled)
            .or_else(|| ctx.routines.iter().position(|r| r.source == RoutineSource::Create)),
    };

    let Some(index) = target else {
        if matches!(trigger, Trigger::Manual(_)) {
            return Err(Msg::new("err.no_create_routine").into());
        }
        return Ok(None);
    };

    if ctx.routines[index].source != RoutineSource::Create {
        return Err(Msg::new("err.routine_is_promote")
            .arg("routine", &ctx.routines[index].name)
            .into());
    }

    // Mindestabstand gilt sofort auch für bestehende Routinen, deren Zeitplan zu häufig ist.
    if trigger == Trigger::Scheduled {
        let routine = &mut ctx.routines[index];
        if let (Ok(cron), Some(last)) = (schedule::parse(&routine.cron), routine.last_run)
            && schedule::too_frequent(&cron, ctx.timezone, now, ctx.min_interval)
            && schedule::earliest_after(last, ctx.min_interval) > now
        {
            routine.next_run =
                schedule::next_allowed(&cron, ctx.timezone, now, Some(last), ctx.min_interval);
            routine.save_run_state(&state.database).await?;
            return Ok(None);
        }
    }

    let outcome = create_backup(state, &mut ctx, index, trigger).await;

    // Wartet die Routine auf ein laufendes Backup, bleibt sie fällig und wird im nächsten Tick erneut versucht.
    if matches!(outcome, Ok(Outcome::Waiting)) {
        return Ok(None);
    }

    let result = outcome.map(|outcome| match outcome {
        Outcome::Created(uuid) => Some(uuid),
        Outcome::Skipped | Outcome::Waiting => None,
    });

    // Beim geplanten Lauf den nächsten Termin setzen – auch bei Fehlern, damit nichts in Schleife läuft.
    if trigger == Trigger::Scheduled {
        let routine = &mut ctx.routines[index];
        routine.next_run = schedule::parse(&routine.cron).ok().and_then(|cron| {
            schedule::next_allowed(&cron, ctx.timezone, now, Some(now), ctx.min_interval)
        });
        routine.last_run = Some(now);
        routine.last_error = result.as_ref().err().map(|err| Msg::from_error(err).to_json());
        routine.save_run_state(&state.database).await?;
    } else if result.is_ok() {
        let routine = &mut ctx.routines[index];
        routine.last_run = Some(now);
        routine.last_error = None;
        routine.save_run_state(&state.database).await?;
    }

    ctx.policy.last_run = Some(now);
    ctx.policy.last_error = result.as_ref().err().map(|err| Msg::from_error(err).to_json());
    ctx.policy.save_run_state(&state.database).await?;

    result
}

/// Nach einer Änderung des Mindestabstands: Termine, die jetzt früher erlaubt sind, vorziehen.
/// (Ein größerer Abstand wird ohnehin bei jedem Lauf geprüft.)
pub async fn reschedule(state: &State, server_uuid: uuid::Uuid) -> Result<(), anyhow::Error> {
    let settings = settings::load(state).await;
    let Some(policy) = Policy::by_server_uuid(&state.database, server_uuid).await? else {
        return Ok(());
    };
    let min_interval = settings.min_interval_for(policy.min_interval_seconds);
    let server = Server::by_uuid(&state.database, server_uuid).await?;
    let (timezone, _) = schedule::resolve_timezone(state, &server, &settings.default_timezone).await;
    let now = now();

    for mut routine in Routine::all_for_server(&state.database, server_uuid).await? {
        if routine.source != RoutineSource::Create || !routine.enabled {
            continue;
        }
        let Ok(cron) = schedule::parse(&routine.cron) else {
            continue;
        };
        let next = schedule::next_allowed(&cron, timezone, now, routine.last_run, min_interval);
        if let (Some(next), Some(current)) = (next, routine.next_run)
            && next < current
        {
            routine.next_run = Some(next);
            routine.save_run_state(&state.database).await?;
        }
    }

    Ok(())
}

/// Stellt sicher, dass es die eigene Backup-Gruppe gibt, und gibt deren UUID zurück.
pub async fn ensure_group(state: &State, policy: &mut Policy) -> Result<uuid::Uuid, anyhow::Error> {
    if let Some(group_uuid) = policy.backup_group_uuid
        && ServerBackupGroup::by_server_uuid_uuid(&state.database, policy.server_uuid, group_uuid)
            .await?
            .is_some()
    {
        return Ok(group_uuid);
    }

    // Die Gruppe nutzt keine eigene Panel-Retention (alles 0 = deaktiviert),
    // damit sich Panel und Extension nicht in die Quere kommen.
    let group = ServerBackupGroup::create(
        state,
        CreateServerBackupGroupOptions {
            server_uuid: policy.server_uuid,
            name: GROUP_NAME.into(),
            retention: Default::default(),
        },
    )
    .await?;

    policy.backup_group_uuid = Some(group.uuid);
    policy.upsert(&state.database).await?;
    models::write_log(
        &state.database,
        policy.server_uuid,
        "info",
        Msg::new("log.group_created").arg("group", GROUP_NAME),
    )
    .await;

    Ok(group.uuid)
}

/// Setzt die Platzhalter einer Namensvorlage ein. Ergebnis leer → `None`.
///
/// `at` ist der Zeitpunkt für `{date}`, `{day}` und `{time}` (beim Umbenennen: die Erstellung
/// des Backups), `original` der bisherige Name (nur beim Umbenennen sinnvoll).
pub fn render_template(
    template: &str,
    server_name: &str,
    routine: &str,
    original: &str,
    at: chrono::DateTime<chrono_tz::Tz>,
) -> Option<String> {
    let name = template
        .replace("{date}", &at.format("%Y-%m-%d %H:%M").to_string())
        .replace("{time}", &at.format("%H:%M").to_string())
        .replace("{day}", &at.format("%Y-%m-%d").to_string())
        .replace("{routine}", routine)
        .replace("{tier}", routine)
        .replace("{server}", server_name)
        .replace("{original}", original);

    let name = name.trim();
    (!name.is_empty()).then(|| name.chars().take(255).collect())
}

fn render_name(
    template: &str,
    server: &Server,
    routine: &str,
    timezone: chrono_tz::Tz,
) -> compact_str::CompactString {
    let now = chrono::Utc::now().with_timezone(&timezone);

    render_template(template, &server.name, routine, "", now)
        .unwrap_or_else(|| ServerBackup::default_name().to_string())
        .into()
}

/// Benennt ein Backup nach der Umbenennungs-Vorlage einer Routine um.
/// Gibt `(alter Name, neuer Name)` zurück, wenn sich der Name geändert hat.
pub async fn rename_for_routine(
    state: &State,
    server: &Server,
    timezone: chrono_tz::Tz,
    backup_uuid: uuid::Uuid,
    routine: &Routine,
) -> Result<Option<(String, String)>, anyhow::Error> {
    use shared::models::{UpdatableModel, server_backup::UpdateServerBackupOptions};

    let Some(template) = routine.rename_template.as_deref().filter(|t| !t.trim().is_empty())
    else {
        return Ok(None);
    };
    let Some(mut backup) =
        ServerBackup::by_server_uuid_uuid(&state.database, server.uuid, backup_uuid).await?
    else {
        return Ok(None);
    };

    let old = backup.name.to_string();
    let created = backup.created.and_utc().with_timezone(&timezone);
    let Some(new) = render_template(template, &server.name, &routine.name, &old, created) else {
        return Ok(None);
    };
    if new == old {
        return Ok(None);
    }

    backup
        .update(
            state,
            UpdateServerBackupOptions {
                name: Some(new.as_str().into()),
                ..Default::default()
            },
        )
        .await?;

    Ok(Some((old, new)))
}

/// Ergebnis eines Erstellversuchs.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// Backup wurde gestartet.
    Created(uuid::Uuid),
    /// Termin wurde übersprungen (steht im Protokoll).
    Skipped,
    /// Ein anderes automatisches Backup läuft noch; die Routine bleibt fällig und versucht es gleich erneut.
    Waiting,
}

/// Darf eine fällige Routine auf ein laufendes Backup warten – und ist es der erste Warte-Tick?
///
/// Gewartet wird, bis der *nächste* Termin derselben Routine erreicht ist. Dann ist der aktuelle
/// Termin hinfällig und wird übersprungen, damit sich nichts aufstaut.
pub fn wait_decision(
    due_since: Option<chrono::NaiveDateTime>,
    following_slot: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> (bool, bool) {
    let Some(due_since) = due_since else {
        // Neue Routine ohne Termin: warten, aber nicht bei jedem Tick protokollieren.
        return (true, false);
    };

    let can_wait = following_slot.is_none_or(|following| now < following);
    // Der Tick läuft alle 5 Sekunden.
    let first = now - due_since < chrono::Duration::seconds(5);

    (can_wait, can_wait && first)
}

async fn create_backup(
    state: &State,
    ctx: &mut Context,
    index: usize,
    trigger: Trigger,
) -> Result<Outcome, anyhow::Error> {
    let server_uuid = ctx.server.uuid;
    let routine_name = ctx.routines[index].name.clone();
    let reason = if ctx.server.suspended {
        Some("log.skipped_suspended")
    } else if matches!(
        ctx.server.status,
        Some(ServerStatus::Installing) | Some(ServerStatus::RestoringBackup)
    ) {
        Some("log.skipped_installing")
    } else if ctx.server.destination_node.is_some() {
        Some("log.skipped_transferring")
    } else if ManagedBackup::count_running(
        &state.database,
        ctx.group_uuid,
        now() - chrono::Duration::hours(STUCK_AFTER_HOURS),
    )
    .await?
        > 0
    {
        Some("log.skipped_running")
    } else {
        None
    };

    // Geplante Läufe warten auf ein noch laufendes Backup, statt ihren Termin zu verlieren.
    if reason == Some("log.skipped_running") && trigger == Trigger::Scheduled {
        let routine = &ctx.routines[index];
        let now = now();
        let following = routine.next_run.and_then(|due| {
            schedule::parse(&routine.cron)
                .ok()
                .and_then(|cron| schedule::next_after(&cron, ctx.timezone, due))
        });
        let (can_wait, first) = wait_decision(routine.next_run, following, now);

        if can_wait {
            if first {
                models::write_log(
                    &state.database,
                    server_uuid,
                    "info",
                    Msg::new("log.waiting").arg("routine", &routine_name),
                )
                .await;
            }
            return Ok(Outcome::Waiting);
        }
    }

    if let Some(reason) = reason {
        models::write_log(
            &state.database,
            server_uuid,
            "warning",
            Msg::new(reason).arg("routine", &routine_name),
        )
        .await;
        return Ok(Outcome::Skipped);
    }

    // Gleiche Sperre wie die Panel-eigene Backup-Erstellung, um Wettläufe zu vermeiden.
    let lock = state
        .cache
        .lock(
            format!("servers::{server_uuid}::backups"),
            Some(30),
            Some(5000),
        )
        .await?;

    // Reicht das Backup-Limit nicht, in dieser Routine vorab einen Platz freimachen.
    let routine_uuid = ctx.routines[index].uuid;
    let mut count = ServerBackup::count_by_server_uuid(&state.database, server_uuid).await?;
    if count >= ctx.server.backup_limit as i64
        && apply_retention(state, ctx, Some(routine_uuid)).await? > 0
    {
        count = ServerBackup::count_by_server_uuid(&state.database, server_uuid).await?;
    }
    if count >= ctx.server.backup_limit as i64 {
        drop(lock);
        let keep_total: i32 = ctx.routines.iter().map(|r| r.keep_count).sum();
        return Err(Msg::new("err.backup_limit")
            .arg("count", count)
            .arg("limit", ctx.server.backup_limit)
            .arg("needed", keep_total + 1)
            .into());
    }

    let routine = &ctx.routines[index];
    let metadata = ServerBackup::generate_metadata(state, &ctx.server).await?;
    let name = render_name(&routine.name_template, &ctx.server, &routine.name, ctx.timezone);

    let backup = ServerBackup::create(
        state,
        CreateServerBackupOptions {
            server: &ctx.server,
            name: name.clone(),
            backup_group_uuid: Some(ctx.group_uuid),
            system_backup_policy_uuid: None,
            database_instance: None,
            backup_configuration: None,
            ignored_files: routine
                .ignored_files
                .iter()
                .filter(|f| !f.trim().is_empty())
                .map(|f| f.trim().into())
                .collect(),
            metadata,
        },
    )
    .await?;

    drop(lock);

    models::set_routine(&state.database, server_uuid, backup.uuid, Some(routine.uuid)).await?;
    log_activity(
        state,
        server_uuid,
        "server:backup.create",
        serde_json::json!({
            "source": "backups-plus",
            "uuid": backup.uuid,
            "name": backup.name,
            "routine": routine.name,
            "backup_group_uuid": ctx.group_uuid,
        }),
    )
    .await;
    models::write_log(
        &state.database,
        server_uuid,
        "info",
        Msg::new("log.backup_started")
            .arg("name", &name)
            .arg("routine", &routine.name),
    )
    .await;

    Ok(Outcome::Created(backup.uuid))
}

/// Baut die Planungs-Eingaben aus Routinen und Backups.
pub fn build_specs(
    routines: &[Routine],
    backups: &[ManagedBackup],
    now: chrono::NaiveDateTime,
) -> (Vec<RoutineSpec>, Vec<BackupSpec>) {
    let routine_specs = routines
        .iter()
        .map(|r| RoutineSpec {
            uuid: r.uuid,
            name: r.name.clone(),
            enabled: r.enabled,
            position: r.position,
            keep: r.keep_count,
            max_age: (r.max_age_hours > 0).then(|| chrono::Duration::hours(r.max_age_hours as i64)),
            promote_from: match r.source {
                RoutineSource::Promote => r.source_routine_uuid,
                RoutineSource::Create => None,
            },
            due: r.source == RoutineSource::Promote && r.is_due(now),
        })
        .collect();

    let backup_specs = backups
        .iter()
        .map(|b| BackupSpec {
            uuid: b.uuid,
            routine: b.routine_uuid,
            created: b.created,
            completed: b.completed.is_some(),
            successful: b.successful,
            locked: b.locked,
        })
        .collect();

    (routine_specs, backup_specs)
}

/// Wendet die Rotation an. Gibt die Anzahl der angestoßenen Löschungen zurück.
async fn apply_retention(
    state: &State,
    ctx: &mut Context,
    reserve: Option<uuid::Uuid>,
) -> Result<u64, anyhow::Error> {
    let now = now();
    let server_uuid = ctx.server.uuid;
    let backups = ManagedBackup::all_for_group(&state.database, server_uuid, ctx.group_uuid).await?;
    let (routine_specs, backup_specs) = build_specs(&ctx.routines, &backups, now);
    let plan = retention::plan(
        &backup_specs,
        &routine_specs,
        ctx.policy.delete_failed,
        now,
        reserve,
    );

    let name_of = |uuid: uuid::Uuid| {
        backups
            .iter()
            .find(|b| b.uuid == uuid)
            .map(|b| b.name.clone())
            .unwrap_or_default()
    };
    let routine_name = |uuid: uuid::Uuid| {
        ctx.routines
            .iter()
            .find(|r| r.uuid == uuid)
            .map(|r| r.name.clone())
            .unwrap_or_default()
    };

    let mut deleted = 0;
    for (uuid, action) in &plan.actions {
        match action {
            PlannedAction::Move { from, to } => {
                models::set_routine(&state.database, server_uuid, *uuid, Some(*to)).await?;
                models::write_log(
                    &state.database,
                    server_uuid,
                    "info",
                    Msg::new("log.moved")
                        .arg("name", name_of(*uuid))
                        .arg("from", routine_name(*from))
                        .arg("to", routine_name(*to)),
                )
                .await;

                // Optional: dem Backup einen zur neuen Routine passenden Namen geben.
                if let Some(target) = ctx.routines.iter().find(|r| r.uuid == *to) {
                    match rename_for_routine(state, &ctx.server, ctx.timezone, *uuid, target).await {
                        Ok(Some((old, new))) => {
                            models::write_log(
                                &state.database,
                                server_uuid,
                                "info",
                                Msg::new("log.renamed").arg("old", old).arg("new", new),
                            )
                            .await;
                        }
                        Ok(None) => {}
                        Err(err) => {
                            models::write_log(
                                &state.database,
                                server_uuid,
                                "error",
                                Msg::new("log.rename_failed")
                                    .arg("name", name_of(*uuid))
                                    .arg_msg("error", &Msg::from_error(&err)),
                            )
                            .await;
                        }
                    }
                }
            }
            PlannedAction::Delete { reason } => {
                match delete_backup(state, server_uuid, *uuid, &reason.msg().render(crate::i18n::Lang::En)).await {
                    Ok(()) => {
                        deleted += 1;
                        models::write_log(
                            &state.database,
                            server_uuid,
                            "info",
                            Msg::new("log.deleted")
                                .arg("name", name_of(*uuid))
                                .arg_msg("reason", &reason.msg()),
                        )
                        .await;
                    }
                    Err(err) => {
                        models::write_log(
                            &state.database,
                            server_uuid,
                            "error",
                            Msg::new("log.delete_failed")
                                .arg("name", name_of(*uuid))
                                .arg_msg("error", &Msg::from_error(&err)),
                        )
                        .await;
                    }
                }
            }
            PlannedAction::Keep { .. }
            | PlannedAction::Pinned
            | PlannedAction::Running
            | PlannedAction::FailedKept
            | PlannedAction::Unassigned => {}
        }
    }

    // Routinen, die ein Backup übernommen haben, sind bis zum nächsten Termin erledigt.
    for routine in ctx.routines.iter_mut() {
        if plan.received.contains(&routine.uuid) {
            routine.last_run = Some(now);
            routine.last_error = None;
            routine.next_run = schedule::parse(&routine.cron)
                .ok()
                .and_then(|cron| schedule::next_after(&cron, ctx.timezone, now));
            routine.save_run_state(&state.database).await?;
        }
    }

    Ok(deleted)
}

async fn delete_backup(
    state: &State,
    server_uuid: uuid::Uuid,
    backup_uuid: uuid::Uuid,
    reason: &str,
) -> Result<(), anyhow::Error> {
    let Some(backup) =
        ServerBackup::by_server_uuid_uuid(&state.database, server_uuid, backup_uuid).await?
    else {
        return Ok(());
    };

    // Doppelte Absicherung: gesperrte Backups werden nie automatisch gelöscht.
    if backup.locked {
        return Ok(());
    }

    backup
        .delete(state, DeleteServerBackupOptions::default())
        .await?;

    log_activity(
        state,
        server_uuid,
        "server:backup.delete",
        serde_json::json!({
            "source": "backups-plus",
            "uuid": backup.uuid,
            "name": backup.name,
            "reason": reason,
        }),
    )
    .await;

    Ok(())
}

async fn log_activity(state: &State, server_uuid: uuid::Uuid, event: &str, data: serde_json::Value) {
    if let Err(err) = ServerActivity::create(
        state,
        CreateServerActivityOptions {
            server_uuid,
            user_uuid: None,
            impersonator_uuid: None,
            api_key_uuid: None,
            schedule_uuid: None,
            event: event.into(),
            ip: None,
            data,
            created: None,
        },
    )
    .await
    {
        tracing::warn!(server = %server_uuid, "backups-plus: failed to log activity: {err:#?}");
    }
}

#[cfg(test)]
mod tests {
    use super::wait_decision;

    fn at(h: u32, m: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn waits_until_the_next_slot_and_logs_once() {
        // Fällig seit 04:00, nächster Termin derselben Routine 05:00.
        assert_eq!(wait_decision(Some(at(4, 0)), Some(at(5, 0)), at(4, 0)), (true, true));
        assert_eq!(wait_decision(Some(at(4, 0)), Some(at(5, 0)), at(4, 30)), (true, false));
        // Der nächste Termin ist erreicht: dieser wird übersprungen.
        assert_eq!(wait_decision(Some(at(4, 0)), Some(at(5, 0)), at(5, 0)), (false, false));
    }

    #[test]
    fn rename_template_uses_creation_time_and_original_name() {
        use super::render_template;

        // Erstellt 24.11. 23:30 UTC = 25.11. 00:30 in Berlin.
        let created = chrono::NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(23, 30, 0)
            .unwrap()
            .and_utc()
            .with_timezone(&chrono_tz::Europe::Berlin);

        assert_eq!(
            render_template("{routine} {date}", "Survival", "Weekly", "Auto Hourly x", created).as_deref(),
            Some("Weekly 2026-11-25 00:30")
        );
        assert_eq!(
            render_template("{original} (kept as {routine})", "Survival", "Monthly", "Auto Hourly x", created)
                .as_deref(),
            Some("Auto Hourly x (kept as Monthly)")
        );
        assert_eq!(render_template("   ", "S", "R", "O", created), None);
        assert_eq!(
            render_template("{server} – {routine}", "Survival", "Weekly", "", created).as_deref(),
            Some("Survival – Weekly")
        );
    }

    #[test]
    fn new_routines_wait_silently() {
        assert_eq!(wait_decision(None, None, at(4, 0)), (true, false));
    }
}
