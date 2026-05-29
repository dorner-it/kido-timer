// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import type { TimerMode } from './api';

/**
 * Discipline-aware penalty presets for the Decider pad.
 *
 * These are *starting defaults* aligned with the German Feuerwehr-
 * Wettkampfordnung family (TRV Kocab competitions). Edit freely;
 * the reason_code is the durable identifier (kept stable across
 * label tweaks), reason_label is what appears on the button caption,
 * and variant changes the visual emphasis.
 */
export interface PenaltyPreset {
  /** Big stencil label (e.g. "+ 10 s"). */
  label: string;
  /** Time delta added to the run's correction in milliseconds. */
  delta_ms: number;
  /** Stable identifier sent to the backend; tag for later analytics. */
  reason_code: string;
  /** Short human-readable label rendered as caption under the button. */
  reason_label: string;
  /** Visual emphasis. */
  variant?: 'default' | 'severe' | 'critical';
}

const COMMON_DSQ: PenaltyPreset = {
  label: 'DSQ',
  // 999 s effectively guarantees last place; the standalone app
  // surfaces this as a disqualification, not a literal time.
  delta_ms: 999_000,
  reason_code: 'dsq',
  reason_label: 'Disqualifikation',
  variant: 'critical',
};

const COMMON_SEVERE: PenaltyPreset = {
  label: '+ 1:00',
  delta_ms: 60_000,
  reason_code: 'schwerer_verstoss',
  reason_label: 'Schwerer Verstoß',
  variant: 'severe',
};

export const PRESETS_BY_MODE: Record<TimerMode, PenaltyPreset[]> = {
  // Löschangriff: 4 Bahnen, klassische Feuerwehr-Wettkampfordnung.
  loeschangriff: [
    { label: '+ 5 s', delta_ms: 5_000, reason_code: 'kleiner_fehler', reason_label: 'Kleiner Fehler' },
    { label: '+ 10 s', delta_ms: 10_000, reason_code: 'knoten_bund', reason_label: 'Knoten / Bund' },
    { label: '+ 20 s', delta_ms: 20_000, reason_code: 'schlauchfuehrung', reason_label: 'Schlauchführung' },
    { label: '+ 30 s', delta_ms: 30_000, reason_code: 'strahlrohr', reason_label: 'Strahlrohrführer' },
    COMMON_SEVERE,
    COMMON_DSQ,
  ],
  // Gruppenstaffette: 2 Bahnen, Staffellauf-Format.
  gruppenstaffette: [
    { label: '+ 5 s', delta_ms: 5_000, reason_code: 'kleiner_fehler', reason_label: 'Kleiner Fehler' },
    { label: '+ 10 s', delta_ms: 10_000, reason_code: 'uebergabe', reason_label: 'Übergabefehler' },
    { label: '+ 20 s', delta_ms: 20_000, reason_code: 'wechselzone', reason_label: 'Wechselzone' },
    { label: '+ 30 s', delta_ms: 30_000, reason_code: 'hindernis', reason_label: 'Hindernisfehler' },
    COMMON_SEVERE,
    COMMON_DSQ,
  ],
};

/**
 * Reason vocabulary keyed by mode, for the "Andere …" custom-value form's
 * dropdown. Always includes a free-text escape (`null`).
 */
export function reasonsForMode(mode: TimerMode): { code: string; label: string }[] {
  return PRESETS_BY_MODE[mode].map((p) => ({
    code: p.reason_code,
    label: p.reason_label,
  }));
}

/**
 * Tailwind classes per variant. Kept here so the pad component is purely
 * structural; restyle the discipline by editing this map.
 */
export function variantClasses(variant: PenaltyPreset['variant']): string {
  switch (variant) {
    case 'critical':
      return 'border-status-unknown/60 bg-status-unknown/10 hover:bg-status-unknown/20';
    case 'severe':
      return 'border-status-armed/60 bg-status-armed/10 hover:bg-status-armed/20';
    default:
      return '';
  }
}
