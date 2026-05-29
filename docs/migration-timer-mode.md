# Migration: `TimerMode` v1/v2 → v3

Schema-Bump **2 → 3** (siehe `desktop-integration.md` §2.2). Die alten
generischen `TimerMode`-Werte werden durch zwei disziplin-spezifische
Varianten ersetzt:

| Alt (v1/v2)         | Neu (v3)           | Bahnen | Disziplin                    |
|---------------------|--------------------|--------|------------------------------|
| `single_lane`       | `gruppenstaffette` | 2      | Staffette-Variante           |
| `two_lane_parallel` | `gruppenstaffette` | 2      | Staffette-Variante           |
| `relay`             | `gruppenstaffette` | 2      | Staffellauf = Staffette      |
| `individual`        | `loeschangriff`    | 4      | 4-Bahn-Einzelwertung ≙ LA    |

Begründung: 1-/2-Bahn-Formate liefen alle als Gruppenstaffette; `relay`
ist explizit Staffellauf; `individual` war in der Praxis die 4-Bahn-
Einzelwertung des Löschangriff-Formats.

## Reihenfolge im Release

1. Backend mit v3 lokal bauen (`cargo build`) und Frontend (`npm run build`).
2. Migrations-Tool **gegen die Ziel-Tabelle** laufen lassen (Dry-Run zuerst).
3. Erst danach Lambda und Frontend deployen.
4. Desktop-Client v3 erst nach erfolgreichem Web-Deploy verteilen (alte
   Desktops senden v1-Strings und bekommen `400 validation`).

## Lauf-Befehl

Voraussetzungen: AWS-Profil `aws` mit DDB-Update-Rechten auf der Tabelle.

```sh
# Vorschau ohne Schreibzugriffe
AWS_PROFILE=aws cargo run --bin migrate_timer_mode -- \
    --table ki-do-timer-prod --dry-run

# Schreibend
AWS_PROFILE=aws cargo run --bin migrate_timer_mode -- \
    --table ki-do-timer-prod
```

Für die dev-Tabelle entsprechend `--table ki-do-timer-dev`.

## Eigenschaften

- **Idempotent**: Items, die bereits `gruppenstaffette`/`loeschangriff`
  enthalten, werden übersprungen. Re-runs sind sicher.
- **Unbekannte Werte** (weder v1 noch v3) werden mit Warn-Log gemeldet und
  *nicht* angefasst. Manuell prüfen und ggf. nachtragen.
- **Scan statt Query**: Bewusst — der Datensatz ist klein und das Tool
  läuft ad-hoc von der Dev-Maschine, nicht im Request-Pfad.
- **Kein Backfill anderer Felder**: Nur das `mode`-Attribut wird
  umgeschrieben. `lane`-Werte auf `TeamEntry`/`Run` bleiben wie sie sind
  (die UI zeigt sie passend zur neuen Bahnenzahl an).

## Rollback

Es gibt keinen automatischen Rückweg, weil die Quell-Information
verloren geht (drei v1-Werte fallen auf `gruppenstaffette` zusammen).
Falls ein Rollback nötig wird: aus dem letzten DDB-Backup vor der
Migration wiederherstellen.
