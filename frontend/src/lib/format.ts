// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

export function formatDateDE(iso: string): string {
  return new Date(iso + 'T00:00:00').toLocaleDateString('de-DE', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
  });
}

export function formatTimeMs(ms: number | null | undefined): string {
  if (ms == null) return '—';
  const sign = ms < 0 ? '-' : '';
  const abs = Math.abs(ms);
  const minutes = Math.floor(abs / 60000);
  const seconds = Math.floor((abs % 60000) / 1000);
  const hundredths = Math.floor((abs % 1000) / 10);
  if (minutes > 0) {
    return `${sign}${minutes}:${seconds.toString().padStart(2, '0')}.${hundredths.toString().padStart(2, '0')}`;
  }
  return `${sign}${seconds}.${hundredths.toString().padStart(2, '0')}`;
}

export function formatDeltaSeconds(ms: number): string {
  const seconds = ms / 1000;
  const sign = seconds >= 0 ? '+' : '';
  if (Number.isInteger(seconds)) {
    return `${sign}${seconds} s`;
  }
  return `${sign}${seconds.toFixed(2)} s`;
}
