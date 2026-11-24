# Backup Plus – Calagopus Extension

Automatische Server-Backups mit GFS-Rotation („Grandfather–Father–Son“) und automatischer Bereinigung.

Paketname: `dev.backupplus.gfsbackups` · benötigt Calagopus Panel **>= 1.2.11** (heavy-Image oder Dev-Umgebung)

## Was sie macht

Standard-Richtlinie pro Server (frei anpassbar):

| Stufe        | Anzahl | Bedeutung                                   |
| ------------ | ------ | ------------------------------------------- |
| Stündlich    | 3      | die Backups der letzten drei Stunden        |
| Wöchentlich  | 1      | höchstens ~7 Tage alt, wird wöchentlich erneuert |
| Monatlich    | 1      | höchstens ~30 Tage alt, wird monatlich erneuert  |

Ergebnis: **immer genau 5 Backups**, alle älteren werden automatisch gelöscht.

So funktioniert die Rotation:

1. Jede Stunde (Intervall und Minute einstellbar) entsteht ein neues Backup in der Stufe *stündlich*.
2. Fällt ein Backup aus den 3 neuesten stündlichen heraus, wird geprüft, ob eine höhere Stufe fällig ist
   (monatlich → wöchentlich → täglich). Wenn ja, wird es **befördert** statt gelöscht – es geht also kein
   Backup verloren, das gebraucht wird.
3. Jede Stufe wird auf ihre Höchstanzahl gekürzt; das jeweils älteste fliegt raus.

> Tipp: Mit „Wöchentlich behalten = 2“ hast du immer mindestens ein Backup, das älter als eine Woche ist.

## Zusätzliche Features

- **Server-Tab „Auto-Backups“** mit Status, nächstem/letztem Lauf, Speicherbelegung und Backup-Limit-Anzeige
- **Vorschau**: Für jedes Backup wird angezeigt, was beim nächsten Lauf passiert (bleibt / wird befördert / wird gelöscht – inkl. Grund)
- **Jetzt sichern**-Button (Rate-Limit: 3 pro Minute)
- **Anpinnen**: angepinnte Backups (= im Panel gesperrt) werden nie automatisch gelöscht
- **Stufe manuell ändern** (z. B. ein bestimmtes Backup als monatlich markieren)
- **Tägliche Stufe** und beliebige Anzahlen pro Stufe
- **Namensvorlage** mit Platzhaltern `{date}`, `{day}`, `{time}`, `{tier}`, `{server}`
- **Ignorierte Dateien** pro Server (wie `.gitignore`)
- **Fehlgeschlagene Backups** automatisch aufräumen (abschaltbar)
- **Protokoll** pro Server + Einträge im normalen Aktivitätslog des Panels
- **Schutzmechanismen**: überspringt gesperrte, installierende, wiederherstellende und übertragene Server; startet kein neues Backup, solange das vorherige noch läuft; nutzt dieselbe Sperre wie das Panel
- **Eigene Backup-Gruppe** „Backup Plus (automatisch)“ – deine manuellen Backups werden **nie** angefasst
- **Admin-Seite** (Extensions → Backup Plus → Konfigurieren): globaler Hauptschalter, minimales Intervall,
  max. Backups pro Server, Lastverteilung (max. neue Backups pro Minute), Protokoll-Länge, Statistik
- **Berechtigungen** für Subuser: `backup-plus.read`, `backup-plus.manage`

## Wichtig: Backup-Limit

Das Panel zählt alle Backups eines Servers gegen dessen **Backup-Limit**. Setze es für jeden Server auf mindestens
die Summe der behaltenen Backups – bei der Standard-Richtlinie **5, empfohlen 6** (beim Erstellen existiert das
neue Backup kurz zusätzlich, bis die Bereinigung ein paar Minuten später läuft). Ist das Limit genau 5, macht die
Extension vor dem Erstellen selbst einen Platz frei. Manuelle Backups zählen ebenfalls mit.

## Installation

**Docker (heavy-Image):** Die Datei `dev_backupplus_gfsbackups.c7s.zip` unter [Releases](https://github.com/Luis-1805/calagopus-backup-plus/releases/latest) herunterladen und im Admin-Bereich unter *Extensions*
hochladen – das Panel kompiliert und lädt sie (1–2 Minuten). Alternativ die Datei nach `./build/extensions/`
kopieren und `docker compose restart web` ausführen.

**Dev-Umgebung:**

```bash
panel-rs extensions add dev_backupplus_gfsbackups.c7s.zip
panel-rs extensions apply --profile balanced
```

Danach am Server den Tab **Auto-Backups** öffnen, Richtlinie prüfen, „Automatische Backups aktivieren“ einschalten
und speichern. Das erste Backup entsteht innerhalb einer Minute.

## Technisches

- Backend: Rust, Cron-Task jede Minute (`backup_plus_tick`, nur auf der Primär-Instanz)
- Nutzt die Panel-internen Modelle (`ServerBackup::create`, `delete`, `ServerBackupGroup`) – funktioniert also mit
  jedem Backup-Speicher (Local, S3, Restic, Kopia, …)
- Eigene Tabellen: `backup_plus_policies`, `backup_plus_backups`, `backup_plus_log`
- Zeiten im Backend in UTC; die „Minute“ bezieht sich auf die volle UTC-Stunde
