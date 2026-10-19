/**
 * Wire types of the desktop ⇄ web contract, **schema v4**
 * (Gruppenstafette 2027, `docs/contract-pr1-2027.md`). Mirrors
 * `src-tauri/src/cloud/types.rs`; enum values come from `kido-core`.
 * The Rust side applies the read-time migration (defaults), so every
 * field below is present when it reaches the UI.
 */

/** Disziplin. Legacy-Werte (`single_lane` …) werden abgelehnt. */
export type TimerMode = "gruppenstaffette" | "loeschangriff";

/** `offline` = On-prem (Desktop ist Master, keine Cloud-Übertragung). */
export type SyncMode = "live" | "offline";
export type JudgeMode = "per_station" | "per_station_and_lane";
export type RunStatus = "pending" | "active" | "completed" | "cancelled";
export type PenaltyStatus = "pending" | "approved" | "rejected";
/** SZ = +15 s, FV = Lauf ungültig. */
export type PenaltyKind = "sz" | "fv";
export type TimeSource = "lichtschranke" | "handstopp";
export type LineupConfirmedBy = "startlinie" | "organizer";

export type Gender = "male" | "female";

/** Vom Verein angekreuzt (nie abgeleitet). */
export type Wertungsgruppe = "wg1" | "wg2" | "wg3" | "wg4" | "wg5";

/** Kanonische Stations-Schlüssel (Ausgabe des Backends). */
export type StationKind =
  | "startlinie"
  | "verteilerlinie"
  | "strahlrohrlinie"
  | "knotengestell"
  | "angriffslinie"
  | "podest";

export interface CloudIdentity {
  baseUrl: string;
  sub: string;
  email: string;
  displayName: string;
  keyId: string;
}

export interface EventSummary {
  id: string;
  name: string;
  date: string;
  location: string | null;
  is_active: boolean;
  current_discipline_id: string | null;
  current_run_id: string | null;
  current_run_ids: string[];
}

export interface DisciplineListItem {
  id: string;
  event_id: string;
  event_name: string;
  event_date: string;
  name: string;
  mode: TimerMode;
  lane_count: number;
  judge_mode: JudgeMode;
  /** Standard aus dem Web; der Desktop kann ihn überschreiben. */
  sync_mode: SyncMode;
  /** True iff this discipline is the one selected on the active event's
   *  lane. The desktop usually pairs to this one. */
  is_current: boolean;
}

export interface ExportedEvent {
  id: string;
  owner_sub: string;
  name: string;
  date: string;
  location: string | null;
  is_active: boolean;
  current_discipline_id: string | null;
  /** Legacy-Spiegel von `current_run_ids[0]`. */
  current_run_id: string | null;
  /** Aktuelle Läufe — einer pro aktiver Bahn. */
  current_run_ids: string[];
  created_at?: string | null;
  updated_at?: string | null;
}

export interface Discipline {
  id: string;
  event_id: string;
  name: string;
  mode: TimerMode;
  /** Gruppenstafette 1 | 2, Löschangriff 4. */
  lane_count: number;
  judge_mode: JudgeMode;
  sync_mode: SyncMode;
  position: number;
  created_at?: string | null;
  updated_at?: string | null;
}

export interface Team {
  id: string;
  event_id: string;
  name: string;
  position: number;
}

export interface WgHistoryEntry {
  from: Wertungsgruppe | null;
  to: Wertungsgruppe;
  by: string;
  at: string;
  note: string | null;
}

export interface TeamEntry {
  team_id: string;
  discipline_id: string;
  event_id: string;
  start_number: number;
  lane: number | null;
  /** `null` = „WG fehlt“ (Altbestand) — nicht in der Wertung. */
  wg: Wertungsgruppe | null;
  wg_history: WgHistoryEntry[];
}

export interface Runner {
  id: string;
  team_id: string;
  event_id: string;
  first_name: string;
  last_name: string;
  birth_date: string;
  position: number;
  gender?: Gender | null;
}

export interface Run {
  id: string;
  event_id: string;
  discipline_id: string;
  team_id: string;
  runner_id: string | null;
  run_number: number;
  lane: number | null;
  status: RunStatus;
  /** Lichtschranke oder Mittel der Handzeiten — Wertungsgrundlage. */
  original_time_ms: number | null;
  /** Legacy, nur Anzeige — wird nie zur Wertung addiert. */
  correction_ms: number;
  started_at: string | null;
  ended_at: string | null;
  lineup: string[];
  lineup_confirmed_at: string | null;
  lineup_confirmed_by: LineupConfirmedBy | null;
  time_source: TimeSource;
  hand_time_1_ms: number | null;
  hand_time_2_ms: number | null;
  /** Letztes „Übernehmen“ (RFC 3339). Entscheidungen bis dahin sind
   *  eingefroren; spätere lassen sich noch rückgängig machen. Fehlt bei
   *  älteren Snapshots. */
  penalties_accepted_at?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
}

export interface Penalty {
  id: string;
  event_id: string;
  discipline_id: string;
  run_id: string;
  team_id: string;
  runner_id: string | null;
  kind: PenaltyKind;
  /** Katalog-Code, `null` bei Altbestand. */
  code: string | null;
  station: StationKind | null;
  is_disqualification: boolean;
  decider_slot: number;
  delta_ms: number;
  reason_code: string | null;
  status: PenaltyStatus;
  created_at: string | null;
  reviewed_at: string | null;
  reviewed_by: string | null;
}

export interface DisciplinePayload {
  schema_version: number;
  exported_at: string;
  owner_sub: string;
  event: ExportedEvent;
  discipline: Discipline;
  teams: Team[];
  runners: Runner[];
  team_entries: TeamEntry[];
  runs: Run[];
  /** Nur bestätigte Fehler (Altleser). */
  approved_penalties: Penalty[];
  /** Alle Fehler inkl. offen / zurückgenommen (PR 6, A1). Fehlt das
   *  Feld (älteres Web), füllt Rust es aus `approved_penalties`. */
  penalties: Penalty[];
}

export interface FailedPost {
  runId: string;
  disciplineId: string;
  runNumber: number;
  status: RunStatus;
  originalTimeMs: number | null;
  startedAt: string | null;
  endedAt: string | null;
  error: string;
  failedAt: string;
}

/** Andere lokale On-prem-Änderungen, die noch nicht im Web sind. */
export interface UnsyncedSummary {
  penalties: number;
  lineups: number;
  handTimes: number;
}

/** Abweichungen beim Wechsel zurück auf Live. */
export interface DivergenceState {
  divergences: RunDivergence[];
  unsynced: UnsyncedSummary;
}

/** Ergebnis von „Ins Web hochladen“ (A3). */
export interface UploadOutcome {
  runsUpdated: number;
  penaltiesUpserted: number;
  runsSent: number;
  penaltiesSent: number;
}

export interface OnPremClient {
  ip: string;
  lastSeenSecs: number;
  requests: number;
}

export interface JudgeLink {
  address: string;
  url: string;
  qrSvg: string;
}

/** Status des lokalen Strafrichter-Servers. */
export interface OnPremStatus {
  running: boolean;
  port: number | null;
  links: JudgeLink[];
  token: string | null;
  ownerSub: string | null;
  uptimeSecs: number | null;
  clients: OnPremClient[];
  error: string | null;
  judgeUiBuilt: boolean;
  /** Vorschau „Ins Web hochladen“: geänderte Läufe / Fehler. */
  uploadRuns: number;
  uploadPenalties: number;
  lastUploadAt: string | null;
}

/** Lokal erfasstes Ergebnis, das vom Web-Stand abweicht. */
export interface RunDivergence {
  runId: string;
  runNumber: number;
  teamId: string | null;
  localStatus: RunStatus;
  localTimeMs: number | null;
  remoteStatus: RunStatus | null;
  remoteTimeMs: number | null;
}

export interface KidoConflict {
  currentDisciplineId: string;
  currentDisciplineName: string;
  newDisciplineId: string;
  newDisciplineName: string;
}

export interface OpenKidoResult {
  adopted: boolean;
  payload: DisciplinePayload;
  conflict?: KidoConflict | null;
}

export interface RecordOutcome {
  runId: string | null;
  posted: boolean;
}

/** Fehlerkatalog-Eintrag (`kido_core::CATALOGUE`). */
export interface CatalogueItem {
  code: string;
  station: StationKind;
  station_label: string;
  kind: PenaltyKind;
  label_de: string;
}

export type CloudEvent =
  | { type: "paired"; identity: CloudIdentity }
  | { type: "cleared" }
  | { type: "snapshotChanged"; snapshot: DisciplinePayload }
  | { type: "snapshotError"; message: string }
  | { type: "resultPosted"; run_id: string; status: RunStatus }
  | { type: "resultPostFailed"; run_id: string; message: string }
  | { type: "failedPostsChanged"; failures: FailedPost[] }
  | { type: "divergenceChanged"; divergences: RunDivergence[]; unsynced: UnsyncedSummary }
  | { type: "onPremChanged"; status: OnPremStatus }
  | { type: "onPremClientSeen"; ip: string };
