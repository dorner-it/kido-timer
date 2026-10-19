// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Tap flow for one error entry (pure state machine, UI-agnostic).
 *
 *   tap error ──┬─ SZ, one run ────────────────────────────▶ submit
 *               ├─ SZ, two runs ─▶ choose_run (Bahn 1 / 2) ▶ submit
 *               └─ FV (one or two runs) ─▶ confirm_fv ─────▶ submit
 *
 * For FV with two runs the lane buttons *are* the confirmation
 * ("Fehlversuch – Lauf ungültig?" + "Ja, Bahn 1 · Team" / "Ja, Bahn 2 ·
 * Team"), so an FV is never more than two taps and never one.
 */

import type { PenaltyKind } from './api';
import { currentRunLabel, type CurrentRunLike, type SubmitTarget } from './decider';

export interface EntryLike {
  code: string;
  kind: PenaltyKind;
  label_de: string;
}

export interface RunChoice {
  run_id: string;
  lane: number | null;
  label: string;
}

export type EntryFlow<E extends EntryLike = EntryLike> =
  | { step: 'idle' }
  | { step: 'blocked'; message: string }
  | { step: 'choose_run'; entry: E; choices: RunChoice[] }
  | { step: 'confirm_fv'; entry: E; choices: RunChoice[] }
  | { step: 'submit'; entry: E; run_id: string };

export const IDLE: EntryFlow = { step: 'idle' };

export const FV_CONFIRM_TITLE = 'Fehlversuch – Lauf ungültig?';

export function runChoice(cr: CurrentRunLike): RunChoice {
  return { run_id: cr.run.id, lane: cr.run.lane, label: currentRunLabel(cr) };
}

/** First step after tapping a catalogue button. */
export function startEntry<E extends EntryLike>(
  entry: E,
  target: SubmitTarget,
): EntryFlow<E> {
  let choices: RunChoice[];
  if (target.ok) choices = [runChoice(target.run)];
  else if (target.reason === 'choose_run') choices = target.options.map(runChoice);
  else return { step: 'blocked', message: target.message };

  if (entry.kind === 'fv') return { step: 'confirm_fv', entry, choices };
  if (choices.length === 1) return { step: 'submit', entry, run_id: choices[0]!.run_id };
  return { step: 'choose_run', entry, choices };
}

/** Tap on a lane / confirm button. Unknown run ids leave the flow unchanged. */
export function chooseRun<E extends EntryLike>(flow: EntryFlow<E>, runId: string): EntryFlow<E> {
  if (flow.step !== 'choose_run' && flow.step !== 'confirm_fv') return flow;
  if (!flow.choices.some((c) => c.run_id === runId)) return flow;
  return { step: 'submit', entry: flow.entry, run_id: runId };
}

export function cancelEntry(): EntryFlow {
  return IDLE;
}

/** The current runs changed while a dialog was open: drop choices that
 *  are gone; nothing left → cancel. */
export function reconcileFlow<E extends EntryLike>(
  flow: EntryFlow<E>,
  currentRunIds: readonly string[],
): EntryFlow<E> {
  if (flow.step !== 'choose_run' && flow.step !== 'confirm_fv') return flow;
  const choices = flow.choices.filter((c) => currentRunIds.includes(c.run_id));
  if (choices.length === flow.choices.length) return flow;
  if (choices.length === 0) return IDLE as EntryFlow<E>;
  return { ...flow, choices };
}

/** Button text of a choice in the dialog. */
export function choiceButtonLabel(flow: EntryFlow, choice: RunChoice): string {
  if (flow.step === 'confirm_fv') {
    return flow.choices.length > 1 ? `Ja, ${choice.label}` : 'Ja, Fehlversuch melden';
  }
  return choice.label;
}

/** Dialog title. */
export function flowTitle(flow: EntryFlow): string | null {
  if (flow.step === 'confirm_fv') return FV_CONFIRM_TITLE;
  if (flow.step === 'choose_run') return 'Welche Bahn?';
  return null;
}

/** A dialog (lane choice / FV confirmation) is open. */
export function isDialogOpen(flow: EntryFlow): boolean {
  return flow.step === 'choose_run' || flow.step === 'confirm_fv';
}
