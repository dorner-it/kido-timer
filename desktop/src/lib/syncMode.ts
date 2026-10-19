/**
 * Live vs. on-prem per discipline. The web sets the default
 * (`discipline.sync_mode`); the desktop may override it on the day
 * (e.g. no internet at the venue). Overrides are stored locally per
 * discipline id (localStorage).
 */
import type { DisciplinePayload, SyncMode } from "./cloudTypes";

export type SyncOverrides = Record<string, SyncMode>;

const STORAGE_KEY = "kido-timer:sync-overrides:v1";

/** Effective mode: override if set, else the web default. */
export function effectiveSyncMode(
  webDefault: SyncMode,
  override?: SyncMode | null,
): SyncMode {
  return override ?? webDefault;
}

/** Effective sync mode of a loaded discipline (`null` = none loaded). */
export function snapshotSyncMode(
  snapshot: DisciplinePayload | null,
  overrides: SyncOverrides,
): SyncMode | null {
  if (!snapshot) return null;
  return effectiveSyncMode(
    snapshot.discipline.sync_mode,
    overrides[snapshot.discipline.id],
  );
}

export function isOverridden(
  webDefault: SyncMode,
  override?: SyncMode | null,
): boolean {
  return override != null && override !== webDefault;
}

/** UI label: "Live" / "On-prem". */
export function syncModeLabel(mode: SyncMode): string {
  return mode === "live" ? "Live" : "On-prem";
}

/**
 * Set or clear the override of one discipline. Choosing the web default
 * removes the override (so later web changes apply again).
 */
export function withOverride(
  overrides: SyncOverrides,
  disciplineId: string,
  mode: SyncMode | null,
  webDefault: SyncMode,
): SyncOverrides {
  const next = { ...overrides };
  if (mode == null || mode === webDefault) delete next[disciplineId];
  else next[disciplineId] = mode;
  return next;
}

export function sanitizeOverrides(raw: unknown): SyncOverrides {
  const out: SyncOverrides = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (v === "live" || v === "offline") out[k] = v;
  }
  return out;
}

export function loadSyncOverrides(): SyncOverrides {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw ? sanitizeOverrides(JSON.parse(raw)) : {};
  } catch {
    return {};
  }
}

export function saveSyncOverrides(overrides: SyncOverrides): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
  } catch {
    // ignore quota / disabled storage
  }
}
