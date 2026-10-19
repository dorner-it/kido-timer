// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// Smoke tests: the organizer review components render (SSR, no effects)
// and show the key states. Written without JSX so the `*.test.ts`
// include pattern picks them up.

import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { Discipline, Event, EventDetail, Penalty, Run, Runner, Team, TeamEntry } from '../../../lib/api';
import { indexCatalogue } from '../../../lib/penalties';
import CurrentRunsControl from '../CurrentRunsControl';
import RunReviewCard from '../RunReviewCard';
import RunsPanel from '../RunsPanel';
import { RunTime } from '../RunTime';

const ts = '2027-05-01T08:42:00Z';
const team: Team = { id: 't1', event_id: 'e1', name: 'FF Musterdorf', position: 1, created_at: ts, updated_at: ts };
const entry: TeamEntry = {
  team_id: 't1',
  discipline_id: 'd1',
  event_id: 'e1',
  start_number: 12,
  lane: 1,
  wg: 'wg2',
  wg_history: [],
  wg_warnings: [],
  created_at: ts,
  updated_at: ts,
};
const runner: Runner = {
  id: 'r1',
  team_id: 't1',
  event_id: 'e1',
  first_name: 'Max',
  last_name: 'Muster',
  birth_date: '2014-03-01',
  position: 1,
  created_at: ts,
  updated_at: ts,
};
const run: Run = {
  id: 'run1',
  event_id: 'e1',
  discipline_id: 'd1',
  team_id: 't1',
  runner_id: null,
  run_number: 3,
  lane: 1,
  status: 'completed',
  original_time_ms: 47_240,
  correction_ms: 0,
  started_at: null,
  ended_at: null,
  lineup: [],
  lineup_confirmed_at: null,
  lineup_confirmed_by: null,
  time_source: 'handstopp',
  hand_time_1_ms: 47_230,
  hand_time_2_ms: 47_245,
  created_at: ts,
  updated_at: ts,
};
function pen(id: string, kind: 'sz' | 'fv', status: Penalty['status'], code: string): Penalty {
  return {
    id,
    event_id: 'e1',
    discipline_id: 'd1',
    run_id: 'run1',
    team_id: 't1',
    runner_id: null,
    station: code.startsWith('knoten') ? 'knotengestell' : 'startlinie',
    kind,
    code,
    is_disqualification: kind === 'fv',
    decider_slot: 0,
    delta_ms: kind === 'sz' ? 15_000 : 0,
    reason_code: null,
    status,
    created_at: ts,
    reviewed_at: null,
    reviewed_by: null,
  };
}
const catalogue = indexCatalogue([
  { code: 'knotengestell.mehrere_knoten_falsch', station: 'knotengestell', kind: 'fv', label_de: 'Mehrere Knoten falsch' },
  { code: 'startlinie.fruehstart', station: 'startlinie', kind: 'sz', label_de: 'Ein WK startet vor dem Abschlagen' },
]);
const discipline: Discipline = {
  id: 'd1',
  event_id: 'e1',
  name: 'Gruppenstafette',
  mode: 'gruppenstaffette',
  sync_mode: 'live',
  position: 1,
  lane_count: 2,
  judge_mode: 'per_station',
  created_at: ts,
  updated_at: ts,
};
const event: Event = {
  id: 'e1',
  owner_sub: 'o',
  name: 'KJF Tag',
  date: '2027-05-01',
  location: null,
  is_active: true,
  current_discipline_id: 'd1',
  current_run_id: 'run1',
  current_run_ids: ['run1'],
  decider_tokens: ['1', '2', '3', '4'],
  station_token: 'x',
  created_at: ts,
  updated_at: ts,
};
const noop = () => {};

function card(penalties: Penalty[], r: Run = run) {
  return renderToStaticMarkup(
    h(RunReviewCard, {
      eventId: 'e1',
      disciplineId: 'd1',
      run: r,
      team,
      entry,
      roster: [runner],
      penalties,
      catalogue,
      laneLabel: 'Bahn 1',
      onPenaltyUpdated: noop,
      onRunUpdated: noop,
      onTakenOver: noop,
      onError: noop,
    }),
  );
}

describe('RunReviewCard', () => {
  it('pending errors get Bestätigen / Zurücknehmen, summary is provisional', () => {
    const html = card([pen('a', 'sz', 'pending', 'startlinie.fruehstart')]);
    expect(html).toContain('Bestätigen');
    expect(html).toContain('Zurücknehmen');
    expect(html).toContain('1 SZ (+15 s) · gültig · vorläufig');
    expect(html).toContain('Übernehmen (1)');
    expect(html).toContain('Startlinie/Zeitnehmer');
    expect(html).toContain('Ein WK startet vor dem Abschlagen');
  });

  it('withdrawn errors are crossed out; FV makes the run invalid', () => {
    const html = card([
      pen('a', 'sz', 'rejected', 'startlinie.fruehstart'),
      pen('b', 'fv', 'approved', 'knotengestell.mehrere_knoten_falsch'),
    ]);
    expect(html).toContain('line-through');
    expect(html).toContain('Zurückgenommen');
    expect(html).toContain('FV · ungültig');
    expect(html).toContain('Mehrere Knoten falsch');
    expect(html).not.toContain('>Bestätigen<');
  });

  it('shows Handzeit with both raw times and the lineup warning', () => {
    const html = card([]);
    expect(html).toContain('Handzeit');
    expect(html).toContain('47,23 / 47,245');
    expect(html).toContain('Besetzung fehlt — Tagessieger nutzt dann den ganzen Kader');
    expect(html).toContain('Startnr. 12');
    expect(html).toContain('WG2');
    expect(html).toContain('fehlerfrei · gültig');
  });

  it('confirmed lineup shows who and when', () => {
    const html = card([], {
      ...run,
      lineup: ['r1'],
      lineup_confirmed_by: 'startlinie',
      lineup_confirmed_at: ts,
    });
    expect(html).toMatch(/1 Läufer · Startlinie, \d{2}:\d{2}/);
    expect(html).not.toContain('Besetzung fehlt');
  });
});

describe('RunTime', () => {
  it('light barrier: no badge', () => {
    const html = renderToStaticMarkup(
      h(RunTime, { run: { ...run, time_source: 'lichtschranke', hand_time_1_ms: null, hand_time_2_ms: null } }),
    );
    expect(html).toContain('47,24');
    expect(html).not.toContain('Handzeit');
  });
});

describe('CurrentRunsControl', () => {
  it('one slot per lane with team, start number and WG', () => {
    const html = renderToStaticMarkup(
      h(CurrentRunsControl, {
        event,
        discipline,
        runs: [run],
        entries: [entry],
        teams: [team],
        onSaved: noop,
        onError: noop,
      }),
    );
    expect(html).toContain('Bahn 1');
    expect(html).toContain('Bahn 2');
    expect(html).toContain('Bahn frei');
    expect(html).toContain('FF Musterdorf');
    expect(html).toContain('Startnr. 12');
    expect(html).toContain('läuft');
  });
});

describe('RunsPanel', () => {
  it('renders the run list and the current run card', () => {
    const data: EventDetail = {
      event,
      disciplines: [discipline],
      teams: [team],
      runners: [runner],
      team_entries: [entry],
      runs: [run],
    };
    const html = renderToStaticMarkup(h(RunsPanel, { data, onChange: noop }));
    expect(html).toContain('Laufende Läufe');
    expect(html).toContain('Alle Läufe (1)');
    expect(html).toContain('Durchgang anlegen (1)');
    expect(html).toContain('Handzeit');
  });

  it('inactive discipline offers "Aktiv schalten"', () => {
    const data: EventDetail = {
      event: { ...event, current_discipline_id: null, current_run_ids: [] },
      disciplines: [discipline],
      teams: [team],
      runners: [],
      team_entries: [],
      runs: [],
    };
    const html = renderToStaticMarkup(h(RunsPanel, { data, onChange: noop }));
    expect(html).toContain('Aktiv schalten');
    expect(html).not.toContain('Auf die Bahn');
  });
});
