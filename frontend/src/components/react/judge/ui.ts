// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Shared class strings of the judge app. Phone-first, large touch
 * targets (≥ 56 px, usable with gloves) and high contrast (solid
 * fills, dark text, thick borders) for outdoor use in sunlight.
 */

export const BIG_BTN =
  'flex min-h-[64px] w-full items-center gap-3 rounded-xl border-2 px-4 py-3 text-left text-[17px] font-semibold leading-snug transition active:scale-[0.98] disabled:opacity-40 disabled:active:scale-100';

export const BTN_NEUTRAL = `${BIG_BTN} border-ink-50 bg-white text-ink-50`;
export const BTN_PRIMARY = `${BIG_BTN} justify-center border-ink-50 bg-ink-50 text-white`;
export const BTN_DANGER = `${BIG_BTN} justify-center border-red-800 bg-red-700 text-white`;
export const BTN_CANCEL = `${BIG_BTN} justify-center border-ink-300 bg-white text-ink-50`;

export const SZ_BADGE =
  'inline-flex shrink-0 flex-col items-center justify-center rounded-lg bg-amber-400 px-2 py-1 font-mono text-[13px] font-bold leading-tight text-black';
export const FV_BADGE =
  'inline-flex shrink-0 flex-col items-center justify-center rounded-lg bg-red-700 px-2 py-1 font-mono text-[13px] font-bold leading-tight text-white';

export const CARD = 'rounded-2xl border-2 border-ink-50 bg-white';
