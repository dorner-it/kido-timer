import type {
  DisciplinePayload,
  Penalty,
  PenaltyKind,
  PenaltyStatus,
  Run,
} from "../cloudTypes";

export const RUN_A1 = "run-a1";
export const RUN_B1 = "run-b1";
export const RUN_C1 = "run-c1";

export function run(partial: Partial<Run> & Pick<Run, "id" | "team_id" | "run_number">): Run {
  return {
    event_id: "ev",
    discipline_id: "disc",
    runner_id: null,
    lane: null,
    status: "pending",
    original_time_ms: null,
    correction_ms: 0,
    started_at: null,
    ended_at: null,
    lineup: [],
    lineup_confirmed_at: null,
    lineup_confirmed_by: null,
    time_source: "lichtschranke",
    hand_time_1_ms: null,
    hand_time_2_ms: null,
    ...partial,
  };
}

export function penalty(
  id: string,
  kind: PenaltyKind,
  status: PenaltyStatus,
  extra: Partial<Penalty> = {},
): Penalty {
  return {
    id,
    event_id: "ev",
    discipline_id: "disc",
    run_id: RUN_A1,
    team_id: "team-a",
    runner_id: null,
    kind,
    code: null,
    station: null,
    is_disqualification: kind === "fv",
    decider_slot: 0,
    delta_ms: kind === "sz" ? 15000 : 0,
    reason_code: null,
    status,
    created_at: `2027-06-12T10:00:0${id.length % 10}Z`,
    reviewed_at: null,
    reviewed_by: null,
    ...extra,
  };
}

/** Gruppenstafette, 2 lanes; A1 lane 1 + B1 lane 2 current; C1 without lane. */
export function samplePayload(): DisciplinePayload {
  return {
    schema_version: 4,
    exported_at: "2027-06-12T09:00:00Z",
    owner_sub: "sub-1",
    event: {
      id: "ev",
      owner_sub: "sub-1",
      name: "Kreisausscheid",
      date: "2027-06-12",
      location: null,
      is_active: true,
      current_discipline_id: "disc",
      current_run_id: RUN_A1,
      current_run_ids: [RUN_A1, RUN_B1],
    },
    discipline: {
      id: "disc",
      event_id: "ev",
      name: "Gruppenstafette",
      mode: "gruppenstaffette",
      lane_count: 2,
      judge_mode: "per_station",
      sync_mode: "offline",
      position: 1,
    },
    teams: [
      { id: "team-a", event_id: "ev", name: "FF Alpha", position: 1 },
      { id: "team-b", event_id: "ev", name: "FF Beta", position: 2 },
      { id: "team-c", event_id: "ev", name: "FF Gamma", position: 3 },
    ],
    runners: [],
    team_entries: [
      { team_id: "team-a", discipline_id: "disc", event_id: "ev", start_number: 1, lane: 1, wg: "wg2", wg_history: [] },
      { team_id: "team-b", discipline_id: "disc", event_id: "ev", start_number: 2, lane: 2, wg: "wg2", wg_history: [] },
      { team_id: "team-c", discipline_id: "disc", event_id: "ev", start_number: 3, lane: null, wg: null, wg_history: [] },
    ],
    runs: [
      run({ id: RUN_A1, team_id: "team-a", run_number: 1, lane: 1 }),
      run({ id: RUN_B1, team_id: "team-b", run_number: 2, lane: 2 }),
      run({ id: RUN_C1, team_id: "team-c", run_number: 3, lane: null }),
    ],
    approved_penalties: [],
  };
}
