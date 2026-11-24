//! Übersetzbare Backend-Meldungen (Englisch als Basis, Deutsch zusätzlich).
//!
//! Meldungen werden als Schlüssel + Parameter erzeugt und erst beim Ausliefern in der
//! Sprache des anfragenden Nutzers formuliert. Protokolleinträge und gespeicherte Fehler
//! liegen deshalb als kleines JSON in der Datenbank (`{"k": "...", "p": {...}}`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

impl Lang {
    /// Sprache aus dem Panel-Sprachcode des Nutzers (z. B. `de`, `de-DE`, `en`).
    pub fn from_code(code: &str) -> Self {
        if code.to_ascii_lowercase().starts_with("de") {
            Lang::De
        } else {
            Lang::En
        }
    }
}

/// Eine übersetzbare Meldung.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Msg {
    #[serde(rename = "k")]
    pub key: String,
    #[serde(rename = "p", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl Msg {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.into(),
            params: BTreeMap::new(),
        }
    }

    /// Parameter setzen. Werte, die selbst Meldungen sind, mit [`Msg::arg_msg`] übergeben.
    pub fn arg(mut self, name: &str, value: impl ToString) -> Self {
        self.params.insert(name.into(), value.to_string());
        self
    }

    /// Verschachtelte Meldung als Parameter (wird beim Rendern mit übersetzt).
    pub fn arg_msg(mut self, name: &str, value: &Msg) -> Self {
        self.params.insert(format!("{name}@"), value.to_json());
        self
    }

    /// Freier Text, der nicht übersetzt wird (z. B. Fehler aus dem Panel selbst).
    pub fn raw(text: impl ToString) -> Self {
        Self::new("raw").arg("text", text)
    }

    /// Meldung aus einem Fehler: eigene Meldungen bleiben übersetzbar, alles andere wird roher Text.
    pub fn from_error(err: &anyhow::Error) -> Self {
        match err.downcast_ref::<MsgError>() {
            Some(MsgError(msg)) => msg.clone(),
            None => Self::raw(format!("{err:#}")),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Gespeicherten Text lesen: JSON-Meldung oder (ältere Einträge) freier Text.
    pub fn from_stored(text: &str) -> Self {
        if text.starts_with("{\"k\"")
            && let Ok(msg) = serde_json::from_str(text)
        {
            return msg;
        }
        Self::raw(text)
    }

    pub fn render(&self, lang: Lang) -> String {
        // Unbekannte Schlüssel (z. B. aus einer neueren Version) als Schlüssel anzeigen.
        let Some(pattern) = template(&self.key, lang) else {
            return self.key.clone();
        };

        let mut out = pattern.to_string();
        for (name, value) in &self.params {
            match name.strip_suffix('@') {
                Some(name) => {
                    let nested = Msg::from_stored(value).render(lang);
                    out = out.replace(&format!("{{{name}}}"), &nested);
                }
                None => out = out.replace(&format!("{{{name}}}"), value),
            }
        }

        out
    }
}

/// Fehler, der eine übersetzbare Meldung trägt.
#[derive(Debug)]
pub struct MsgError(pub Msg);

impl std::fmt::Display for MsgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.render(Lang::En))
    }
}

impl std::error::Error for MsgError {}

impl From<Msg> for anyhow::Error {
    fn from(msg: Msg) -> Self {
        anyhow::Error::new(MsgError(msg))
    }
}

fn template(key: &str, lang: Lang) -> Option<&'static str> {
    use Lang::*;

    Some(match (key, lang) {
        ("raw", _) => "{text}",

        // Protokoll
        ("log.group_created", En) => "Created backup group \"{group}\"",
        ("log.group_created", De) => "Backup-Gruppe \"{group}\" angelegt",
        ("log.skipped_suspended", En) => "\"{routine}\" skipped: the server is suspended",
        ("log.skipped_suspended", De) => "\"{routine}\" übersprungen: Server ist gesperrt",
        ("log.skipped_installing", En) => "\"{routine}\" skipped: the server is being installed or restored",
        ("log.skipped_installing", De) => "\"{routine}\" übersprungen: Server wird gerade installiert oder wiederhergestellt",
        ("log.skipped_transferring", En) => "\"{routine}\" skipped: the server is being transferred",
        ("log.skipped_transferring", De) => "\"{routine}\" übersprungen: Server wird gerade übertragen",
        ("log.waiting", En) => "\"{routine}\" waits for the running backup to finish",
        ("log.waiting", De) => "\"{routine}\" wartet, bis das laufende Backup fertig ist",
        ("log.skipped_running", En) => "\"{routine}\" skipped: a previous automatic backup is still running",
        ("log.skipped_running", De) => "\"{routine}\" übersprungen: ein vorheriges automatisches Backup läuft noch",
        ("log.backup_started", En) => "Started \"{name}\" for routine \"{routine}\"",
        ("log.backup_started", De) => "\"{name}\" für Routine \"{routine}\" gestartet",
        ("log.moved", En) => "\"{name}\" moved from \"{from}\" to \"{to}\"",
        ("log.moved", De) => "\"{name}\" von \"{from}\" nach \"{to}\" übernommen",
        ("log.deleted", En) => "\"{name}\" deleted ({reason})",
        ("log.deleted", De) => "\"{name}\" gelöscht ({reason})",
        ("log.delete_failed", En) => "\"{name}\" could not be deleted: {error}",
        ("log.delete_failed", De) => "\"{name}\" konnte nicht gelöscht werden: {error}",
        ("log.renamed", En) => "\"{old}\" renamed to \"{new}\"",
        ("log.renamed", De) => "\"{old}\" umbenannt in \"{new}\"",
        ("log.rename_failed", En) => "\"{name}\" could not be renamed: {error}",
        ("log.rename_failed", De) => "\"{name}\" konnte nicht umbenannt werden: {error}",
        ("log.pinned", En) => "\"{name}\" pinned (will never be deleted automatically)",
        ("log.pinned", De) => "\"{name}\" angepinnt (wird nie automatisch gelöscht)",
        ("log.unpinned", En) => "\"{name}\" unpinned (part of the rotation again)",
        ("log.unpinned", De) => "\"{name}\" losgelöst (unterliegt wieder der Rotation)",
        ("log.assigned", En) => "\"{name}\" manually assigned to routine \"{routine}\"",
        ("log.assigned", De) => "\"{name}\" manuell der Routine \"{routine}\" zugeordnet",
        ("log.unassigned", En) => "\"{name}\" removed from the rotation (no routine)",
        ("log.unassigned", De) => "\"{name}\" aus der Rotation genommen (keiner Routine zugeordnet)",
        ("log.template_applied", En) => "Template \"{template}\" applied",
        ("log.template_applied", De) => "Vorlage \"{template}\" angewendet",
        ("log.template_applied_admin", En) => "Template \"{template}\" applied by an administrator",
        ("log.template_applied_admin", De) => "Vorlage \"{template}\" vom Admin angewendet",
        ("log.manual_run_failed", En) => "Manual run failed: {error}",
        ("log.manual_run_failed", De) => "Manueller Lauf fehlgeschlagen: {error}",

        // Löschgründe
        ("reason.failed", En) => "backup failed",
        ("reason.failed", De) => "Backup ist fehlgeschlagen",
        ("reason.too_old", En) => "older than the maximum age of routine \"{routine}\"",
        ("reason.too_old", De) => "älter als das Höchstalter der Routine \"{routine}\"",
        ("reason.over_keep", En) => "older than the {keep} newest backups of routine \"{routine}\"",
        ("reason.over_keep", De) => "älter als die {keep} neuesten Backups der Routine \"{routine}\"",

        // Fehler
        ("err.no_create_routine", En) => "There is no routine that creates new backups.",
        ("err.no_create_routine", De) => "Es gibt keine Routine, die neue Backups erstellt.",
        ("err.routine_is_promote", En) => "Routine \"{routine}\" takes over backups from another routine and doesn't create its own.",
        ("err.routine_is_promote", De) => "Die Routine \"{routine}\" übernimmt Backups aus einer anderen Routine und erstellt selbst keine.",
        ("err.backup_limit", En) => "The server's backup limit has been reached ({count}/{limit}). Raise the limit to at least {needed} or keep fewer backups.",
        ("err.backup_limit", De) => "Backup-Limit des Servers erreicht ({count}/{limit}). Erhöhe das Limit auf mindestens {needed} oder reduziere die Anzahl behaltener Backups.",
        ("err.backup_not_found", En) => "Backup not found.",
        ("err.backup_not_found", De) => "Backup nicht gefunden.",
        ("err.routine_not_found", En) => "Routine not found.",
        ("err.routine_not_found", De) => "Routine nicht gefunden.",
        ("err.template_not_found", En) => "Template not found.",
        ("err.template_not_found", De) => "Vorlage nicht gefunden.",
        ("err.template_doesnt_fit", En) => "The template doesn't fit: {error}",
        ("err.template_doesnt_fit", De) => "Vorlage passt nicht: {error}",
        ("err.template_invalid", En) => "Template \"{template}\": {error}",
        ("err.template_invalid", De) => "Vorlage \"{template}\": {error}",
        ("err.duplicate_template_id", En) => "The template ID \"{id}\" is used twice.",
        ("err.duplicate_template_id", De) => "Die Vorlagen-ID \"{id}\" ist doppelt.",
        ("err.invalid_timezone", En) => "Unknown time zone \"{timezone}\".",
        ("err.invalid_timezone", De) => "Unbekannte Zeitzone \"{timezone}\".",
        ("err.default_template_missing", En) => "The default template doesn't exist.",
        ("err.default_template_missing", De) => "Die Standard-Vorlage existiert nicht.",
        ("err.duplicate_key", En) => "Duplicate routine key.",
        ("err.duplicate_key", De) => "Doppelter Routinen-Schlüssel.",
        ("err.no_routines", En) => "Please add at least one routine or apply a template first.",
        ("err.no_routines", De) => "Bitte zuerst mindestens eine Routine anlegen oder eine Vorlage anwenden.",
        ("err.skipped_see_log", En) => "The backup was skipped – see the log for details.",
        ("err.skipped_see_log", De) => "Backup wurde übersprungen – Details stehen im Protokoll.",

        // Validierung
        ("val.too_many_routines", En) => "At most {max} routines per server are allowed.",
        ("val.too_many_routines", De) => "Es sind höchstens {max} Routinen pro Server erlaubt.",
        ("val.name_required", En) => "Every routine needs a name (1–64 characters).",
        ("val.name_required", De) => "Jede Routine braucht einen Namen (1–64 Zeichen).",
        ("val.name_duplicate", En) => "The routine name \"{name}\" is used twice.",
        ("val.name_duplicate", De) => "Der Routinen-Name \"{name}\" ist doppelt vergeben.",
        ("val.keep_range", En) => "\"{name}\": the number of backups to keep must be between 1 and 500.",
        ("val.keep_range", De) => "\"{name}\": Anzahl behaltener Backups muss zwischen 1 und 500 liegen.",
        ("val.max_age_range", En) => "\"{name}\": the maximum age must be between 0 and 87600 hours.",
        ("val.max_age_range", De) => "\"{name}\": Höchstalter muss zwischen 0 und 87600 Stunden liegen.",
        ("val.name_template", En) => "\"{name}\": the name template must be 1–200 characters long.",
        ("val.name_template", De) => "\"{name}\": Namensvorlage muss 1–200 Zeichen lang sein.",
        ("val.rename_template", En) => "\"{name}\": the rename template may be at most 200 characters long.",
        ("val.rename_template", De) => "\"{name}\": Die Umbenennungs-Vorlage darf höchstens 200 Zeichen lang sein.",
        ("val.invalid_cron", En) => "\"{name}\": invalid schedule \"{cron}\" ({error}).",
        ("val.invalid_cron", De) => "\"{name}\": ungültiger Zeitplan \"{cron}\" ({error}).",
        ("val.no_future_runs", En) => "\"{name}\": the schedule has no upcoming runs.",
        ("val.no_future_runs", De) => "\"{name}\": Der Zeitplan hat keine zukünftigen Termine.",
        ("val.min_interval", En) => "\"{name}\": backups may be created at most every {interval}.",
        ("val.min_interval", De) => "\"{name}\": Backups dürfen höchstens alle {interval} erstellt werden.",
        ("val.source_missing", En) => "\"{name}\": please choose a source routine to take backups over from.",
        ("val.source_missing", De) => "\"{name}\": Bitte eine Quell-Routine auswählen, aus der Backups übernommen werden.",
        ("val.source_cycle", En) => "\"{name}\": the source routines form a cycle.",
        ("val.source_cycle", De) => "\"{name}\": Die Quell-Routinen bilden einen Kreis.",
        ("val.keep_total", En) => "At most {max} backups per server may be kept (currently {total}).",
        ("val.keep_total", De) => "Es dürfen höchstens {max} Backups pro Server behalten werden (aktuell {total}).",
        ("val.backup_limit", En) => "This server's backup limit is {limit}, but the routines would need {total} slots.",
        ("val.backup_limit", De) => "Das Backup-Limit dieses Servers ist {limit}, die Routinen bräuchten aber {total} Plätze.",
        ("val.template_source_unknown", En) => "Routine \"{routine}\": source routine \"{source}\" is missing in the template.",
        ("val.template_source_unknown", De) => "Routine \"{routine}\": Quell-Routine \"{source}\" fehlt in der Vorlage.",
        ("val.template_source_missing", En) => "Routine \"{routine}\": source routine is missing.",
        ("val.template_source_missing", De) => "Routine \"{routine}\": Quell-Routine fehlt.",

        // Eingebaute Vorlagen
        ("tpl.gfs_compact.name", En) => "GFS compact (5 backups)",
        ("tpl.gfs_compact.name", De) => "GFS kompakt (5 Backups)",
        ("tpl.gfs_compact.description", En) => "The last 3 hours, plus a weekly backup (renewed every Monday) and a monthly backup (renewed on the 1st). Older backups are deleted.",
        ("tpl.gfs_compact.description", De) => "Die letzten 3 Stunden, dazu ein wöchentliches (jeden Montag erneuert) und ein monatliches (jeden 1. erneuert) Backup. Ältere werden gelöscht.",
        ("tpl.gfs_extended.name", En) => "GFS extended (41 backups)",
        ("tpl.gfs_extended.name", De) => "GFS erweitert (41 Backups)",
        ("tpl.gfs_extended.description", En) => "24 hourly, 7 daily, 4 weekly and 6 monthly backups.",
        ("tpl.gfs_extended.description", De) => "24 stündliche, 7 tägliche, 4 wöchentliche und 6 monatliche Backups.",
        ("tpl.daily_7.name", En) => "Daily, 7 days",
        ("tpl.daily_7.name", De) => "Täglich, 7 Tage",
        ("tpl.daily_7.description", En) => "A backup every night at 04:00, the last 7 are kept.",
        ("tpl.daily_7.description", De) => "Jede Nacht um 04:00 ein Backup, die letzten 7 werden behalten.",
        ("tpl.every_6h.name", En) => "Every 6 hours, 2 days",
        ("tpl.every_6h.name", De) => "Alle 6 Stunden, 2 Tage",
        ("tpl.every_6h.description", En) => "A backup every 6 hours, at most 2 days old.",
        ("tpl.every_6h.description", De) => "Alle 6 Stunden ein Backup, maximal 2 Tage alt.",
        ("routine.hourly", En) => "Hourly",
        ("routine.hourly", De) => "Stündlich",
        ("routine.daily", En) => "Daily",
        ("routine.daily", De) => "Täglich",
        ("routine.weekly", En) => "Weekly",
        ("routine.weekly", De) => "Wöchentlich",
        ("routine.monthly", En) => "Monthly",
        ("routine.monthly", De) => "Monatlich",
        ("routine.every_6h", En) => "Every 6 hours",
        ("routine.every_6h", De) => "Alle 6 Stunden",

        _ => return None,
    })
}

/// Kurzform: Meldung direkt übersetzen.
pub fn t(key: &str, lang: Lang) -> String {
    Msg::new(key).render(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_params_and_nested_messages() {
        let reason = Msg::new("reason.over_keep").arg("keep", 3).arg("routine", "Hourly");
        let msg = Msg::new("log.deleted").arg("name", "B1").arg_msg("reason", &reason);

        assert_eq!(
            msg.render(Lang::En),
            "\"B1\" deleted (older than the 3 newest backups of routine \"Hourly\")"
        );
        assert_eq!(
            msg.render(Lang::De),
            "\"B1\" gelöscht (älter als die 3 neuesten Backups der Routine \"Hourly\")"
        );
    }

    #[test]
    fn stored_round_trip_and_legacy_text() {
        let msg = Msg::new("log.pinned").arg("name", "X");
        assert_eq!(Msg::from_stored(&msg.to_json()), msg);
        assert_eq!(Msg::from_stored("alter Text").render(Lang::En), "alter Text");
    }

    #[test]
    fn errors_keep_their_message() {
        let err: anyhow::Error = Msg::new("err.backup_not_found").into();
        assert_eq!(Msg::from_error(&err).render(Lang::De), "Backup nicht gefunden.");
        assert_eq!(err.to_string(), "Backup not found.");
    }

    #[test]
    fn language_codes() {
        assert_eq!(Lang::from_code("de"), Lang::De);
        assert_eq!(Lang::from_code("de-AT"), Lang::De);
        assert_eq!(Lang::from_code("fr"), Lang::En);
    }
}
