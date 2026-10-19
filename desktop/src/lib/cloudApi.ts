import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { IS_TAURI } from "./tauri";
import type {
  CatalogueItem,
  CloudEvent,
  CloudIdentity,
  DisciplineListItem,
  DisciplinePayload,
  DivergenceState,
  EventSummary,
  FailedPost,
  OnPremStatus,
  OpenKidoResult,
  Penalty,
  RecordOutcome,
  RunStatus,
  SyncMode,
  UploadOutcome,
} from "./cloudTypes";
import type { ResultsResponse } from "./results";

export const CLOUD_EVENT = "kido://cloud-event";

export const DEFAULT_BASE_URL =
  "https://idr9teznt4.execute-api.eu-central-1.amazonaws.com";

function requireDesktop<T>(name: string): T {
  throw new Error(`${name} requires the desktop app`);
}

export async function cloudIdentity(): Promise<CloudIdentity | null> {
  if (!IS_TAURI) return null;
  return invoke<CloudIdentity | null>("cloud_identity");
}

export async function cloudPair(
  baseUrl: string,
  apiKey: string,
): Promise<CloudIdentity> {
  if (!IS_TAURI) return requireDesktop("Pairing");
  return invoke<CloudIdentity>("cloud_pair", { baseUrl, apiKey });
}

export async function cloudClear(): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_clear");
}

export async function cloudListEvents(): Promise<EventSummary[]> {
  if (!IS_TAURI) return [];
  return invoke<EventSummary[]>("cloud_list_events");
}

export async function cloudListDisciplines(): Promise<DisciplineListItem[]> {
  if (!IS_TAURI) return [];
  return invoke<DisciplineListItem[]>("cloud_list_disciplines");
}

/** Load a discipline. `mode` = effective sync mode (web default +
 *  desktop override); local results recorded earlier are kept. */
export async function cloudSelectDiscipline(
  id: string,
  mode: SyncMode | null,
): Promise<DisciplinePayload> {
  if (!IS_TAURI) return requireDesktop("Wettkampfauswahl");
  return invoke<DisciplinePayload>("cloud_select_discipline", { id, mode });
}

/** Apply the effective sync mode to the loaded discipline. */
export async function cloudSetSyncMode(mode: SyncMode): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_set_sync_mode", { mode });
}

export const EMPTY_DIVERGENCE: DivergenceState = {
  divergences: [],
  unsynced: { penalties: 0, lineups: 0, handTimes: 0 },
};

export async function cloudDivergences(): Promise<DivergenceState> {
  if (!IS_TAURI) return EMPTY_DIVERGENCE;
  return invoke<DivergenceState>("cloud_divergences");
}

/** `upload = true`: alle lokalen Änderungen hochladen (A3), sonst verwerfen. */
export async function cloudResolveDivergence(upload: boolean): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_resolve_divergence", { upload });
}

export async function cloudTakeStartupWarning(): Promise<string | null> {
  if (!IS_TAURI) return null;
  return invoke<string | null>("cloud_take_startup_warning");
}

export async function computeResults(): Promise<ResultsResponse> {
  if (!IS_TAURI) return requireDesktop("Ergebnisse");
  return invoke<ResultsResponse>("compute_results");
}

export async function kidoCatalogue(): Promise<CatalogueItem[]> {
  if (!IS_TAURI) return [];
  return invoke<CatalogueItem[]>("kido_catalogue");
}

export async function cloudDeselect(): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_deselect");
}

export async function cloudSnapshot(): Promise<DisciplinePayload | null> {
  if (!IS_TAURI) return null;
  return invoke<DisciplinePayload | null>("cloud_snapshot");
}

export interface RecordLaneArgs {
  /** Desktop lane 1-based (= timer channel). */
  lane: number;
  /** Operator overrides lane → run id. */
  overrides: Record<number, string>;
  status: RunStatus;
  originalTimeMs?: number | null;
  startedAt?: string | null;
  endedAt?: string | null;
}

/** Record a lane timing change: always stored locally (persisted),
 *  posted to the web when the effective mode is live. */
export async function cloudRecordLaneStatus(
  args: RecordLaneArgs,
): Promise<RecordOutcome> {
  if (!IS_TAURI) return { runId: null, posted: false };
  return invoke<RecordOutcome>("cloud_record_lane_status", {
    lane: args.lane,
    overrides: args.overrides,
    status: args.status,
    originalTimeMs: args.originalTimeMs ?? null,
    startedAt: args.startedAt ?? null,
    endedAt: args.endedAt ?? null,
  });
}

export async function cloudRetryPost(runId: string): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_retry_post", { runId });
}

export async function cloudClearFailedPost(runId: string): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_clear_failed_post", { runId });
}

export async function cloudFailedPosts(): Promise<FailedPost[]> {
  if (!IS_TAURI) return [];
  return invoke<FailedPost[]>("cloud_failed_posts");
}

export async function pickKidoFile(): Promise<string | null> {
  if (!IS_TAURI) return null;
  const path = await openDialog({
    multiple: false,
    directory: false,
    title: "KiDo-Datei öffnen",
    filters: [
      { name: "KiDo-Datei", extensions: ["kido", "json"] },
      { name: "Alle Dateien", extensions: ["*"] },
    ],
  });
  return typeof path === "string" ? path : null;
}

/** Verify a `.kido` file. Adopts the snapshot unless `force` is false and a
 *  different discipline is already loaded — in which case the result will
 *  carry a `conflict` and `adopted: false`, leaving the existing snapshot. */
export async function cloudOpenKido(
  path: string,
  force = false,
): Promise<OpenKidoResult> {
  if (!IS_TAURI) return requireDesktop("KiDo-Datei");
  return invoke<OpenKidoResult>("cloud_open_kido", { path, force });
}

export async function cloudExportKido(suggestedName: string): Promise<string | null> {
  if (!IS_TAURI) return null;
  const path = await saveDialog({
    defaultPath: suggestedName,
    filters: [{ name: "KiDo-Datei", extensions: ["kido"] }],
  });
  if (typeof path !== "string") return null;
  await invoke("cloud_export_kido", { path });
  return path;
}

export async function listenCloudEvents(
  handler: (event: CloudEvent) => void,
): Promise<UnlistenFn> {
  if (!IS_TAURI) return () => {};
  return listen<CloudEvent>(CLOUD_EVENT, (e) => handler(e.payload));
}

// ─── On-prem (lokaler Strafrichter-Server + Organisator) ─────────────────────

export const OFFLINE_STATUS: OnPremStatus = {
  running: false,
  port: null,
  links: [],
  token: null,
  ownerSub: null,
  uptimeSecs: null,
  clients: [],
  error: null,
  judgeUiBuilt: false,
  uploadRuns: 0,
  uploadPenalties: 0,
  lastUploadAt: null,
};

export async function onPremStatus(): Promise<OnPremStatus> {
  if (!IS_TAURI) return OFFLINE_STATUS;
  return invoke<OnPremStatus>("onprem_status");
}

/** „Neuer Zugangscode“ — alte Handys bekommen 401. */
export async function onPremRotateToken(): Promise<OnPremStatus> {
  if (!IS_TAURI) return requireDesktop("Zugangscode");
  return invoke<OnPremStatus>("onprem_rotate_token");
}

export async function onPremSetCurrentRuns(runIds: string[]): Promise<void> {
  if (!IS_TAURI) return requireDesktop("Aktuelle Läufe");
  await invoke("onprem_set_current_runs", { runIds });
}

export async function onPremReviewPenalty(penaltyId: string, approve: boolean): Promise<Penalty> {
  if (!IS_TAURI) return requireDesktop("Prüfung");
  return invoke<Penalty>("onprem_review_penalty", { penaltyId, approve });
}

/** „Rückgängig“: entschiedener Fehler → wieder offen (bis „Übernehmen“). */
export async function onPremUndoReview(penaltyId: string): Promise<Penalty> {
  if (!IS_TAURI) return requireDesktop("Rückgängig");
  return invoke<Penalty>("onprem_undo_review", { penaltyId });
}

/** Fehler, deren Entscheidung aus dem Web stammt (kein „Rückgängig“). */
export async function onPremWebDecidedPenalties(): Promise<string[]> {
  if (!IS_TAURI) return [];
  return invoke<string[]>("onprem_web_decided_penalties");
}

/** „Übernehmen“: offene Fehler des Laufs → bestätigt. */
export async function onPremTakeOver(runId: string): Promise<number> {
  if (!IS_TAURI) return requireDesktop("Übernehmen");
  return invoke<number>("onprem_take_over", { runId });
}

export async function onPremSetLineup(runId: string, runnerIds: string[]): Promise<void> {
  if (!IS_TAURI) return requireDesktop("Besetzung");
  await invoke("onprem_set_lineup", { runId, runnerIds });
}

export async function onPremSetHandTimes(
  runId: string,
  handTime1Ms: number,
  handTime2Ms: number,
): Promise<number> {
  if (!IS_TAURI) return requireDesktop("Handzeit");
  return invoke<number>("onprem_set_hand_times", { runId, handTime1Ms, handTime2Ms });
}

export async function onPremClearHandTimes(runId: string): Promise<void> {
  if (!IS_TAURI) return requireDesktop("Handzeit");
  await invoke("onprem_clear_hand_times", { runId });
}

/** Mittelwert der Handzeiten (kido-core, 1/100 s, halb aufgerundet). */
export async function handTimeMean(handTime1Ms: number, handTime2Ms: number): Promise<number> {
  if (!IS_TAURI) {
    // Browser preview only: same formula as kido_core::timing.
    return Math.floor((handTime1Ms + handTime2Ms + 10) / 20) * 10;
  }
  return invoke<number>("hand_time_mean", { handTime1Ms, handTime2Ms });
}

/** „Ins Web hochladen“ (A3). */
export async function cloudUploadOnPrem(): Promise<UploadOutcome> {
  if (!IS_TAURI) return requireDesktop("Upload");
  return invoke<UploadOutcome>("cloud_upload_onprem");
}
