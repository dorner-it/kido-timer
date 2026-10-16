import type { ConnectionEvent } from "./types";

/**
 * Dev-only simulator that emits synthetic frames so the UI can be exercised
 * without a Tauri runtime or real hardware. Active only when:
 *   - Not running inside Tauri (plain browser)
 *   - Vite is in dev mode
 */
export function startDevSimulator(
  emit: (event: ConnectionEvent) => void,
): () => void {
  const start = performance.now();
  let stopped = false;

  emit({ type: "connected", source: "simulator (browser dev)" });

  const interval = window.setInterval(() => {
    if (stopped) return;
    const t = performance.now() - start;
    // 30s cycle: 0-2s standby, 2-12s lanes 1+2 running, 12s lane1 confirms,
    // 13s lane2 captures, 18s lane2 confirms, 22s reset, repeat
    const cycle = (t / 1000) % 30;

    const channels: Array<{ timeMs: number; status: "inactive" | "running" | "captured" | "confirmed" | "unknown"; statusRaw: number }> = [
      { timeMs: 0, status: "inactive", statusRaw: 0 },
      { timeMs: 0, status: "inactive", statusRaw: 0 },
      { timeMs: 0, status: "inactive", statusRaw: 0 },
      { timeMs: 0, status: "inactive", statusRaw: 0 },
    ];

    if (cycle >= 2 && cycle < 12) {
      const elapsed = Math.round((cycle - 2) * 1000);
      channels[0] = { timeMs: elapsed, status: "running", statusRaw: 0x81 };
      channels[1] = { timeMs: elapsed, status: "running", statusRaw: 0x81 };
    } else if (cycle >= 12 && cycle < 13) {
      channels[0] = { timeMs: 10240, status: "confirmed", statusRaw: 0x84 };
      channels[1] = { timeMs: Math.round((cycle - 2) * 1000), status: "running", statusRaw: 0x81 };
    } else if (cycle >= 13 && cycle < 18) {
      channels[0] = { timeMs: 10240, status: "confirmed", statusRaw: 0x84 };
      channels[1] = { timeMs: 11420, status: "captured", statusRaw: 0x82 };
    } else if (cycle >= 18 && cycle < 22) {
      channels[0] = { timeMs: 10240, status: "confirmed", statusRaw: 0x84 };
      channels[1] = { timeMs: 11420, status: "confirmed", statusRaw: 0x84 };
    }

    emit({
      type: "frame",
      frame: {
        deviceMode: cycle < 2 ? "standby" : "active",
        deviceModeRaw: cycle < 2 ? 0 : 2,
        lane: 1,
        stateFlag: "measuring",
        stateFlagRaw: 0,
        channels: channels as [
          (typeof channels)[number],
          (typeof channels)[number],
          (typeof channels)[number],
          (typeof channels)[number],
        ],
        rawHex: "52 57 3A 01 02 01 00 …",
      },
    });
  }, 41); // ~24fps to mimic real device

  return () => {
    stopped = true;
    window.clearInterval(interval);
    emit({ type: "disconnected" });
  };
}
