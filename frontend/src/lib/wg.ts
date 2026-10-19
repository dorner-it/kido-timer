// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Wertungsgruppen (WG1–WG5). The WG is ticked by the club on the
 * registration form — never derived. The backend only *warns* when a
 * runner's age (31.12.-age of the event year) doesn't fit the WG.
 */

import type { Runner, WgHistoryEntry, WgWarning, Wertungsgruppe } from './api';

export const WERTUNGSGRUPPEN: readonly Wertungsgruppe[] = ['wg1', 'wg2', 'wg3', 'wg4', 'wg5'];

export const WERTUNGSGRUPPE_LABELS: Record<Wertungsgruppe, string> = {
  wg1: 'WG1',
  wg2: 'WG2',
  wg3: 'WG3',
  wg4: 'WG4',
  wg5: 'WG5',
};

/** Long German labels (= `Wertungsgruppe::label_de`). */
export const WERTUNGSGRUPPE_DESCRIPTIONS: Record<Wertungsgruppe, string> = {
  wg1: 'Jungen + Mädchen 8–10',
  wg2: 'Jungen bis 14',
  wg3: 'Mädchen bis 14',
  wg4: 'Jungen bis 18',
  wg5: 'Mädchen bis 18',
};

export function isWertungsgruppe(value: unknown): value is Wertungsgruppe {
  return typeof value === 'string' && (WERTUNGSGRUPPEN as readonly string[]).includes(value);
}

/** `"wg2"` → `"WG2"`; `null`/unknown → fallback. */
export function wgLabel(wg: unknown, fallback = '—'): string {
  return isWertungsgruppe(wg) ? WERTUNGSGRUPPE_LABELS[wg] : fallback;
}

/** `"WG2 – Jungen bis 14"` for select options. */
export function wgOptionLabel(wg: Wertungsgruppe): string {
  return `${WERTUNGSGRUPPE_LABELS[wg]} – ${WERTUNGSGRUPPE_DESCRIPTIONS[wg]}`;
}

export const WG_MISSING_LABEL = 'WG fehlt';

type RunnerName = Pick<Runner, 'id' | 'first_name' | 'last_name'>;

export function runnerDisplayName(runner: RunnerName | undefined | null): string {
  if (!runner) return 'Unbekannter Läufer';
  const name = `${runner.first_name} ${runner.last_name}`.trim();
  return name || 'Unbekannter Läufer';
}

/**
 * One warning line, e.g. `"Max Muster: 15 J. – zu alt für WG2"`.
 * `reason` comes from the backend (`"zu alt"` / `"zu jung"`).
 */
export function formatWgWarning(
  warning: WgWarning,
  runnersById: ReadonlyMap<string, RunnerName>,
): string {
  const name = runnerDisplayName(runnersById.get(warning.runner_id));
  const reason = warning.reason?.trim() || 'passt nicht';
  return `${name}: ${warning.age} J. – ${reason} für ${wgLabel(warning.wg)}`;
}

export function formatWgWarnings(
  warnings: readonly WgWarning[] | null | undefined,
  runnersById: ReadonlyMap<string, RunnerName>,
): string[] {
  return (warnings ?? []).map((w) => formatWgWarning(w, runnersById));
}

/** Tooltip text for the red marker; empty string when there is nothing to warn. */
export function wgWarningTooltip(
  warnings: readonly WgWarning[] | null | undefined,
  runnersById: ReadonlyMap<string, RunnerName>,
): string {
  const lines = formatWgWarnings(warnings, runnersById);
  if (lines.length === 0) return '';
  return ['Alter passt nicht zur Wertungsgruppe:', ...lines].join('\n');
}

function formatDateTimeDE(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** `"WG2 → WG1 · von orga@example.de · 10.10.2027, 09:15 · 2 Mädchen"` */
export function formatWgHistoryEntry(entry: WgHistoryEntry): string {
  const parts = [
    `${wgLabel(entry.from, WG_MISSING_LABEL)} → ${wgLabel(entry.to, WG_MISSING_LABEL)}`,
  ];
  if (entry.by) parts.push(`von ${entry.by}`);
  if (entry.at) parts.push(formatDateTimeDE(entry.at));
  if (entry.note && entry.note.trim()) parts.push(entry.note.trim());
  return parts.join(' · ');
}
