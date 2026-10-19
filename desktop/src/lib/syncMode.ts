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

export interface SyncSwitchPlan {
  /** Override to store (`null` = follow the web default). */
  override: SyncMode | null;
  /** Rust will switch the web to live first (A2). */
  switchesWebToLive: boolean;
  /** The on-prem judge server will start. */
  startsLocalServer: boolean;
  /** Hints shown before / while switching. */
  notes: string[];
}

/**
 * What happens when the operator picks `target` (`null` = back to the web
 * default) in the Live / On-prem switch.
 */
export function planSyncSwitch(input: {
  webDefault: SyncMode;
  target: SyncMode | null;
  paired: boolean;
  /** Local changes not on the web yet (timing / errors / lineups …). */
  hasUnsynced: boolean;
}): SyncSwitchPlan {
  const effective = input.target ?? input.webDefault;
  const override =
    input.target == null || input.target === input.webDefault ? null : input.target;
  const notes: string[] = [];
  if (effective === "live") {
    if (!input.paired) {
      notes.push("Nicht gekoppelt — Live braucht die Verbindung zum Web-Konto.");
    }
    if (input.hasUnsynced) {
      notes.push(
        "Lokale Änderungen sind noch nicht im Web — beim Wechsel wird gefragt: hochladen oder verwerfen.",
      );
    }
    if (input.webDefault === "offline" && input.paired) {
      notes.push("Das Web wird für diese Disziplin auf Live umgestellt.");
    }
  } else {
    notes.push(
      "Der Laptop startet einen lokalen Server für die Strafrichter-Handys (QR-Code im On-prem-Bildschirm).",
    );
  }
  return {
    override,
    switchesWebToLive: effective === "live" && input.webDefault === "offline" && input.paired,
    startsLocalServer: effective === "offline",
    notes,
  };
}
