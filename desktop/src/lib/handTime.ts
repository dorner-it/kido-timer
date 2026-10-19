/**
 * Hand-time input (light barrier failed): parse / format stopwatch
 * readings and validate both inputs. Same rules and wording as the web
 * organizer review (PR 3). The mean itself comes from
 * `kido_core::timing::hand_time_mean` via the `hand_time_mean` command.
 */
import type { Run } from "./cloudTypes";

const U32_MAX = 4_294_967_295;

export type ParseTimeResult = { ok: true; ms: number } | { ok: false; error: string };

// [m:]ss[,hh] — minutes optional, separator comma or dot, 1–3 fraction digits.
const TIME_RE = /^(?:(\d{1,3}):)?(\d+)(?:[.,](\d{1,3}))?$/;

/** `47,23` · `47.23` · `0:47,23` · `1:02,50` · `62,5` · `47` · `47,235`. */
export function parseTimeInput(raw: string | null | undefined): ParseTimeResult {
  const s = (raw ?? "").replace(/\s+/g, "");
  if (s === "") return { ok: false, error: "Bitte eine Zeit eingeben." };
  const m = TIME_RE.exec(s);
  if (!m) {
    return { ok: false, error: "Format: m:ss,hh oder ss,hh (z.B. 47,23 oder 1:02,50)." };
  }
  const [, minStr, secStr, fracStr] = m;
  const minutes = minStr != null ? parseInt(minStr, 10) : 0;
  const seconds = parseInt(secStr ?? "0", 10);
  if (minStr != null && ((secStr ?? "").length > 2 || seconds > 59)) {
    return { ok: false, error: "Mit Minuten müssen die Sekunden zwischen 0 und 59 liegen." };
  }
  const fracMs = fracStr != null ? parseInt(fracStr.padEnd(3, "0"), 10) : 0;
  const ms = (minutes * 60 + seconds) * 1000 + fracMs;
  if (!Number.isSafeInteger(ms) || ms > U32_MAX) {
    return { ok: false, error: "Zeit ist zu groß." };
  }
  if (ms <= 0) return { ok: false, error: "Die Zeit muss größer als 0 sein." };
  return { ok: true, ms };
}

/** `47,23`, `1:02,50`; a non-zero ms digit keeps three digits (`47,235`). */
export function formatTimeInput(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "";
  const abs = Math.abs(Math.trunc(ms));
  const sign = ms < 0 ? "-" : "";
  const minutes = Math.floor(abs / 60_000);
  const seconds = Math.floor((abs % 60_000) / 1000);
  const frac = abs % 1000;
  const fracStr =
    frac % 10 === 0 ? String(frac / 10).padStart(2, "0") : String(frac).padStart(3, "0");
  if (minutes > 0) return `${sign}${minutes}:${String(seconds).padStart(2, "0")},${fracStr}`;
  return `${sign}${seconds},${fracStr}`;
}

/** Above this spread between the two stopwatches the UI shows a hint. */
export const HAND_TIME_SPREAD_WARN_MS = 1000;

export type HandTimesValidation =
  | { ok: true; t1: number; t2: number; spreadMs: number; warning: string | null }
  | { ok: false; errors: { t1?: string; t2?: string } };

export function validateHandTimes(raw1: string, raw2: string): HandTimesValidation {
  const p1 = parseTimeInput(raw1);
  const p2 = parseTimeInput(raw2);
  if (!p1.ok || !p2.ok) {
    const errors: { t1?: string; t2?: string } = {};
    if (!p1.ok) errors.t1 = p1.error;
    if (!p2.ok) errors.t2 = p2.error;
    return { ok: false, errors };
  }
  const spreadMs = Math.abs(p1.ms - p2.ms);
  return {
    ok: true,
    t1: p1.ms,
    t2: p2.ms,
    spreadMs,
    warning:
      spreadMs > HAND_TIME_SPREAD_WARN_MS
        ? `Die beiden Handzeiten weichen um ${formatTimeInput(spreadMs)} s ab – bitte prüfen.`
        : null,
  };
}

export function isHandTime(run: Pick<Run, "time_source">): boolean {
  return run.time_source === "handstopp";
}

/** `"47,23 / 47,25"` or `null` when not a hand time. */
export function handTimeRawText(
  run: Pick<Run, "time_source" | "hand_time_1_ms" | "hand_time_2_ms">,
): string | null {
  if (!isHandTime(run)) return null;
  const a = run.hand_time_1_ms != null ? formatTimeInput(run.hand_time_1_ms) : "—";
  const b = run.hand_time_2_ms != null ? formatTimeInput(run.hand_time_2_ms) : "—";
  return `${a} / ${b}`;
}
