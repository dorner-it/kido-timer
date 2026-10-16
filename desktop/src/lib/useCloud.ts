import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  cloudClear,
  cloudClearFailedPost,
  cloudDeselect,
  cloudExportKido,
  cloudFailedPosts,
  cloudIdentity,
  cloudListDisciplines,
  cloudOpenKido,
  cloudPair,
  cloudPostRunStatus,
  cloudRetryPost,
  cloudSelectDiscipline,
  cloudSnapshot,
  listenCloudEvents,
  pickKidoFile,
  type PostRunArgs,
} from "./cloudApi";
import type {
  CloudEvent,
  CloudIdentity,
  DisciplineListItem,
  DisciplinePayload,
  FailedPost,
  KidoConflict,
  OpenKidoResult,
  Run,
} from "./cloudTypes";

interface State {
  identity: CloudIdentity | null;
  snapshot: DisciplinePayload | null;
  disciplines: DisciplineListItem[];
  loading: boolean;
  error: string | null;
  lastResultMessage: string | null;
  failedPosts: FailedPost[];
  /** Operator overrides — desktop lane (1..4) → run_id (UUID). */
  laneOverrides: Record<number, string>;
}

type Action =
  | { type: "setIdentity"; identity: CloudIdentity | null }
  | { type: "setDisciplines"; list: DisciplineListItem[] }
  | { type: "setSnapshot"; snapshot: DisciplinePayload | null }
  | { type: "setLoading"; loading: boolean }
  | { type: "setError"; error: string | null }
  | { type: "setResultMessage"; message: string | null }
  | { type: "setFailedPosts"; failures: FailedPost[] }
  | { type: "setLaneOverride"; lane: number; runId: string | null }
  | { type: "clearLaneOverrides" }
  | { type: "cleared" };

const INITIAL: State = {
  identity: null,
  snapshot: null,
  disciplines: [],
  loading: false,
  error: null,
  lastResultMessage: null,
  failedPosts: [],
  laneOverrides: {},
};

function reducer(s: State, a: Action): State {
  switch (a.type) {
    case "setIdentity":
      return { ...s, identity: a.identity };
    case "setDisciplines":
      return { ...s, disciplines: a.list };
    case "setSnapshot": {
      // If the underlying discipline changed, drop lane overrides.
      const sameDisc =
        s.snapshot && a.snapshot
          ? s.snapshot.discipline.id === a.snapshot.discipline.id
          : !s.snapshot && !a.snapshot;
      return {
        ...s,
        snapshot: a.snapshot,
        laneOverrides: sameDisc ? s.laneOverrides : {},
      };
    }
    case "setLoading":
      return { ...s, loading: a.loading };
    case "setError":
      return { ...s, error: a.error };
    case "setResultMessage":
      return { ...s, lastResultMessage: a.message };
    case "setFailedPosts":
      return { ...s, failedPosts: a.failures };
    case "setLaneOverride": {
      const next = { ...s.laneOverrides };
      if (a.runId) next[a.lane] = a.runId;
      else delete next[a.lane];
      return { ...s, laneOverrides: next };
    }
    case "clearLaneOverrides":
      return { ...s, laneOverrides: {} };
    case "cleared":
      return { ...INITIAL };
  }
}

export function useCloud() {
  const [state, dispatch] = useReducer(reducer, INITIAL);
  const snapshotRef = useRef<DisciplinePayload | null>(null);
  const overridesRef = useRef<Record<number, string>>({});

  useEffect(() => {
    snapshotRef.current = state.snapshot;
    overridesRef.current = state.laneOverrides;
  }, [state.snapshot, state.laneOverrides]);

  useEffect(() => {
    let mounted = true;
    let unlisten: (() => void) | undefined;

    cloudIdentity().then((id) => {
      if (mounted) dispatch({ type: "setIdentity", identity: id });
    });
    cloudSnapshot().then((s) => {
      if (mounted) dispatch({ type: "setSnapshot", snapshot: s });
    });
    cloudFailedPosts().then((f) => {
      if (mounted) dispatch({ type: "setFailedPosts", failures: f });
    });

    listenCloudEvents((event: CloudEvent) => {
      if (!mounted) return;
      switch (event.type) {
        case "paired":
          dispatch({ type: "setIdentity", identity: event.identity });
          break;
        case "cleared":
          dispatch({ type: "cleared" });
          break;
        case "snapshotChanged":
          dispatch({ type: "setSnapshot", snapshot: event.snapshot });
          dispatch({ type: "setError", error: null });
          break;
        case "snapshotError":
          dispatch({ type: "setError", error: event.message });
          break;
        case "resultPosted":
          dispatch({
            type: "setResultMessage",
            message: `Ergebnis übertragen (${event.status})`,
          });
          break;
        case "resultPostFailed":
          dispatch({
            type: "setResultMessage",
            message: `Übertragung fehlgeschlagen: ${event.message}`,
          });
          break;
        case "failedPostsChanged":
          dispatch({ type: "setFailedPosts", failures: event.failures });
          break;
      }
    }).then((u) => {
      unlisten = u;
    });

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const pair = useCallback(async (baseUrl: string, apiKey: string) => {
    dispatch({ type: "setLoading", loading: true });
    dispatch({ type: "setError", error: null });
    try {
      const identity = await cloudPair(baseUrl, apiKey);
      dispatch({ type: "setIdentity", identity });
      return identity;
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
      throw e;
    } finally {
      dispatch({ type: "setLoading", loading: false });
    }
  }, []);

  const clear = useCallback(async () => {
    await cloudClear();
    dispatch({ type: "cleared" });
  }, []);

  const refreshDisciplines = useCallback(async () => {
    dispatch({ type: "setLoading", loading: true });
    dispatch({ type: "setError", error: null });
    try {
      const list = await cloudListDisciplines();
      dispatch({ type: "setDisciplines", list });
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
    } finally {
      dispatch({ type: "setLoading", loading: false });
    }
  }, []);

  const selectDiscipline = useCallback(async (id: string) => {
    dispatch({ type: "setLoading", loading: true });
    dispatch({ type: "setError", error: null });
    try {
      const snapshot = await cloudSelectDiscipline(id);
      dispatch({ type: "setSnapshot", snapshot });
      return snapshot;
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
      throw e;
    } finally {
      dispatch({ type: "setLoading", loading: false });
    }
  }, []);

  const deselect = useCallback(async () => {
    await cloudDeselect();
    dispatch({ type: "setSnapshot", snapshot: null });
  }, []);

  const openKido = useCallback(async (): Promise<
    { path: string; result: OpenKidoResult } | null
  > => {
    const path = await pickKidoFile();
    if (!path) return null;
    dispatch({ type: "setLoading", loading: true });
    dispatch({ type: "setError", error: null });
    try {
      const result = await cloudOpenKido(path, false);
      if (result.adopted) {
        dispatch({ type: "setSnapshot", snapshot: result.payload });
      }
      return { path, result };
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
      throw e;
    } finally {
      dispatch({ type: "setLoading", loading: false });
    }
  }, []);

  const openKidoForce = useCallback(
    async (path: string): Promise<OpenKidoResult> => {
      const result = await cloudOpenKido(path, true);
      if (result.adopted) {
        dispatch({ type: "setSnapshot", snapshot: result.payload });
      }
      return result;
    },
    [],
  );

  const exportKido = useCallback(async () => {
    if (!state.snapshot) return null;
    const date = state.snapshot.event.date;
    const eventSlug = state.snapshot.event.name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    const discSlug = state.snapshot.discipline.name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return cloudExportKido(`kido-${eventSlug || "export"}-${discSlug || "wettkampf"}-${date}.kido`);
  }, [state.snapshot]);

  /**
   * POST a run state change. Resolves the matching run_id from the current
   * lane via overrides → active-on-lane → current_run_id. No-op if no run
   * matches or no discipline is loaded.
   */
  const postLaneStatus = useCallback(
    async (
      lane: number,
      patch: Pick<PostRunArgs, "status" | "originalTimeMs" | "startedAt" | "endedAt">,
    ): Promise<void> => {
      const snapshot = snapshotRef.current;
      if (!snapshot) return;
      if (snapshot.discipline.sync_mode !== "live") return;
      const overrides = overridesRef.current;
      const run = pickRunForLane(snapshot, lane, overrides);
      if (!run) return;
      try {
        await cloudPostRunStatus({
          disciplineId: snapshot.discipline.id,
          runId: run.id,
          runNumber: run.run_number,
          ...patch,
        });
      } catch {
        /* events surface failure */
      }
    },
    [],
  );

  const setLaneOverride = useCallback(
    (lane: number, runId: string | null) => {
      dispatch({ type: "setLaneOverride", lane, runId });
    },
    [],
  );

  const retryFailedPost = useCallback(async (runId: string) => {
    try {
      await cloudRetryPost(runId);
    } catch {
      /* event surfaces */
    }
  }, []);

  const dismissFailedPost = useCallback(async (runId: string) => {
    await cloudClearFailedPost(runId);
  }, []);

  const dismissResultMessage = useCallback(() => {
    dispatch({ type: "setResultMessage", message: null });
  }, []);

  return {
    state,
    pair,
    clear,
    refreshDisciplines,
    selectDiscipline,
    deselect,
    openKido,
    openKidoForce,
    exportKido,
    postLaneStatus,
    setLaneOverride,
    retryFailedPost,
    dismissFailedPost,
    dismissResultMessage,
  };
}

export type KidoConflictInfo = KidoConflict;

/**
 * Pick the run that should receive a confirmed time on the given desktop lane.
 * Order: explicit operator override → active run on matching lane →
 * snapshot.event.current_run_id (if active and lane-compatible).
 */
export function pickRunForLane(
  snapshot: DisciplinePayload | null,
  lane: number,
  overrides: Record<number, string> = {},
): Run | null {
  if (!snapshot) return null;
  const overrideId = overrides[lane];
  if (overrideId) {
    const r = snapshot.runs.find((x) => x.id === overrideId);
    if (r) return r;
  }
  const active = snapshot.runs.find(
    (r) => r.status === "active" && r.lane === lane,
  );
  if (active) return active;
  if (snapshot.event.current_run_id) {
    const r = snapshot.runs.find(
      (x) => x.id === snapshot.event.current_run_id && x.status === "active",
    );
    if (r && (r.lane === null || r.lane === lane)) return r;
  }
  return null;
}

export function teamForRun(
  snapshot: DisciplinePayload | null,
  runId: string | null | undefined,
) {
  if (!snapshot || !runId) return null;
  const run = snapshot.runs.find((r) => r.id === runId);
  if (!run) return null;
  const team = snapshot.teams.find((t) => t.id === run.team_id) ?? null;
  return { run, team };
}

export function runnerForRun(
  snapshot: DisciplinePayload | null,
  runId: string | null | undefined,
) {
  if (!snapshot || !runId) return null;
  const run = snapshot.runs.find((r) => r.id === runId);
  if (!run || !run.runner_id) return null;
  return snapshot.runners.find((r) => r.id === run.runner_id) ?? null;
}
