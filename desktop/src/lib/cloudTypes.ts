/**
 * Disziplin-Bezeichner (Schema v3+). Während des Web-Schema-Rollouts kann
 * das Backend kurzzeitig noch alte v2-Werte liefern; der Helper
 * `modeLabelDE` in `i18n.ts` mappt beide.
 */
export type TimerMode =
  | "gruppenstaffette"
  | "loeschangriff"
  // Legacy v2-Werte — werden vom alten Backend weiter geliefert, bis die
  // DDB-Migration durchläuft.
  | "single_lane"
  | "two_lane_parallel"
  | "relay"
  | "individual";

export type SyncMode = "live" | "offline";
export type RunStatus = "pending" | "active" | "completed" | "cancelled";
export type PenaltyStatus = "pending" | "approved" | "rejected";

export type Gender = "male" | "female";

/**
 * Feste Wertungsgruppen aus Alter + Geschlecht des Roster:
 * - WG1: Jungen + Mädchen 8-10 (gemischt)
 * - WG2: Jungen bis 14
 * - WG3: Mädchen bis 14
 * - WG4: Jungen bis 18
 * - WG5: Mädchen bis 18
 */
export type Wertungsgruppe = "wg1" | "wg2" | "wg3" | "wg4" | "wg5";

/** Stationen, an denen Strafrichter Strafen melden können. */
export type StationKind =
  | "start"
  | "verteiler"
  | "strahlrohr"
  | "knoten"
  | "kuebelspritze"
  | "podest"          // nur Löschangriff
  | "strahlrohrlinie"; // nur Löschangriff

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
}

export interface DisciplineListItem {
  id: string;
  event_id: string;
  event_name: string;
  event_date: string;
  name: string;
  mode: TimerMode;
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
  current_run_id: string | null;
  created_at?: string | null;
  updated_at?: string | null;
  /** Gemeinsamer Token für alle Strafrichter (Schema v3+). */
  station_token?: string | null;
}

export interface Discipline {
  id: string;
  event_id: string;
  name: string;
  mode: TimerMode;
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

export interface TeamEntry {
  team_id: string;
  discipline_id: string;
  event_id: string;
  start_number: number;
  lane: number | null;
  /** Aus Roster abgeleitete Wertungsgruppe; null bei Altbestand oder
   *  inkonsistentem Roster. */
  wg?: Wertungsgruppe | null;
}

export interface Runner {
  id: string;
  team_id: string;
  event_id: string;
  first_name: string;
  last_name: string;
  birth_date: string;
  position: number;
  /** Pflichtfeld ab Schema v3 — additiv eingeführt (null bei Altbestand). */
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
  original_time_ms: number | null;
  correction_ms: number;
  started_at: string | null;
  ended_at: string | null;
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
  decider_slot: number;
  delta_ms: number;
  reason_code: string | null;
  status: PenaltyStatus;
  created_at: string;
  reviewed_at: string | null;
  reviewed_by: string | null;
  /** Station, an der die Strafe gemeldet wurde (Schema v3+). */
  station?: StationKind | null;
  /** Bei Löschangriff: Disqualifikation statt Zeitstrafe. */
  is_disqualification?: boolean;
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
  approved_penalties: Penalty[];
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

export type CloudEvent =
  | { type: "paired"; identity: CloudIdentity }
  | { type: "cleared" }
  | { type: "snapshotChanged"; snapshot: DisciplinePayload }
  | { type: "snapshotError"; message: string }
  | { type: "resultPosted"; runId: string; status: RunStatus }
  | { type: "resultPostFailed"; runId: string; message: string }
  | { type: "failedPostsChanged"; failures: FailedPost[] };
