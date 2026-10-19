/**
 * "Ins Web hochladen" (on-prem → web, contract A3) — pure state machine
 * of the upload dialog plus the async driver (dependencies injected, so
 * it is testable without Tauri).
 *
 *   idle ─open─▶ confirm ─start─▶ uploading ─succeeded─▶ done
 *                  ▲  │                    └─failed──▶ error ─retry─▶ uploading
 *                  │  └cancel─▶ idle                     └close──▶ idle
 *
 * "Danach auf Live umschalten" (goLive) switches the desktop override to
 * live after a successful upload; the Rust side then sets the web to live
 * (A2) before the next result POST.
 */
import type { DisciplinePayload, UploadOutcome } from "./cloudTypes";

export type UploadState =
  | { step: "idle" }
  | { step: "confirm"; goLive: boolean }
  | { step: "uploading"; goLive: boolean }
  | { step: "done"; outcome: UploadOutcome; wentLive: boolean }
  | { step: "error"; message: string; goLive: boolean };

export type UploadAction =
  | { type: "open"; goLive?: boolean }
  | { type: "toggleGoLive" }
  | { type: "cancel" }
  | { type: "start" }
  | { type: "succeeded"; outcome: UploadOutcome }
  | { type: "failed"; message: string }
  | { type: "retry" }
  | { type: "close" };

export const UPLOAD_IDLE: UploadState = { step: "idle" };

export function uploadReducer(state: UploadState, action: UploadAction): UploadState {
  switch (action.type) {
    case "open":
      return state.step === "idle" || state.step === "done" || state.step === "error"
        ? { step: "confirm", goLive: action.goLive ?? false }
        : state;
    case "toggleGoLive":
      return state.step === "confirm" ? { ...state, goLive: !state.goLive } : state;
    case "cancel":
      return state.step === "confirm" ? UPLOAD_IDLE : state;
    case "start":
      return state.step === "confirm" ? { step: "uploading", goLive: state.goLive } : state;
    case "succeeded":
      return state.step === "uploading"
        ? { step: "done", outcome: action.outcome, wentLive: state.goLive }
        : state;
    case "failed":
      return state.step === "uploading"
        ? { step: "error", message: action.message, goLive: state.goLive }
        : state;
    case "retry":
      return state.step === "error" ? { step: "uploading", goLive: state.goLive } : state;
    case "close":
      return state.step === "uploading" ? state : UPLOAD_IDLE;
  }
}

export function isUploadBusy(state: UploadState): boolean {
  return state.step === "uploading";
}

/** Preconditions of the upload button. */
export function uploadAvailability(input: {
  paired: boolean;
  snapshot: DisciplinePayload | null;
}): { ok: true } | { ok: false; reason: string } {
  if (!input.snapshot) return { ok: false, reason: "Kein Wettkampf geladen." };
  if (!input.paired) {
    return {
      ok: false,
      reason: "Nicht mit dem Web-Konto gekoppelt — bitte koppeln oder KiDo-Datei exportieren.",
    };
  }
  return { ok: true };
}

/** Operator-friendly German message for a Rust / HTTP error string. */
export function friendlyUploadError(raw: string): string {
  const msg = raw.replace(/^Error:\s*/, "");
  if (/^unauthorized/i.test(msg)) {
    return "API-Schlüssel ungültig — bitte die Desktop-App neu mit dem Web koppeln.";
  }
  if (/^not_found/i.test(msg)) {
    return "Die Disziplin wurde im Web nicht gefunden (gelöscht?). Bitte KiDo-Datei exportieren.";
  }
  if (/^forbidden/i.test(msg)) {
    return "Keine Berechtigung für diese Disziplin im Web.";
  }
  if (/request failed|upload failed|error sending request|dns|connect/i.test(msg)) {
    return "Keine Verbindung zum Web — bitte Internet prüfen und erneut versuchen (die Daten bleiben lokal gesichert).";
  }
  if (/^http 400|validation/i.test(msg)) {
    return `Das Web hat die Daten abgelehnt: ${msg}`;
  }
  return `Hochladen fehlgeschlagen: ${msg}`;
}

/** `"3 Läufe und 12 Fehler hochgeladen."` */
export function uploadSummaryText(o: UploadOutcome): string {
  const runs = o.runsUpdated === 1 ? "1 Lauf" : `${o.runsUpdated} Läufe`;
  const pens = o.penaltiesUpserted === 1 ? "1 Fehler" : `${o.penaltiesUpserted} Fehler`;
  return `${runs} und ${pens} hochgeladen.`;
}

export interface UploadDeps {
  upload: () => Promise<UploadOutcome>;
  /** Switch the desktop override to live (Rust then calls A2). */
  goLive: () => Promise<void>;
}

/**
 * Run an upload from the `uploading` state; returns the next state.
 * A failure of the live switch after a successful upload is not an upload
 * error (data is on the web) — it is reported in the `done` state.
 */
export async function performUpload(
  state: UploadState,
  deps: UploadDeps,
): Promise<UploadState> {
  if (state.step !== "uploading") return state;
  let outcome: UploadOutcome;
  try {
    outcome = await deps.upload();
  } catch (e) {
    return uploadReducer(state, { type: "failed", message: friendlyUploadError(String(e)) });
  }
  const done = uploadReducer(state, { type: "succeeded", outcome });
  if (state.goLive) {
    try {
      await deps.goLive();
    } catch {
      return done.step === "done" ? { ...done, wentLive: false } : done;
    }
  }
  return done;
}
