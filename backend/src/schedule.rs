//! Cron-Zeitpläne (mit Sekundenfeld, wie im Panel üblich) in der Zeitzone des Servers.

use std::str::FromStr;

/// Parst einen Cron-Ausdruck mit 5 oder 6 Feldern.
pub fn parse(expression: &str) -> Result<croner::Cron, String> {
    croner::Cron::from_str(expression.trim())
        .map_err(|err| err.to_string())
}

/// Zeitzone eines Servers (fällt wie das Panel auf UTC zurück).
pub fn server_timezone(timezone: Option<&str>) -> chrono_tz::Tz {
    timezone
        .and_then(|tz| tz.parse().ok())
        .unwrap_or(chrono_tz::UTC)
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
