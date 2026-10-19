// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * zod schemas for the PR 1 (2027) wire contract.
 *
 * Two families:
 * - **read schemas** normalize API responses. Every field that is new in
 *   schema v4 is optional/defaulted so pre-2027 rows (and an older
 *   backend) still load. They never reject a row because of a missing
 *   new field; `normalize*` helpers additionally fall back to the raw
 *   value if a row is malformed in some unexpected way.
 * - **form schemas** validate organizer input before it is sent.
 */

import { z } from 'zod';

import type {
  CatalogueEntry,
  Discipline,
  Event,
  EventDetail,
  Penalty,
  Run,
  Runner,
  StationDeciderState,
  TeamEntry,
} from './api';
import { effectiveLaneCount, isValidLaneCount } from './lanes';
import { penaltyKind } from './penalties';
import { normalizeStation, normalizeStations } from './stations';

// ===== enums =====

export const timerModeSchema = z.enum(['gruppenstaffette', 'loeschangriff']);
export const judgeModeSchema = z.enum(['per_station', 'per_station_and_lane']);
export const wertungsgruppeSchema = z.enum(['wg1', 'wg2', 'wg3', 'wg4', 'wg5']);
export const penaltyKindSchema = z.enum(['sz', 'fv']);
export const timeSourceSchema = z.enum(['lichtschranke', 'handstopp']);
export const lineupConfirmedBySchema = z.enum(['startlinie', 'organizer']);

/** Canonical station key; legacy keys are mapped, unknown → `null`. */
const stationReadSchema = z.unknown().transform((v) => normalizeStation(v));

const nullableString = z.string().nullable().catch(null);
const nullableInt = z.number().int().nullable().catch(null);

// ===== read schemas =====

export const disciplineReadSchema = z
  .object({
    mode: timerModeSchema,
    lane_count: z.number().int().nullish().catch(null),
    judge_mode: judgeModeSchema.catch('per_station'),
    lane_conflicts: z.array(z.string()).optional().catch(undefined),
  })
  .passthrough()
  .transform((d) => ({ ...d, lane_count: effectiveLaneCount(d) }));

export const eventReadSchema = z
  .object({
    current_run_id: z.string().nullable().catch(null),
    current_run_ids: z.array(z.string()).optional().catch(undefined),
  })
  .passthrough()
  .transform((e) => ({
    ...e,
    current_run_ids:
      e.current_run_ids ?? (e.current_run_id ? [e.current_run_id] : []),
  }));

export const runnerReadSchema = z
  .object({
    gender: z.enum(['male', 'female']).nullable().catch(null),
  })
  .passthrough();

export const wgHistoryEntrySchema = z
  .object({
    from: wertungsgruppeSchema.nullable().catch(null),
    to: wertungsgruppeSchema.nullable().catch(null),
    by: z.string().catch(''),
    at: z.string().catch(''),
    note: z.string().nullish().catch(null),
  })
  .passthrough();

export const wgWarningSchema = z
  .object({
    runner_id: z.string(),
    age: z.number(),
    wg: wertungsgruppeSchema,
    reason: z.string().catch(''),
  })
  .passthrough();

export const teamEntryReadSchema = z
  .object({
    wg: wertungsgruppeSchema.nullable().catch(null),
    wg_history: z.array(wgHistoryEntrySchema).catch([]),
    wg_warnings: z.array(wgWarningSchema).catch([]),
  })
  .passthrough();

export const runReadSchema = z
  .object({
    lineup: z.array(z.string()).catch([]),
    lineup_confirmed_at: nullableString,
    lineup_confirmed_by: lineupConfirmedBySchema.nullable().catch(null),
    time_source: timeSourceSchema.catch('lichtschranke'),
    hand_time_1_ms: nullableInt,
    hand_time_2_ms: nullableInt,
    penalties_accepted_at: nullableString,
  })
  .passthrough();

export const penaltyReadSchema = z
  .object({
    kind: penaltyKindSchema.optional().catch(undefined),
    is_disqualification: z.boolean().catch(false),
    code: nullableString,
    station: stationReadSchema,
    reason_code: nullableString,
    delta_ms: z.number().catch(0),
  })
  .passthrough()
  .transform((p) => ({ ...p, kind: penaltyKind(p) }));

export const catalogueEntrySchema = z
  .object({
    code: z.string().min(1),
    station: stationReadSchema,
    kind: penaltyKindSchema,
    label_de: z.string(),
  })
  .passthrough();

function safe<T>(schema: z.ZodTypeAny, raw: unknown): T {
  const res = schema.safeParse(raw);
  if (res.success) return res.data as T;
  if (typeof console !== 'undefined') {
    console.warn('[kido] could not normalize API row', res.error.issues, raw);
  }
  return raw as T;
}

export function normalizeDiscipline(raw: unknown): Discipline {
  return safe<Discipline>(disciplineReadSchema, raw);
}
export function normalizeEvent(raw: unknown): Event {
  return safe<Event>(eventReadSchema, raw);
}
export function normalizeRunner(raw: unknown): Runner {
  return safe<Runner>(runnerReadSchema, raw);
}
export function normalizeTeamEntry(raw: unknown): TeamEntry {
  return safe<TeamEntry>(teamEntryReadSchema, raw);
}
export function normalizeRun(raw: unknown): Run {
  return safe<Run>(runReadSchema, raw);
}
export function normalizePenalty(raw: unknown): Penalty {
  return safe<Penalty>(penaltyReadSchema, raw);
}

/** Catalogue rows with an unknown station are dropped. */
export function normalizeCatalogue(raw: unknown): CatalogueEntry[] {
  if (!Array.isArray(raw)) return [];
  const out: CatalogueEntry[] = [];
  for (const row of raw) {
    const res = catalogueEntrySchema.safeParse(row);
    if (res.success && res.data.station) out.push(res.data as CatalogueEntry);
  }
  return out;
}

function mapArray<T>(raw: unknown, fn: (r: unknown) => T): T[] {
  return Array.isArray(raw) ? raw.map(fn) : [];
}

export function normalizeEventDetail(raw: unknown): EventDetail {
  const d = (raw ?? {}) as Record<string, unknown>;
  return {
    ...(d as object),
    event: normalizeEvent(d.event),
    disciplines: mapArray(d.disciplines, normalizeDiscipline),
    teams: (Array.isArray(d.teams) ? d.teams : []) as EventDetail['teams'],
    runners: mapArray(d.runners, normalizeRunner),
    team_entries: mapArray(d.team_entries, normalizeTeamEntry),
    runs: mapArray(d.runs, normalizeRun),
  } as EventDetail;
}

type CurrentRunRaw = { run: unknown; team: unknown; runners?: unknown; runner?: unknown };

function normalizeCurrentRun(raw: unknown) {
  const c = raw as CurrentRunRaw;
  return {
    ...(c as object),
    run: normalizeRun(c.run),
    team: c.team,
    runners: mapArray(c.runners, normalizeRunner),
    runner: c.runner ? normalizeRunner(c.runner) : null,
  };
}

/**
 * Station state: `current_runs` falls back to `[current_run]` for an
 * older backend; `current_run` is kept = `current_runs[0]`. Runs are
 * ordered by lane (nulls last), as promised by the contract.
 */
export function normalizeStationState(raw: unknown): StationDeciderState {
  const s = (raw ?? {}) as Record<string, unknown>;
  const fromList = Array.isArray(s.current_runs) ? s.current_runs : null;
  const list = (fromList ?? (s.current_run ? [s.current_run] : [])).map(normalizeCurrentRun);
  list.sort((a, b) => (a.run.lane ?? Infinity) - (b.run.lane ?? Infinity));
  const discipline = s.discipline ? normalizeDiscipline(s.discipline) : null;
  return {
    ...(s as object),
    discipline,
    current_runs: list,
    current_run: list[0] ?? null,
    disabled: Boolean(s.disabled),
    allowed_stations: normalizeStations(s.allowed_stations),
    penalties: Array.isArray(s.penalties) ? s.penalties.map(normalizePenalty) : undefined,
  } as StationDeciderState;
}

// ===== form schemas =====

export const disciplineFormSchema = z
  .object({
    name: z.string().trim().min(1, 'Bitte einen Namen eingeben.'),
    mode: timerModeSchema,
    position: z.number().int().min(0),
    lane_count: z.number({ invalid_type_error: 'Bitte Bahnenzahl wählen.' }).int(),
    judge_mode: judgeModeSchema,
  })
  .superRefine((d, ctx) => {
    if (!isValidLaneCount(d.mode, d.lane_count)) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['lane_count'],
        message:
          d.mode === 'loeschangriff'
            ? 'Löschangriff hat immer 4 Bahnen.'
            : 'Gruppenstaffette hat 1 oder 2 Bahnen.',
      });
    }
  });
export type DisciplineFormValues = z.infer<typeof disciplineFormSchema>;

const WG_REQUIRED = 'Bitte Wertungsgruppe wählen.';

export const teamEntryFormSchema = z.object({
  team_id: z.string().min(1, 'Bitte Mannschaft wählen.'),
  start_number: z
    .number({ invalid_type_error: 'Startnummer fehlt.' })
    .int()
    .min(1, 'Startnummer muss mindestens 1 sein.'),
  lane: z.number().int().min(1).nullable(),
  wg: z.enum(['wg1', 'wg2', 'wg3', 'wg4', 'wg5'], {
    errorMap: () => ({ message: WG_REQUIRED }),
  }),
});
export type TeamEntryFormValues = z.infer<typeof teamEntryFormSchema>;

export const moveWgFormSchema = z.object({
  wg: z.enum(['wg1', 'wg2', 'wg3', 'wg4', 'wg5'], {
    errorMap: () => ({ message: 'Bitte Ziel-WG wählen.' }),
  }),
  note: z
    .string()
    .trim()
    .max(500, 'Notiz ist zu lang (max. 500 Zeichen).')
    .optional()
    .transform((v) => (v ? v : undefined)),
});
export type MoveWgFormValues = z.infer<typeof moveWgFormSchema>;

/** Runner input — no gender any more (2027). */
export const runnerFormSchema = z
  .object({
    first_name: z.string().trim().min(1),
    last_name: z.string().trim().min(1),
    birth_date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
    position: z.number().int().min(1),
  })
  .strip();
export type RunnerFormValues = z.infer<typeof runnerFormSchema>;

export const lineupSchema = z
  .array(z.string().min(1))
  .refine((ids) => new Set(ids).size === ids.length, 'Läufer doppelt in der Besetzung.');

export const handTimesSchema = z.object({
  hand_time_1_ms: z.number().int().positive(),
  hand_time_2_ms: z.number().int().positive(),
});

/** First error message per field, for simple `useState` forms. */
export function fieldErrors(error: z.ZodError): Record<string, string> {
  const out: Record<string, string> = {};
  for (const issue of error.issues) {
    const key = issue.path.join('.') || '_';
    if (!(key in out)) out[key] = issue.message;
  }
  return out;
}
