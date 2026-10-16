import { t } from "./i18n";
import { totalTimeMs, type RunEntry } from "./types";

/**
 * Format a millisecond duration as `s,mmm` using a German decimal comma.
 * Example: 10240 -> "10,240"
 */
function formatDe(ms: number): string {
  const sign = ms < 0 ? "-" : "";
  const abs = Math.abs(ms);
  const secs = Math.floor(abs / 1000);
  const millis = abs % 1000;
  return `${sign}${secs}.${millis.toString().padStart(3, "0")}`.replace(".", ",");
}

function escapeCsv(value: string): string {
  if (/[";\n\r]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

/**
 * Build a German-locale-friendly CSV (semicolon separator, comma decimal,
 * UTF-8 BOM so Excel auto-detects the encoding).
 *
 * Columns: Lauf · Bahn · Original-Zeit · Strafzeit · Gesamt-Zeit · Zeitstempel · Datum
 */
export function entriesToCsv(entries: RunEntry[]): string {
  const SEP = ";";
  const lines: string[] = [];
  lines.push(t.csv.headers.map(escapeCsv).join(SEP));
  // Reverse so the export is oldest-first (chronological)
  const ordered = [...entries].reverse();
  ordered.forEach((entry, idx) => {
    const date = new Date(entry.timestamp);
    const dateStr = date.toLocaleDateString("de-DE");
    const timeStr = date.toLocaleTimeString("de-DE", { hour12: false });
    lines.push(
      [
        String(idx + 1),
        `Bahn ${entry.channel}`,
        formatDe(entry.originalTimeMs),
        formatDe(entry.correctionMs),
        formatDe(totalTimeMs(entry)),
        timeStr,
        dateStr,
      ]
        .map(escapeCsv)
        .join(SEP),
    );
  });
  return "﻿" + lines.join("\r\n") + "\r\n";
}

export function defaultCsvFilename(now: Date = new Date()): string {
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `KiDo_Protokoll_${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}.csv`;
}
