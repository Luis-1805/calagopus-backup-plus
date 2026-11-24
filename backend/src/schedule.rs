//! Cron-Zeitpläne (mit Sekundenfeld, wie im Panel üblich) in der Zeitzone des Servers.

use std::str::FromStr;

/// Parst einen Cron-Ausdruck mit 5 oder 6 Feldern.
pub fn parse(expression: &str) -> Result<croner::Cron, String> {
    croner::Cron::from_str(expression.trim())
        .map_err(|err| err.to_string())
}

/// Woher die verwendete Zeitzone stammt.
#[derive(utoipa::ToSchema, serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimezoneSource {
    /// In den Einstellungen des Servers gesetzt.
    Server,
    /// Systemzeitzone des Nodes (Wings) – das, was der Server ohne eigene Einstellung nutzt.
    Node,
    /// Standard-Zeitzone aus den Backup-Plus-Einstellungen des Admins.
    Extension,
    /// Nichts davon verfügbar: UTC.
    Fallback,
}

/// Wählt die erste gültige Zeitzone aus Server → Node → Extension-Standard → UTC.
pub fn pick_timezone(
    server: Option<&str>,
    node: Option<&str>,
    extension_default: &str,
) -> (chrono_tz::Tz, TimezoneSource) {
    let parse = |tz: Option<&str>| tz.map(str::trim).filter(|tz| !tz.is_empty())?.parse().ok();

    if let Some(tz) = parse(server) {
        (tz, TimezoneSource::Server)
    } else if let Some(tz) = parse(node) {
        (tz, TimezoneSource::Node)
    } else if let Some(tz) = parse(Some(extension_default)) {
        (tz, TimezoneSource::Extension)
    } else {
        (chrono_tz::UTC, TimezoneSource::Fallback)
    }
}

/// Ermittelt die Zeitzone eines Servers. Die Node-Konfiguration wird nur gefragt, wenn der
/// Server selbst keine Zeitzone hat (das Panel hält sie 120 s im Cache).
pub async fn resolve_timezone(
    state: &shared::State,
    server: &shared::models::server::Server,
    extension_default: &str,
) -> (chrono_tz::Tz, TimezoneSource) {
    let server_tz = server.timezone.as_deref();
    if server_tz.is_some_and(|tz| tz.parse::<chrono_tz::Tz>().is_ok()) {
        return pick_timezone(server_tz, None, extension_default);
    }

    let node_tz = match server.node.fetch_cached(&state.database).await {
        Ok(node) => node.fetch_default_timezone(&state.database).await,
        Err(_) => None,
    };

    pick_timezone(server_tz, node_tz.as_deref(), extension_default)
}

/// Nächster Termin strikt nach `after` (UTC), ausgewertet in `timezone`. Ergebnis in UTC.
pub fn next_after(
    cron: &croner::Cron,
    timezone: chrono_tz::Tz,
    after: chrono::NaiveDateTime,
) -> Option<chrono::NaiveDateTime> {
    let local = after.and_utc().with_timezone(&timezone);

    cron.find_next_occurrence(&local, false)
        .ok()
        .map(|next| next.naive_utc())
}

/// Kleinster Abstand zwischen zwei aufeinanderfolgenden Terminen in den nächsten Läufen.
pub fn min_gap(
    cron: &croner::Cron,
    timezone: chrono_tz::Tz,
    from: chrono::NaiveDateTime,
) -> Option<chrono::Duration> {
    let mut previous = next_after(cron, timezone, from)?;
    let mut gap: Option<chrono::Duration> = None;

    for _ in 0..48 {
        let Some(next) = next_after(cron, timezone, previous) else {
            break;
        };
        let current = next - previous;
        gap = Some(gap.map_or(current, |g| g.min(current)));
        previous = next;
    }

    gap
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn hourly_next_and_gap() {
        let cron = parse("0 0 * * * *").unwrap();
        let next = next_after(&cron, chrono_tz::UTC, at(2026, 11, 24, 12, 30)).unwrap();
        assert_eq!(next, at(2026, 11, 24, 13, 0));
        assert_eq!(min_gap(&cron, chrono_tz::UTC, at(2026, 11, 24, 12, 30)), Some(chrono::Duration::hours(1)));
    }

    #[test]
    fn weekly_monday_in_timezone() {
        // Montag 00:00 Berlin = Sonntag 23:00 UTC (Winterzeit)
        let cron = parse("0 0 0 * * 1").unwrap();
        let next = next_after(&cron, chrono_tz::Europe::Berlin, at(2026, 11, 24, 12, 0)).unwrap();
        assert_eq!(next, at(2026, 11, 29, 23, 0));
    }

    #[test]
    fn five_field_is_accepted() {
        assert!(parse("30 3 * * *").is_ok());
        assert!(parse("kein cron").is_err());
    }
}

#[cfg(test)]
mod timezone_tests {
    use super::{TimezoneSource, pick_timezone};

    #[test]
    fn server_then_node_then_extension_then_utc() {
        assert_eq!(
            pick_timezone(Some("Europe/Berlin"), Some("Asia/Tokyo"), "America/New_York"),
            (chrono_tz::Europe::Berlin, TimezoneSource::Server)
        );
        assert_eq!(
            pick_timezone(None, Some("Asia/Tokyo"), "America/New_York"),
            (chrono_tz::Asia::Tokyo, TimezoneSource::Node)
        );
        assert_eq!(
            pick_timezone(Some("kaputt"), Some(""), "America/New_York"),
            (chrono_tz::America::New_York, TimezoneSource::Extension)
        );
        assert_eq!(pick_timezone(None, None, ""), (chrono_tz::UTC, TimezoneSource::Fallback));
    }
}
