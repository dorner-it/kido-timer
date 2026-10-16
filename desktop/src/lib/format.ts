import type { ChannelStatus } from "./types";

/**
 * Format a millisecond value as `MM:SS.mmm` (e.g. 3620 -> "00:03.620",
 * 125_400 -> "02:05.400"). Returns dashes for inactive lanes. Capped at
 * 99:59.999 — the KiDo hardware never goes past two-digit minutes.
 */
export function formatTime(timeMs: number, status: ChannelStatus): string {
  if (status === "inactive") return "––:––.–––";
  const ms = Math.max(0, Math.min(99 * 60_000 + 59_999, Math.round(timeMs)));
  const mins = Math.floor(ms / 60_000);
  const secs = Math.floor((ms % 60_000) / 1000);
  const millis = ms % 1000;
  return `${mins.toString().padStart(2, "0")}:${secs
    .toString()
    .padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

/**
 * Format a correction delta as `±MM:SS.mmm`. Used both on the lane card
 * footer and inside the breakdown bubble.
 */
export function formatCorrection(ms: number): string {
  if (ms === 0) return "±00:00.000";
  const sign = ms > 0 ? "+" : "−";
  const abs = Math.min(99 * 60_000 + 59_999, Math.abs(ms));
  const mins = Math.floor(abs / 60_000);
  const secs = Math.floor((abs % 60_000) / 1000);
  const millis = abs % 1000;
  return `${sign}${mins.toString().padStart(2, "0")}:${secs
    .toString()
    .padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

/**
 * Status accent colors as CSS variable references — automatically swap
 * between light and dark themes. Safe to use in inline styles and (via
 * the `style` prop) on SVG `stop-color`, `fill`, `stroke`, etc.
 */
export const STATUS_COLOR: Record<ChannelStatus, string> = {
  inactive: "rgb(var(--c-status-inactive))",
  running: "rgb(var(--c-status-running))",
  captured: "rgb(var(--c-status-captured))",
  confirmed: "rgb(var(--c-status-confirmed))",
  unknown: "rgb(var(--c-status-unknown))",
};

/** Same accents but as just the rgb-triple — for `rgb(... / 0.X)` alpha use. */
export const STATUS_RGB: Record<ChannelStatus, string> = {
  inactive: "var(--c-status-inactive)",
  running: "var(--c-status-running)",
  captured: "var(--c-status-captured)",
  confirmed: "var(--c-status-confirmed)",
  unknown: "var(--c-status-unknown)",
};
