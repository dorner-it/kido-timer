import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import {
  cloudClear,
  cloudClearFailedPost,
  cloudDeselect,
  cloudDivergences,
  cloudExportKido,
  cloudFailedPosts,
  cloudIdentity,
  cloudListDisciplines,
  cloudOpenKido,
  cloudPair,
  cloudRecordLaneStatus,
  cloudResolveDivergence,
  cloudRetryPost,
  cloudSelectDiscipline,
  cloudSetSyncMode,
  cloudSnapshot,
  cloudTakeStartupWarning,
  kidoCatalogue,
  listenCloudEvents,
  pickKidoFile,
  type RecordLaneArgs,
} from "./cloudApi";
import type {
  CatalogueItem,
  CloudEvent,
  CloudIdentity,
  DisciplineListItem,
  DisciplinePayload,
  FailedPost,
  KidoConflict,
  OpenKidoResult,
  RunDivergence,
  SyncMode,
} from "./cloudTypes";
import {
  loadSyncOverrides,
  saveSyncOverrides,
  snapshotSyncMode,
  withOverride,
  type SyncOverrides,
} from "./syncMode";

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
  /** Local results that differ from the web (live mode blocked). */
  divergences: RunDivergence[];
  /** Desktop sync-mode overrides per discipline id. */
  syncOverrides: SyncOverrides;
  catalogue: CatalogueItem[];
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
  | { type: "setDivergences"; divergences: RunDivergence[] }
  | { type: "setSyncOverrides"; overrides: SyncOverrides }
  | { type: "setCatalogue"; catalogue: CatalogueItem[] }
  | { type: "cleared" };

function initialState(): State {
  return {
    identity: null,
    snapshot: null,
    disciplines: [],
    loading: false,
    error: null,
    lastResultMessage: null,
    failedPosts: [],
    laneOverrides: {},
    divergences: [],
    syncOverrides: loadSyncOverrides(),
    catalogue: [],
  };
}

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
    case "setDivergences":
      return { ...s, divergences: a.divergences };
    case "setSyncOverrides":
      return { ...s, syncOverrides: a.overrides };
    case "setCatalogue":
      return { ...s, catalogue: a.catalogue };
    case "cleared":
      // Unpairing keeps the loaded discipline (local results stay); Rust
      // re-emits the snapshot right after.
      return {
        ...initialState(),
        snapshot: s.snapshot,
        laneOverrides: s.laneOverrides,
        catalogue: s.catalogue,
        divergences: [],
      };
  }
}

export function useCloud() {
  const [state, dispatch] = useReducer(reducer, undefined, initialState);
  const overridesRef = useRef<Record<number, string>>({});
  const syncOverridesRef = useRef<SyncOverrides>(state.syncOverrides);

  useEffect(() => {
    overridesRef.current = state.laneOverrides;
    syncOverridesRef.current = state.syncOverrides;
  }, [state.laneOverrides, state.syncOverrides]);

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
    cloudDivergences().then((d) => {
      if (mounted) dispatch({ type: "setDivergences", divergences: d });
    });
    cloudTakeStartupWarning().then((w) => {
      if (mounted && w) dispatch({ type: "setError", error: w });
    });
    kidoCatalogue().then((c) => {
      if (mounted) dispatch({ type: "setCatalogue", catalogue: c });
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
        case "divergenceChanged":
          dispatch({ type: "setDivergences", divergences: event.divergences });
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

  const effectiveMode = useMemo(
    () => snapshotSyncMode(state.snapshot, state.syncOverrides),
    [state.snapshot, state.syncOverrides],
  );

  // Tell the Rust side whenever the loaded discipline or its effective
  // mode changes (also covers the restore after an app restart).
  const disciplineId = state.snapshot?.discipline.id ?? null;
  useEffect(() => {
    if (!disciplineId || !effectiveMode) return;
    void cloudSetSyncMode(effectiveMode);
  }, [disciplineId, effectiveMode]);

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
      // `null` → Rust uses the web default.
      const mode = syncOverridesRef.current[id] ?? null;
      const snapshot = await cloudSelectDiscipline(id, mode);
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
    dispatch({ type: "setDivergences", divergences: [] });
  }, []);

  const adoptOpened = useCallback((result: OpenKidoResult) => {
    if (!result.adopted) return;
    dispatch({ type: "setSnapshot", snapshot: result.payload });
    // Rust opens files as on-prem; apply the effective mode explicitly
    // (the effect above does not fire when the discipline id is unchanged).
    const mode = snapshotSyncMode(result.payload, syncOverridesRef.current);
    if (mode) void cloudSetSyncMode(mode);
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
      adoptOpened(result);
      return { path, result };
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
      throw e;
    } finally {
      dispatch({ type: "setLoading", loading: false });
    }
  }, [adoptOpened]);

  const openKidoForce = useCallback(
    async (path: string): Promise<OpenKidoResult> => {
      const result = await cloudOpenKido(path, true);
      adoptOpened(result);
      return result;
    },
    [adoptOpened],
  );

  const exportKido = useCallback(async () => {
    if (!state.snapshot) return null;
    const date = state.snapshot.event.date;
    const eventSlug = slug(state.snapshot.event.name);
    const discSlug = slug(state.snapshot.discipline.name);
    return cloudExportKido(
      `kido-${eventSlug || "export"}-${discSlug || "wettkampf"}-${date}.kido`,
    );
  }, [state.snapshot]);

  /**
   * Record a lane timing change. The Rust side resolves the run (current
   * runs → overrides → active run), stores it locally (persisted) and
   * posts it when live. No-op without a loaded discipline.
   */
  const recordLaneStatus = useCallback(
    async (
      lane: number,
      patch: Pick<RecordLaneArgs, "status" | "originalTimeMs" | "startedAt" | "endedAt">,
    ): Promise<void> => {
      try {
        await cloudRecordLaneStatus({
          lane,
          overrides: overridesRef.current,
          ...patch,
        });
      } catch (e) {
        dispatch({ type: "setError", error: String(e) });
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

  /** Override (or reset with `null`) the sync mode of a discipline. */
  const setSyncOverride = useCallback(
    (disciplineId: string, webDefault: SyncMode, mode: SyncMode | null) => {
      const next = withOverride(
        syncOverridesRef.current,
        disciplineId,
        mode,
        webDefault,
      );
      syncOverridesRef.current = next;
      saveSyncOverrides(next);
      dispatch({ type: "setSyncOverrides", overrides: next });
    },
    [],
  );

  const resolveDivergence = useCallback(async (upload: boolean) => {
    try {
      await cloudResolveDivergence(upload);
    } catch (e) {
      dispatch({ type: "setError", error: String(e) });
    }
  }, []);

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

  const dismissError = useCallback(() => {
    dispatch({ type: "setError", error: null });
  }, []);

  return {
    state,
    effectiveMode,
    pair,
    clear,
    refreshDisciplines,
    selectDiscipline,
    deselect,
    openKido,
    openKidoForce,
    exportKido,
    recordLaneStatus,
    setLaneOverride,
    setSyncOverride,
    resolveDivergence,
    retryFailedPost,
    dismissFailedPost,
    dismissResultMessage,
    dismissError,
  };
}

export type KidoConflictInfo = KidoConflict;

function slug(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
