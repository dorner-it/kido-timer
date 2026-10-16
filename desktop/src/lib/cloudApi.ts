import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { IS_TAURI } from "./tauri";
import type {
  CloudEvent,
  CloudIdentity,
  DisciplineListItem,
  DisciplinePayload,
  EventSummary,
  FailedPost,
  OpenKidoResult,
  RunStatus,
} from "./cloudTypes";

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

export async function cloudSelectDiscipline(
  id: string,
): Promise<DisciplinePayload> {
  if (!IS_TAURI) return requireDesktop("Wettkampfauswahl");
  return invoke<DisciplinePayload>("cloud_select_discipline", { id });
}

export async function cloudDeselect(): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("cloud_deselect");
}

export async function cloudSnapshot(): Promise<DisciplinePayload | null> {
  if (!IS_TAURI) return null;
  return invoke<DisciplinePayload | null>("cloud_snapshot");
}

export interface PostRunArgs {
  disciplineId: string;
  runId: string;
  runNumber: number;
  status: RunStatus;
  originalTimeMs?: number | null;
  startedAt?: string | null;
  endedAt?: string | null;
}

export async function cloudPostRunStatus(args: PostRunArgs): Promise<boolean> {
  if (!IS_TAURI) return false;
  return invoke<boolean>("cloud_post_run_status", {
    disciplineId: args.disciplineId,
    runId: args.runId,
    runNumber: args.runNumber,
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
