//! Ausführung: Backups anlegen, zwischen Routinen verschieben und löschen.

use crate::{
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

pub const GROUP_NAME: &str = "Backup Plus (automatisch)";

/// Laufende Backups, die älter sind, gelten als hängengeblieben und blockieren nicht mehr.
const STUCK_AFTER_HOURS: i64 = 6;

#[inline]
pub fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Was einen Lauf ausgelöst hat.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Minütlicher Hintergrund-Task: fällige Routinen ausführen.
    Scheduled,
    /// "Jetzt sichern": bestimmte Routine (oder die erste erstellende) sofort ausführen.
    Manual(Option<uuid::Uuid>),
}

/// Wird jede Minute vom Hintergrund-Task aufgerufen.
pub async fn tick(state: State) -> Result<(), anyhow::Error> {
    let settings = settings::load(&state).await;
    if !settings.enabled {
        return Ok(());
    }

    let now = now();
    let due = Policy::servers_due(&state.database, now).await?;
    let mut handled = std::collections::HashSet::new();

    for (index, server_uuid) in due.into_iter().enumerate() {
        // Lastverteilung: Überzählige im nächsten Tick erledigen.
        if index as u32 >= settings.max_backups_per_tick {
            break;
        }
        handled.insert(server_uuid);

        if let Err(err) = run_server(&state, server_uuid, Trigger::Scheduled).await {
            tracing::error!(server = %server_uuid, "backup-plus: Lauf fehlgeschlagen: {err:#?}");
            models::write_log(&state.database, server_uuid, "error", format!("{err:#}")).await;
        }
    }

    // Alle 5 Minuten: Rotation für alle aktiven Server (fertig gewordene Backups einsortieren).
    use chrono::Timelike;
    if now.minute().is_multiple_of(5) {
        for server_uuid in Policy::servers_enabled(&state.database).await? {
            if handled.contains(&server_uuid) {
                continue;
            }

            if let Err(err) = run_retention(&state, server_uuid).await {
                tracing::warn!(server = %server_uuid, "backup-plus: Bereinigung fehlgeschlagen: {err:#?}");
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
}

async fn load_context(state: &State, server_uuid: uuid::Uuid) -> Result<Context, anyhow::Error> {
    let mut policy = Policy::by_server_uuid(&state.database, server_uuid)
        .await?
        .unwrap_or_else(|| Policy::default_for(server_uuid));
    let server = Server::by_uuid(&state.database, server_uuid).await?;
    let timezone = schedule::server_timezone(server.timezone.as_deref());
    let group_uuid = ensure_group(state, &mut policy).await?;
    let routines = Routine::all_for_server(&state.database, server_uuid).await?;

    Ok(Context {
        policy,
        server,
        timezone,
        group_uuid,
        routines,
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
            return Err(anyhow::anyhow!(
                "Es gibt keine Routine, die neue Backups erstellt."
            ));
        }
        return Ok(None);
    };

    if ctx.routines[index].source != RoutineSource::Create {
        return Err(anyhow::anyhow!(
            "Die Routine \"{}\" übernimmt Backups aus einer anderen Routine und erstellt selbst keine.",
            ctx.routines[index].name
        ));
    }

    let result = create_backup(state, &mut ctx, index).await;

    // Beim geplanten Lauf den nächsten Termin setzen – auch bei Fehlern, damit nichts in Schleife läuft.
    if trigger == Trigger::Scheduled {
        let routine = &mut ctx.routines[index];
        routine.next_run = schedule::parse(&routine.cron)
            .ok()
            .and_then(|cron| schedule::next_after(&cron, ctx.timezone, now));
        routine.last_run = Some(now);
        routine.last_error = result.as_ref().err().map(|err| format!("{err:#}"));
        routine.save_run_state(&state.database).await?;
    } else if result.is_ok() {
        let routine = &mut ctx.routines[index];
        routine.last_run = Some(now);
        routine.last_error = None;
        routine.save_run_state(&state.database).await?;
    }

    ctx.policy.last_run = Some(now);
    ctx.policy.last_error = result.as_ref().err().map(|err| format!("{err:#}"));
    ctx.policy.save_run_state(&state.database).await?;

    result
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
        format!("Backup-Gruppe \"{GROUP_NAME}\" angelegt"),
    )
    .await;

    Ok(group.uuid)
}

fn render_name(
    template: &str,
    server: &Server,
    routine: &str,
    timezone: chrono_tz::Tz,
) -> compact_str::CompactString {
    let now = chrono::Utc::now().with_timezone(&timezone);
    let name = template
        .replace("{date}", &now.format("%Y-%m-%d %H:%M").to_string())
        .replace("{time}", &now.format("%H:%M").to_string())
        .replace("{day}", &now.format("%Y-%m-%d").to_string())
        .replace("{routine}", routine)
        .replace("{tier}", routine)
        .replace("{server}", &server.name);

    let name = name.trim();
    let name: String = if name.is_empty() {
        ServerBackup::default_name().to_string()
    } else {
        name.chars().take(255).collect()
    };

    name.into()
}

async fn create_backup(
    state: &State,
    ctx: &mut Context,
    index: usize,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let server_uuid = ctx.server.uuid;
    let routine_name = ctx.routines[index].name.clone();
    let skip = |reason: &str| {
        format!("\"{routine_name}\" übersprungen: {reason}")
    };

    let reason = if ctx.server.suspended {
        Some("Server ist gesperrt")
    } else if matches!(
        ctx.server.status,
        Some(ServerStatus::Installing) | Some(ServerStatus::RestoringBackup)
    ) {
        Some("Server wird gerade installiert oder wiederhergestellt")
    } else if ctx.server.destination_node.is_some() {
        Some("Server wird gerade übertragen")
    } else if ManagedBackup::count_running(
        &state.database,
        ctx.group_uuid,
        now() - chrono::Duration::hours(STUCK_AFTER_HOURS),
    )
    .await?
        > 0
    {
        Some("ein vorheriges automatisches Backup läuft noch")
    } else {
        None
    };

    if let Some(reason) = reason {
        models::write_log(&state.database, server_uuid, "warning", skip(reason)).await;
        return Ok(None);
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
        return Err(anyhow::anyhow!(
            "Backup-Limit des Servers erreicht ({count}/{}). Erhöhe das Limit auf mindestens {} oder reduziere die Anzahl behaltener Backups.",
            ctx.server.backup_limit,
            keep_total + 1
        ));
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
            "source": "backup-plus",
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
        format!("\"{name}\" für Routine \"{}\" gestartet", routine.name),
    )
    .await;

    Ok(Some(backup.uuid))
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
                    format!(
                        "\"{}\" von \"{}\" nach \"{}\" übernommen",
                        name_of(*uuid),
                        routine_name(*from),
                        routine_name(*to)
                    ),
                )
                .await;
            }
            PlannedAction::Delete { reason } => {
                match delete_backup(state, server_uuid, *uuid, reason).await {
                    Ok(()) => {
                        deleted += 1;
                        models::write_log(
                            &state.database,
                            server_uuid,
                            "info",
                            format!("\"{}\" gelöscht ({reason})", name_of(*uuid)),
                        )
                        .await;
                    }
                    Err(err) => {
                        models::write_log(
                            &state.database,
                            server_uuid,
                            "error",
                            format!("\"{}\" konnte nicht gelöscht werden: {err:#}", name_of(*uuid)),
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
            "source": "backup-plus",
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
        tracing::warn!(server = %server_uuid, "backup-plus: Aktivität nicht protokolliert: {err:#?}");
    }
}
