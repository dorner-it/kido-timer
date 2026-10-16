import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  connectSerial,
  currentSource,
  disconnect as disconnectCmd,
  IS_TAURI,
  listenConnectionEvents,
  sendReset,
  startDemo,
} from "./tauri";
import { startDevSimulator } from "./devSimulator";
import type { ConnectionEvent, ConnectionStatus, FrameDto, RunEntry } from "./types";

interface State {
  status: ConnectionStatus;
  source: string | null;
  frame: FrameDto | null;
  error: string | null;
  fps: number;
  frameCount: number;
  errorCount: number;
  history: RunEntry[];
  /** Per-lane (0-3) operator-applied correction in ms. */
  corrections: number[];
  /**
   * Per-lane id of the currently "active" run entry — i.e. the one that's
   * still being shown on the device. When the lane starts a new run this
   * resets to null. Used to retroactively update an entry when corrections
   * change after the time was confirmed.
   */
  activeEntryId: Array<string | null>;
  /** Per-lane last seen confirmed device time, used to dedupe entries. */
  lastConfirmed: Array<number | null>;
}

type Action =
  | { type: "frame"; frame: FrameDto }
  | { type: "tick"; fps: number }
  | { type: "error"; message: string }
  | { type: "connecting" }
  | { type: "connected"; source: string }
  | { type: "disconnected" }
  | { type: "clearError" }
  | { type: "clearHistory" }
  | { type: "adjustCorrection"; lane: number; deltaMs: number }
  | { type: "clearCorrection"; lane: number };

const HISTORY_LIMIT = 256;

function applyCorrectionToActiveEntry(
  history: RunEntry[],
  activeEntryId: Array<string | null>,
  lane: number,
  newCorrectionMs: number,
): RunEntry[] {
  const id = activeEntryId[lane];
  if (!id) return history;
  return history.map((e) =>
    e.id === id ? { ...e, correctionMs: newCorrectionMs } : e,
  );
}

function reducer(s: State, a: Action): State {
  switch (a.type) {
    case "connecting":
      return { ...s, status: "connecting", error: null };
    case "connected":
      return { ...s, status: "connected", source: a.source, error: null };
    case "disconnected":
      return { ...s, status: "idle", source: null, fps: 0 };
    case "error":
      return {
        ...s,
        error: a.message,
        errorCount: s.errorCount + 1,
        status: s.status === "connecting" ? "error" : s.status,
      };
    case "clearError":
      return { ...s, error: null };
    case "tick":
      return { ...s, fps: a.fps };
    case "clearHistory":
      return {
        ...s,
        history: [],
        lastConfirmed: [null, null, null, null],
        activeEntryId: [null, null, null, null],
      };
    case "adjustCorrection": {
      const newCorr = (s.corrections[a.lane] ?? 0) + a.deltaMs;
      const corrections = s.corrections.map((v, i) => (i === a.lane ? newCorr : v));
      const history = applyCorrectionToActiveEntry(
        s.history,
        s.activeEntryId,
        a.lane,
        newCorr,
      );
      return { ...s, corrections, history };
    }
    case "clearCorrection": {
      const corrections = s.corrections.map((v, i) => (i === a.lane ? 0 : v));
      const history = applyCorrectionToActiveEntry(
        s.history,
        s.activeEntryId,
        a.lane,
        0,
      );
      return { ...s, corrections, history };
    }
    case "frame": {
      const newHistory = [...s.history];
      const lastConfirmed = [...s.lastConfirmed];
      const activeEntryId = [...s.activeEntryId];

      a.frame.channels.forEach((ch, idx) => {
        // The hardware freezes timeMs on both "captured" and "confirmed".
        // Some setups never advance past "captured" (operator hasn't pressed
        // the device-side confirm, or the discipline doesn't need it — e.g.
        // a full water tank stopping a Löschangriff lane on the line) so we
        // treat captured as a result candidate too. The dedupe on
        // lastConfirmed[idx] === ch.timeMs guarantees we only record the
        // entry once even if captured → confirmed transitions later.
        if (ch.status === "captured" || ch.status === "confirmed") {
          if (lastConfirmed[idx] !== ch.timeMs) {
            const id = `${Date.now()}-${idx}-${ch.timeMs}`;
            newHistory.unshift({
              id,
              timestamp: Date.now(),
              lane: a.frame.lane,
              channel: idx + 1,
              originalTimeMs: ch.timeMs,
              correctionMs: s.corrections[idx] ?? 0,
            });
            lastConfirmed[idx] = ch.timeMs;
            activeEntryId[idx] = id;
          }
        } else if (ch.status === "inactive" || ch.status === "running") {
          // A new run is starting — break the link to the old entry so future
          // corrections don't leak into a finished result.
          activeEntryId[idx] = null;
          if (lastConfirmed[idx] !== null && ch.timeMs < (lastConfirmed[idx] ?? 0)) {
            lastConfirmed[idx] = null;
          }
        }
      });

      while (newHistory.length > HISTORY_LIMIT) newHistory.pop();
      return {
        ...s,
        status: "connected",
        frame: a.frame,
        frameCount: s.frameCount + 1,
        history: newHistory,
        lastConfirmed,
        activeEntryId,
      };
    }
  }
}

const INITIAL: State = {
  status: "idle",
  source: null,
  frame: null,
  error: null,
  fps: 0,
  frameCount: 0,
  errorCount: 0,
  history: [],
  corrections: [0, 0, 0, 0],
  activeEntryId: [null, null, null, null],
  lastConfirmed: [null, null, null, null],
};

export function useConnection() {
  const [state, dispatch] = useReducer(reducer, INITIAL);
  const stampsRef = useRef<number[]>([]);

  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    const handleEvent = (event: ConnectionEvent) => {
      if (!mounted) return;
      switch (event.type) {
        case "frame": {
          const now = performance.now();
          const stamps = stampsRef.current;
          stamps.push(now);
          while (stamps.length && now - stamps[0] > 1000) stamps.shift();
          dispatch({ type: "frame", frame: event.frame });
          dispatch({ type: "tick", fps: stamps.length });
          break;
        }
        case "error":
          dispatch({ type: "error", message: event.message });
          break;
        case "disconnected":
          dispatch({ type: "disconnected" });
          break;
        case "connected":
          dispatch({ type: "connected", source: event.source });
          break;
      }
    };

    listenConnectionEvents(handleEvent).then((u) => {
      unlisten = u;
    });

    currentSource().then((src) => {
      if (mounted && src) dispatch({ type: "connected", source: src });
    });

    let stopSim: (() => void) | undefined;
    if (!IS_TAURI && import.meta.env.DEV) {
      stopSim = startDevSimulator(handleEvent);
    }

    return () => {
      mounted = false;
      unlisten?.();
      stopSim?.();
    };
  }, []);

  const connect = useCallback(async (port: string, baud: number) => {
    dispatch({ type: "connecting" });
    try {
      await connectSerial(port, baud);
    } catch (err) {
      dispatch({ type: "error", message: String(err) });
    }
  }, []);

  const startDemoMode = useCallback(async (path: string, speed: number) => {
    dispatch({ type: "connecting" });
    try {
      await startDemo(path, speed);
    } catch (err) {
      dispatch({ type: "error", message: String(err) });
    }
  }, []);

  const disconnect = useCallback(async () => {
    await disconnectCmd();
    dispatch({ type: "disconnected" });
  }, []);

  const reset = useCallback(async () => {
    try {
      await sendReset();
    } catch (err) {
      dispatch({ type: "error", message: String(err) });
    }
  }, []);

  const clearError = useCallback(() => dispatch({ type: "clearError" }), []);
  const clearHistory = useCallback(() => dispatch({ type: "clearHistory" }), []);

  const adjustCorrection = useCallback((lane: number, deltaMs: number) => {
    dispatch({ type: "adjustCorrection", lane, deltaMs });
  }, []);

  const clearCorrection = useCallback((lane: number) => {
    dispatch({ type: "clearCorrection", lane });
  }, []);

  return {
    state,
    connect,
    startDemoMode,
    disconnect,
    reset,
    clearError,
    clearHistory,
    adjustCorrection,
    clearCorrection,
  };
}
