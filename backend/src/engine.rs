//! Ausführung: Backups anlegen, befördern und löschen.

use crate::{
    models::{self, ManagedBackup, Policy, Tier},
    retention::{self, PlannedAction},
    settings,
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

/// Nächster Termin: volle Stunde + Intervall + Minuten-Versatz.
pub fn next_run_after(policy: &Policy, from: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    use chrono::Timelike;

    let hour_start = from
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(from);
    let offset = chrono::Duration::minutes(policy.minute_offset.clamp(0, 59) as i64);

    let mut next = hour_start + offset;
    while next <= from {
        next += chrono::Duration::hours(policy.interval_hours.max(1) as i64);
    }

    next
}

/// Wird jede Minute vom Hintergrund-Task aufgerufen.
pub async fn tick(state: State) -> Result<(), anyhow::Error> {
    let settings = settings::load(&state).await;
    if !settings.enabled {
        return Ok(());
    }

    let now = now();

    // 1. Fällige Richtlinien: aufräumen + neues Backup erstellen.
    let due = Policy::all_due(&state.database, now).await?;
    let mut handled = std::collections::HashSet::new();

    for (index, mut policy) in due.into_iter().enumerate() {
        handled.insert(policy.server_uuid);

        // Lastverteilung: Überzählige im nächsten Tick erledigen.
        if index as u32 >= settings.max_backups_per_tick {
            continue;
        }

        if let Err(err) = run_policy(&state, &mut policy, true).await {
            tracing::error!(server = %policy.server_uuid, "backup-plus: Lauf fehlgeschlagen: {err:#?}");
            models::write_log(&state.database, policy.server_uuid, "error", format!("{err:#}"))
                .await;
            policy.last_error = Some(format!("{err:#}"));
            policy.last_run = Some(now);
            policy.next_run = Some(next_run_after(&policy, now));
            policy.save_run_state(&state.database).await?;
        }
    }

    // 2. Alle 5 Minuten: Bereinigung auch für nicht fällige Richtlinien
    //    (damit gerade fertig gewordene Backups zeitnah rotiert werden).
    use chrono::Timelike;
    if now.minute().is_multiple_of(5) {
        for policy in Policy::all_enabled(&state.database).await? {
            if handled.contains(&policy.server_uuid) {
                continue;
            }

            if let Err(err) = apply_retention(&state, &policy, 0).await {
                tracing::warn!(server = %policy.server_uuid, "backup-plus: Bereinigung fehlgeschlagen: {err:#?}");
            }
        }

        models::trim_log(&state.database, settings.log_retention as i64).await?;
    }

    Ok(())
}

/// Führt eine Richtlinie aus. Mit `create = true` wird zusätzlich ein neues Backup erstellt.
pub async fn run_policy(
    state: &State,
    policy: &mut Policy,
    create: bool,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let now = now();
    let server = Server::by_uuid(&state.database, policy.server_uuid).await?;

    let group_uuid = ensure_group(state, policy).await?;

    let mut created = None;
    if create {
        created = create_backup(state, policy, &server, group_uuid).await?;
    } else {
        apply_retention(state, policy, 0).await?;
    }

    policy.last_run = Some(now);
    policy.last_error = None;
    policy.next_run = Some(next_run_after(policy, now));
    policy.save_run_state(&state.database).await?;

    Ok(created)
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
    policy.save_run_state(&state.database).await?;
    models::write_log(
        &state.database,
        policy.server_uuid,
        "info",
        format!("Backup-Gruppe \"{GROUP_NAME}\" angelegt"),
    )
    .await;

    Ok(group.uuid)
}

fn render_name(template: &str, server: &Server, tier: Tier) -> compact_str::CompactString {
    let now = chrono::Utc::now();
    let name = template
        .replace("{date}", &now.format("%Y-%m-%d %H:%M").to_string())
        .replace("{time}", &now.format("%H:%M").to_string())
        .replace("{day}", &now.format("%Y-%m-%d").to_string())
        .replace("{tier}", tier.label())
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
    policy: &Policy,
    server: &Server,
    group_uuid: uuid::Uuid,
) -> Result<Option<uuid::Uuid>, anyhow::Error> {
    let log = |level: &'static str, msg: String| models::write_log(&state.database, server.uuid, level, msg);

    if server.suspended {
        log("warning", "Übersprungen: Server ist gesperrt".into()).await;
        apply_retention(state, policy, 0).await?;
        return Ok(None);
    }
    if matches!(
        server.status,
        Some(ServerStatus::Installing) | Some(ServerStatus::RestoringBackup)
    ) {
        log(
            "warning",
            "Übersprungen: Server wird gerade installiert oder wiederhergestellt".into(),
        )
        .await;
        apply_retention(state, policy, 0).await?;
        return Ok(None);
    }
    if server.destination_node.is_some() {
        log("warning", "Übersprungen: Server wird gerade übertragen".into()).await;
        apply_retention(state, policy, 0).await?;
        return Ok(None);
    }

    let running = ManagedBackup::count_running(
        &state.database,
        group_uuid,
        now() - chrono::Duration::hours(STUCK_AFTER_HOURS),
    )
    .await?;
    if running > 0 {
        log(
            "warning",
            "Übersprungen: Das vorherige automatische Backup läuft noch".into(),
        )
        .await;
        apply_retention(state, policy, 0).await?;
        return Ok(None);
    }

    // Gleiche Sperre wie die Panel-eigene Backup-Erstellung, um Wettläufe zu vermeiden.
    let lock = state
        .cache
        .lock(
            format!("servers::{}::backups", server.uuid),
            Some(30),
            Some(5000),
        )
        .await?;

    // Zuerst normal aufräumen; reicht das Limit danach nicht, einen stündlichen Platz freimachen.
    apply_retention(state, policy, 0).await?;
    let mut count = ServerBackup::count_by_server_uuid(&state.database, server.uuid).await?;
    if count >= server.backup_limit as i64 && apply_retention(state, policy, 1).await? > 0 {
        // Gelöschte Backups sind sofort als "deleting" markiert und zählen nicht mehr.
        count = ServerBackup::count_by_server_uuid(&state.database, server.uuid).await?;
    }
    if count >= server.backup_limit as i64 {
        drop(lock);
        return Err(anyhow::anyhow!(
            "Backup-Limit des Servers erreicht ({count}/{}). Erhöhe das Limit auf mindestens {} oder reduziere die Anzahl behaltener Backups.",
            server.backup_limit,
            policy.keep_total() + 1
        ));
    }

    let metadata = ServerBackup::generate_metadata(state, server).await?;
    let name = render_name(&policy.name_template, server, Tier::Hourly);

    let backup = ServerBackup::create(
        state,
        CreateServerBackupOptions {
            server,
            name: name.clone(),
            backup_group_uuid: Some(group_uuid),
            system_backup_policy_uuid: None,
            database_instance: None,
            backup_configuration: None,
            ignored_files: policy
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

    models::set_tier(&state.database, server.uuid, backup.uuid, Tier::Hourly, false).await?;
    log_activity(
        state,
        server.uuid,
        "server:backup.create",
        serde_json::json!({
            "source": "backup-plus",
            "uuid": backup.uuid,
            "name": backup.name,
            "backup_group_uuid": group_uuid,
        }),
    )
    .await;
    log("info", format!("Backup \"{name}\" gestartet")).await;

    Ok(Some(backup.uuid))
}

/// Wendet die Rotation an. Gibt die Anzahl der angestoßenen Löschungen zurück.
pub async fn apply_retention(
    state: &State,
    policy: &Policy,
    reserve_hourly: i32,
) -> Result<u64, anyhow::Error> {
    let Some(group_uuid) = policy.backup_group_uuid else {
        return Ok(0);
    };

    let backups = ManagedBackup::all_for_group(&state.database, policy.server_uuid, group_uuid).await?;
    let names: std::collections::HashMap<_, _> =
        backups.iter().map(|b| (b.uuid, b.name.clone())).collect();
    let actions = retention::plan(&backups, policy, reserve_hourly);

    let mut deleted = 0;
    for (uuid, action) in actions {
        let name = names.get(&uuid).cloned().unwrap_or_default();

        match action {
            PlannedAction::Promote { from, to } => {
                models::set_tier(&state.database, policy.server_uuid, uuid, to, true).await?;
                models::write_log(
                    &state.database,
                    policy.server_uuid,
                    "info",
                    format!(
                        "\"{name}\" von {} zu {} befördert",
                        from.label(),
                        to.label()
                    ),
                )
                .await;
            }
            PlannedAction::Keep { tier } => {
                // Fehlende Zuordnung (z.B. manuell in die Gruppe verschoben) nachtragen.
                if backups.iter().any(|b| b.uuid == uuid && b.tier != tier) {
                    models::set_tier(&state.database, policy.server_uuid, uuid, tier, false).await?;
                }
            }
            PlannedAction::Delete { reason } => {
                match delete_backup(state, policy.server_uuid, uuid, &reason).await {
                    Ok(()) => {
                        deleted += 1;
                        models::write_log(
                            &state.database,
                            policy.server_uuid,
                            "info",
                            format!("\"{name}\" gelöscht ({reason})"),
                        )
                        .await;
                    }
                    Err(err) => {
                        models::write_log(
                            &state.database,
                            policy.server_uuid,
                            "error",
                            format!("\"{name}\" konnte nicht gelöscht werden: {err:#}"),
                        )
                        .await;
                    }
                }
            }
            PlannedAction::Pinned | PlannedAction::Running | PlannedAction::FailedKept => {}
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
