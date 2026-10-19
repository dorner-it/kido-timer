// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { STATION_KIND_LABELS, type StationKind } from '../../../lib/stations';
import { Brandmark } from '../Brandmark';
import { BTN_CANCEL, BTN_NEUTRAL, CARD } from './ui';

/**
 * Judge "login": step 1 station, step 2 (only `per_station_and_lane`
 * with 2 lanes) the judge's lane. The parent persists the choice.
 */
export interface JudgeLoginProps {
  step: 'station' | 'lane';
  eventName: string | null;
  disciplineName: string | null;
  allowed: StationKind[];
  station: StationKind | null;
  lanes: number[];
  onStation: (s: StationKind) => void;
  onLane: (lane: number) => void;
  onBack: () => void;
}

export default function JudgeLogin(props: JudgeLoginProps) {
  const { step, eventName, disciplineName, allowed, station, lanes } = props;
  return (
    <div className="mx-auto flex min-h-screen w-full max-w-lg flex-col px-4 py-6">
      <div className="mb-5 flex items-center gap-3">
        <Brandmark size="sm" />
        <div className="flex flex-col leading-tight">
          <span className="text-[13px] font-semibold uppercase tracking-wide text-ink-200">
            Strafrichter
          </span>
          <span className="text-[17px] font-semibold text-ink-50">{eventName ?? '—'}</span>
          {disciplineName && <span className="text-[14px] text-ink-200">{disciplineName}</span>}
        </div>
      </div>

      <div className={`${CARD} p-5`}>
        {step === 'station' ? (
          <>
            <h1 className="text-[22px] font-bold text-ink-50">Deine Station</h1>
            <p className="mt-1 text-[15px] text-ink-100">
              Wähle den Posten, an dem du stehst. Die Auswahl bleibt auf diesem Gerät
              gespeichert und kann über „Station/Bahn wechseln" geändert werden.
            </p>
            <div className="mt-5 flex flex-col gap-3">
              {allowed.map((s) => (
                <button key={s} type="button" className={BTN_NEUTRAL} onClick={() => props.onStation(s)}>
                  <span className="text-[19px]">{STATION_KIND_LABELS[s]}</span>
                  {s === 'startlinie' && (
                    <span className="ml-auto text-[13px] font-normal text-ink-200">
                      + Besetzung
                    </span>
                  )}
                </button>
              ))}
            </div>
            {allowed.length === 0 && (
              <p className="mt-5 text-[15px] font-semibold text-amber-800" role="status">
                Aktuell ist kein Wettkampf auf der Bahn. Sobald die Wettkampfleitung einen
                Wettkampf startet, erscheinen hier die Stationen.
              </p>
            )}
          </>
        ) : (
          <>
            <h1 className="text-[22px] font-bold text-ink-50">Deine Bahn</h1>
            <p className="mt-1 text-[15px] text-ink-100">
              {station ? STATION_KIND_LABELS[station] : ''}: an dieser Station gibt es je Bahn
              einen Strafrichter. Du siehst nur die Mannschaft auf deiner Bahn.
            </p>
            <div className="mt-5 grid grid-cols-2 gap-3">
              {lanes.map((lane) => (
                <button
                  key={lane}
                  type="button"
                  className={`${BTN_NEUTRAL} min-h-[96px] justify-center text-[24px]`}
                  onClick={() => props.onLane(lane)}
                >
                  Bahn {lane}
                </button>
              ))}
            </div>
            <div className="mt-4">
              <button type="button" className={BTN_CANCEL} onClick={props.onBack}>
                Andere Station
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
