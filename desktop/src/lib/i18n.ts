import type { ChannelStatus, DeviceMode, StateFlag } from "./types";

/** Central German UI strings. All visible text in the app comes from here. */
export const t = {
  app: {
    brand: "KiDo·Timer",
    subtitle: "Sportzeit-Konsole",
    title: "KiDo-Timer Sportzeitmessung",
  },

  topbar: {
    menu: "Menü öffnen",
    closeMenu: "Menü schließen",
    localTime: "Ortszeit",
    fps: "Bilder/s",
    frames: "Bilder",
    mode: "Modus",
    lane: "Bahn",
    state: "Status",
    reset: "Reset",
    resetTitle: "Gerät zurücksetzen",
    resetConfirm: "Wirklich alle Bahnen am Gerät zurücksetzen?",
    resetConfirmAction: "Bestätigen",
    resetCancel: "Abbrechen",
    confirmRun: "Lauf übernehmen",
    confirmRunHint: "Bestätigte Bahnen übernehmen (lokal gesichert, live auch ans Web)",
    confirmRunNone: "Keine Bahn wartet auf Übernahme",
    confirmRunCount: (n: number) =>
      n === 1 ? "1 Bahn bereit" : `${n} Bahnen bereit`,
  },

  confirmModal: {
    title: "Lauf übernehmen",
    subtitle: "Diese Zeiten werden an den Wettkampf gemeldet:",
    laneShort: "Bahn",
    cancel: "Abbrechen",
    confirm: "Zeiten übernehmen",
    noEntries: "Keine Bahn wartet auf Übernahme.",
    resetTitle: "Gerät zurücksetzen?",
    resetBody:
      "Zeiten übernommen. Sollen die Bahnen am Gerät jetzt zurückgesetzt werden, damit der nächste Lauf starten kann?",
    resetSkip: "Nicht jetzt",
    resetAccept: "Ja, Gerät zurücksetzen",
  },

  conn: {
    statusIdle: "Bereit",
    statusConnecting: "Verbinde…",
    statusConnected: "Live",
    statusError: "Störung",
    noSource: "Keine Quelle",
  },

  setup: {
    welcomeTitle: "Willkommen",
    welcomeBody:
      "Wähle den Anschluss zu deiner KiDo-Timer Sportzeituhr, starte den Demo-Modus mit einer aufgezeichneten Datei oder koppele die App mit deinem Online-Konto.",
    tabSerial: "Sportzeituhr",
    tabDemo: "Demo-Wiedergabe",
    tabCloud: "Online-Konto",
    serialPort: "Anschluss",
    baudRate: "Baudrate",
    rescan: "Neu suchen",
    scanning: "Suche …",
    noPorts: "Kein Anschluss erkannt",
    rememberLabel: "Beim nächsten Start automatisch verbinden",
    saveAndConnect: "Speichern & Verbinden",
    connect: "Verbinden",
    pickFile: "Aufnahme-Datei auswählen (.txt)",
    speed: "Geschwindigkeit",
    speedMax: "max",
    startDemo: "Demo starten",
    cancel: "Abbrechen",
    settings: "Einstellungen",
    cloudPaired: "Konto bereits gekoppelt — Verbindung kann jederzeit über das Menü geändert werden.",
    cloudFinish: "Fertigstellen",
  },

  drawer: {
    title: "Steuerung",
    sectionConnection: "Verbindung",
    sectionDevice: "Gerät",
    sectionView: "Ansicht",
    sectionAppearance: "Darstellung",
    sectionCloud: "Online-Verbindung",
    themeLabel: "Erscheinungsbild",
    themeLight: "Hell",
    themeDark: "Dunkel",
    source: "Quelle",
    notConnected: "Nicht verbunden",
    edit: "Verbindung ändern",
    disconnect: "Trennen",
    reset: "Gerät zurücksetzen",
    resetConfirm: "Wirklich alle Bahnen am Gerät zurücksetzen?",
    resetConfirmAction: "Zurücksetzen bestätigen",
    resetCommand: "Sendet RST\\r über die serielle Verbindung.",
    openProtocol: "Lauf-Protokoll öffnen",
    backToLanes: "Zurück zur Bahn-Ansicht",
  },

  cloud: {
    title: "Online-Verbindung",
    subtitle: "Mit ki-do.it koppeln",
    notPaired: "Noch nicht gekoppelt",
    pairedAs: "Angemeldet als",
    keyId: "Schlüssel",
    server: "Server",
    pair: "Kopplung einrichten",
    rePair: "Schlüssel ersetzen",
    unpair: "Kopplung aufheben",
    unpairConfirm:
      "Wirklich vom Online-Konto trennen? Schlüssel werden aus dem Schlüsselbund entfernt.",
    unpairAction: "Trennen bestätigen",
    pairTitle: "Mit ki-do.it koppeln",
    pairBody:
      "Erstelle im Web unter Einstellungen einen neuen Schlüssel und füge den Klartext hier ein. Der Schlüssel wird im Schlüsselbund des Betriebssystems abgelegt.",
    baseUrlLabel: "API-Adresse",
    apiKeyLabel: "API-Schlüssel",
    apiKeyPlaceholder: "kido_…",
    pairAction: "Koppeln",
    pairing: "Koppele …",
    cancel: "Abbrechen",
    pickCompetition: "Wettkampf auswählen",
    refresh: "Neu laden",
    loadingCompetitions: "Lade Wettkämpfe …",
    noCompetitions: "Keine Wettkämpfe gefunden",
    activeBadge: "Aktiv",
    liveBadge: "Live",
    offlineBadge: "On-prem",
    overriddenBadge: "Desktop",
    overriddenHint: (webDefault: string) =>
      `Vom Desktop überschrieben (Standard im Web: ${webDefault})`,
    pickAction: "Auswählen",
    deselect: "Auswahl aufheben",
    openKido: "KiDo-Datei öffnen …",
    exportKido: "KiDo-Datei exportieren …",
    exportFailed: "Export fehlgeschlagen",
    exportSaved: "Gespeichert",
    needPairing: "Erst Online-Konto koppeln",
    needSnapshot: "Erst Wettkampf laden",
    failedHeader: "Nicht übertragene Ergebnisse",
    failedRetry: "Wiederholen",
    failedDismiss: "Verwerfen",
    failedRunPrefix: "Lauf",
    kidoConflictTitle: "Anderer Wettkampf geladen",
    kidoConflictBodyPrefix: "Aktuell geladen:",
    kidoConflictBodyToPrefix: "Datei enthält:",
    kidoConflictReplace: "Trotzdem laden",
    kidoConflictKeep: "Abbrechen",
  },

  laneAssign: {
    title: "Bahn-Zuordnung",
    subtitle: "Welcher Lauf wird gerade auf welcher Bahn gemessen?",
    auto: "Automatisch",
    none: "— kein Lauf —",
    laneShort: "Bahn",
    runLabel: "Lauf",
    statusActive: "Aktiv",
    statusPending: "Wartend",
    statusCompleted: "Beendet",
    wgFilter: "Wertungsgruppe",
    wgFilterAll: "Alle",
    penaltyCount: (n: number) =>
      n === 1 ? "1 Fehler" : `${n} Fehler`,
    dq: "FV",
    fromWeb: "vom Web",
    penaltyHint:
      "Fehler werden vom Organisator bestätigt bzw. zurückgenommen (Übernehmen pro Lauf).",
  },

  syncMode: {
    label: "Betriebsart",
    live: "Live",
    onPrem: "On-prem",
    liveHint: "Zeiten werden live an ki-do.it übertragen.",
    onPremHint:
      "Desktop ist Master: Strafrichter-Handys verbinden sich mit diesem Laptop; Zeiten, Fehler und Besetzungen bleiben lokal (gesichert) und werden später hochgeladen oder per KiDo-Datei übertragen.",
    webDefault: "Standard aus dem Web",
    resetToDefault: "Auf Web-Standard zurücksetzen",
  },

  penalties: {
    title: "Fehler",
    none: "Keine Fehler gemeldet",
    sz: "SZ",
    fv: "FV",
    provisionalNote: "vorläufig — offene Fehler zählen bis zur Entscheidung",
  },

  divergence: {
    title: "Lokale Ergebnisse weichen ab",
    body: "Auf diesem Desktop wurden Ergebnisse erfasst, die im Web noch nicht (oder anders) vorliegen. Die Live-Übertragung ist angehalten, bis du entscheidest.",
    run: "Lauf",
    local: "Lokal",
    web: "Web",
    missing: "fehlt im Web",
    upload: "Lokale Änderungen hochladen",
    discard: "Lokale Änderungen verwerfen",
    discardConfirm:
      "Wirklich verwerfen? Die lokal erfassten Zeiten, Fehler, Besetzungen und Handzeiten gehen verloren (Tipp: vorher KiDo-Datei exportieren).",
    discardConfirmAction: "Ja, verwerfen",
    busy: "Bitte warten …",
    extras: (penalties: number, lineups: number, handTimes: number) => {
      const parts: string[] = [];
      if (penalties > 0) parts.push(penalties === 1 ? "1 Fehler" : `${penalties} Fehler`);
      if (lineups > 0) parts.push(lineups === 1 ? "1 Besetzung" : `${lineups} Besetzungen`);
      if (handTimes > 0) parts.push(handTimes === 1 ? "1 Handzeit" : `${handTimes} Handzeiten`);
      return parts.length > 0
        ? `Außerdem lokal erfasst: ${parts.join(", ")} — werden mit hochgeladen.`
        : "";
    },
    uploadNote: "Alles wird in einem Schritt hochgeladen (Laufzeiten, Fehler inkl. Status, Besetzungen, Handzeiten).",
  },

  onprem: {
    open: "Strafrichter & Prüfung",
    title: "On-prem: Strafrichter & Prüfung",
    kicker: "Lokaler Betrieb",
    back: "Zurück",
    server: "Strafrichter-Zugang",
    serverRunning: (port: number) => `Lokaler Server läuft (Port ${port})`,
    serverStopped: "Lokaler Server ist aus",
    serverStoppedHint: "Der Server läuft nur im On-prem-Betrieb mit geladenem Wettkampf.",
    scanHint: "Handys im selben WLAN: QR-Code scannen oder Adresse im Browser öffnen.",
    noAddress: "Keine Netzwerkadresse gefunden — ist der Laptop mit dem WLAN/LAN verbunden?",
    otherAddresses: "Weitere Adressen",
    rotate: "Neuer Zugangscode",
    rotateConfirm:
      "Neuen Zugangscode erzeugen? Alle verbundenen Handys müssen danach den neuen QR-Code scannen.",
    rotateConfirmAction: "Ja, neuer Code",
    phones: "Verbundene Handys",
    phonesShort: (n: number) => (n === 1 ? "1 Handy" : `${n} Handys`),
    noPhones: "Noch kein Handy verbunden.",
    noPhoneHint:
      "Seit über einer Minute hat sich kein Handy verbunden. Prüfen: gleiches WLAN? Windows-Firewall erlaubt KiDo-Timer (Ports 8787–8797)? Gäste-WLANs blockieren oft Verbindungen zwischen Geräten.",
    judgeUiMissing:
      "Diese Version enthält die Strafrichter-Oberfläche nicht (Entwickler-Build). Bitte eine offizielle Version installieren.",
    currentRuns: "Aktuelle Läufe",
    currentRunsHint: "Pro Bahn den Lauf wählen, der gerade startet — die Strafrichter sehen ihn sofort.",
    lane: (n: number) => `Bahn ${n}`,
    noRun: "— kein Lauf —",
    next: "Nächster",
    nextTitle: "Nächsten Lauf dieser Bahn wählen",
    review: "Prüfung",
    noCurrent: "Kein aktueller Lauf — oben einen Lauf wählen.",
    otherRun: "Weiteren Lauf prüfen",
    otherRunPlaceholder: "Lauf wählen …",
    upload: "Ins Web hochladen",
    uploadHint: "Braucht Internet und das gekoppelte Web-Konto.",
    lastUpload: (at: string) => `Zuletzt hochgeladen: ${at}`,
  },

  review: {
    time: "Zeit",
    result: "Ergebnis",
    noTime: "keine Zeit",
    handTime: "Handzeit",
    handTimeButton: "Handzeit …",
    lineup: "Besetzung",
    lineupButton: "Besetzung …",
    lineupMissing: "Besetzung fehlt",
    lineupBy: (by: string) => (by === "startlinie" ? "Startlinie" : "Organisator"),
    errors: (n: number) => `Fehler (${n})`,
    none: "Keine Fehler gemeldet.",
    confirm: "Bestätigen",
    withdraw: "Zurücknehmen",
    undo: "Rückgängig",
    undoHint: "Entscheidung zurücksetzen – der Fehler ist wieder offen",
    takeOver: "Übernehmen",
    takeOverTitle: (title: string) => `Lauf übernehmen – ${title}`,
    takeOverAfter: "Ergebnis danach:",
    takeOverFvAck:
      "Ich bestätige den Fehlversuch (mit dem Mannschaftsführer geklärt) – der Lauf wird ungültig.",
    fvTitle: "Fehlversuch bestätigen?",
    fvConfirm: "FV bestätigen – Lauf ungültig",
    fvBody: (title: string) =>
      `Mit der Bestätigung wird ${title} ungültig (keine Zeit). Bitte vorher mit dem Mannschaftsführer klären.`,
    cancel: "Abbrechen",
    recordedAt: "Erfasst um",
    sz: "Strafzeit: +15 s",
    fv: "Fehlversuch: Lauf ungültig",
  },

  handTime: {
    title: (title: string) => `Handzeit – ${title}`,
    intro:
      "Nur eintragen, wenn die Lichtschranke ausgefallen ist. Ergebnis = Mittelwert der beiden Stoppuhren, auf 1/100 s gerundet. Format m:ss,hh oder ss,hh.",
    lightBarrier: (t: string) => `Lichtschranke bisher: ${t}`,
    watch1: "Stoppuhr 1",
    watch2: "Stoppuhr 2",
    mean: "Mittelwert (Handzeit)",
    save: "Handzeit speichern",
    saving: "Speichere …",
    clear: "Handzeit entfernen",
    clearHint:
      "„Handzeit entfernen“ stellt auf Lichtschranke zurück; die gespeicherte Zeit bleibt, bis der Zeitnehmer eine neue Zeit überträgt.",
    cancel: "Abbrechen",
  },

  lineup: {
    title: (title: string) => `Besetzung – ${title}`,
    intro: "Wer ist tatsächlich gelaufen? Ankreuzen (Ersatzläufer eingeschlossen).",
    empty: "Für diese Mannschaft sind keine Läufer hinterlegt.",
    save: "Besetzung speichern",
    clear: "Leeren",
    cancel: "Abbrechen",
    count: (n: number) => (n === 1 ? "1 Läufer:in" : `${n} Läufer:innen`),
  },

  upload: {
    title: "Ins Web hochladen",
    body:
      "Lädt alle auf diesem Laptop erfassten Laufzeiten, Fehler (inkl. Prüfstatus), Besetzungen und Handzeiten zum Web hoch. Mehrfaches Hochladen ist unschädlich.",
    summary: (runs: number, penalties: number) =>
      `${runs === 1 ? "1 geänderter Lauf" : `${runs} geänderte Läufe`} · ${
        penalties === 1 ? "1 Fehler" : `${penalties} Fehler`
      }`,
    goLive: "Danach auf Live umschalten (Web und Desktop)",
    start: "Jetzt hochladen",
    busy: "Lade hoch …",
    retry: "Erneut versuchen",
    close: "Schließen",
    cancel: "Abbrechen",
    done: "Hochgeladen",
    wentLive: "Desktop und Web sind jetzt auf Live.",
    goLiveFailed: "Hochgeladen — das Umschalten auf Live hat nicht geklappt; bitte später erneut wählen.",
    fallback: "Kein Internet? KiDo-Datei exportieren (Menü) und im Web importieren.",
  },

  results: {
    title: "Ergebnisse",
    kicker: "Wertung",
    open: "Ergebnisse & Druck",
    refresh: "Neu berechnen",
    print: "Drucken",
    back: "Zurück",
    empty: "Noch keine Wertung — kein Team mit Wertungsgruppe.",
    noDiscipline: "Erst Wettkampf laden.",
    tagessieger: "Tagessieger",
    tagessiegerHint: "GW-Zeit = Zeit × (1 − 2·(18 − Ø Alter)/100); Alter am Wettkampftag.",
    colPlace: "Platz",
    colStart: "Start-Nr.",
    colTeam: "Mannschaft",
    colRun: (n: number) => `Lauf ${n}`,
    colBest: "Wertung",
    colNote: "Bemerkung",
    colWg: "WG",
    colTime: "Zeit",
    colAvgAge: "Ø Alter",
    colFactor: "Faktor",
    colGw: "GW-Zeit",
    provisional: "Vorläufig — es gibt noch offene Fehler.",
    legend: "Zeiten in Sekunden (1/100 s) · H = Handzeit · * = vorläufig · FV = Fehlversuch (ungültig)",
    missingWg: (names: string) => `Ohne Wertungsgruppe, nicht gewertet: ${names}`,
    printedAt: "Gedruckt",
    page: "Seite",
    noTagessieger: "Noch kein gültiger Lauf.",
  },

  banner: {
    activeCompetition: "Aktueller Wettkampf",
    currentRun: "Aktueller Lauf",
    none: "Kein Lauf ausgewählt",
    runOf: "Lauf",
    lane: "Bahn",
    runner: "Läufer:in",
    syncLive: "Live-Übertragung an ki-do.it",
    syncOffline: "On-prem — Strafrichter verbinden sich mit diesem Laptop, alles wird lokal gesichert",
    currentRuns: "Aktuelle Läufe",
    handTime: "Handzeit",
    closeSnapshot: "Wettkampf schließen",
  },

  lanes: {
    sectionTitle: "Bahnen",
    sectionKicker: "Live",
    laneShort: "BAHN",
    correction: "Korrektur",
    original: "Original",
    penalty: "Strafe",
    total: "Gesamt",
    clearCorrection: "Korrektur zurücksetzen",
    minus5: "−5 s",
    minus1: "−1 s",
    plus1: "+1 s",
    plus5: "+5 s",
    inactivePlaceholder: "––:––.–––",
    focusHint:
      "Bahn anklicken (oder Tasten 1–4) · +/− verstellt um ±5 s · C löscht Korrektur",
  },

  protocol: {
    title: "Lauf-Protokoll",
    subtitle: "Bestätigte Zeiten · neueste zuerst",
    empty: "Noch keine bestätigten Zeiten",
    kpiCount: "Einträge",
    kpiFastest: "Schnellste",
    kpiSlowest: "Langsamste",
    kpiCorrected: "Mit Korrektur",
    columnIndex: "Lauf",
    columnLane: "Bahn",
    columnOriginal: "Original-Zeit",
    columnPenalty: "Strafzeit",
    columnTotal: "Gesamt-Zeit",
    columnTimestamp: "Zeitstempel",
    columnDate: "Datum",
    exportCsv: "Als CSV exportieren",
    clearAll: "Protokoll leeren",
    clearConfirm: "Alle Einträge im Protokoll löschen?",
    clearConfirmAction: "Leeren bestätigen",
    saved: "Gespeichert: ",
    exportFailed: "Export fehlgeschlagen",
  },

  errors: {
    title: "Fehler vom Gerät",
    dismiss: "Verbergen",
    notConnectedAction: "Nicht verbunden",
    needDesktopApp: "Diese Aktion benötigt die Desktop-App",
  },

  updates: {
    available: "Update verfügbar",
    versionPrefix: "Version",
    installNow: "Jetzt installieren",
    later: "Später",
    downloading: "Wird heruntergeladen…",
    installing: "Installation läuft, App startet neu…",
    failed: "Update fehlgeschlagen",
    checkNow: "Auf Updates prüfen",
    checking: "Suche nach Updates…",
    upToDate: "App ist auf dem neuesten Stand",
  },

  csv: {
    headers: [
      "Lauf",
      "Bahn",
      "Original-Zeit (s,ms)",
      "Strafzeit (s,ms)",
      "Gesamt-Zeit (s,ms)",
      "Zeitstempel",
      "Datum",
    ],
  },
};

const STATUS: Record<ChannelStatus, string> = {
  inactive: "Inaktiv",
  running: "Läuft",
  captured: "Erfasst",
  confirmed: "Bestätigt",
  unknown: "Unbekannt",
};

export function statusLabelDE(s: ChannelStatus): string {
  return STATUS[s];
}

const DEVICE: Record<DeviceMode, string> = {
  standby: "Bereit",
  active: "Aktiv",
  unknown: "Unbekannt",
};

export function deviceModeDE(d: DeviceMode): string {
  return DEVICE[d];
}

const STATE_FLAG: Record<StateFlag, string> = {
  measuring: "Misst",
  armed: "Scharf",
  unknown: "Unbekannt",
};

export function stateFlagDE(s: StateFlag): string {
  return STATE_FLAG[s];
}

/** Kurz-Label für eine Wertungsgruppe (WG1-5). */
export function wgLabelDE(wg: string): string {
  switch (wg) {
    case "wg1":
      return "WG 1";
    case "wg2":
      return "WG 2";
    case "wg3":
      return "WG 3";
    case "wg4":
      return "WG 4";
    case "wg5":
      return "WG 5";
    default:
      return wg.toUpperCase();
  }
}

/** Beschreibung einer Wertungsgruppe (für Tooltips). */
export function wgDescriptionDE(wg: string): string {
  switch (wg) {
    case "wg1":
      return "Jungen + Mädchen 8–10 (gemischt)";
    case "wg2":
      return "Jungen bis 14";
    case "wg3":
      return "Mädchen bis 14";
    case "wg4":
      return "Jungen bis 18";
    case "wg5":
      return "Mädchen bis 18";
    default:
      return wg;
  }
}

/** Label einer Wertungsrichter-Station (kanonische Keys + Alt-Aliase). */
export function stationLabelDE(station: string): string {
  switch (station) {
    case "startlinie":
    case "start":
      return "Startlinie/Zeitnehmer";
    case "verteilerlinie":
    case "verteiler":
      return "Verteilerlinie";
    case "strahlrohrlinie":
    case "strahlrohr":
      return "Strahlrohrlinie";
    case "knotengestell":
    case "knoten":
      return "Knotengestell";
    case "angriffslinie":
    case "kuebelspritze":
      return "Angriffslinie";
    case "podest":
      return "Podest/Wasserbehälter";
    default:
      return station;
  }
}

/** Disziplin-Bezeichnung für die UI, optional mit Bahnenzahl. */
export function modeLabelDE(mode: string, laneCount?: number): string {
  const lanes =
    laneCount == null ? "" : laneCount === 1 ? " (1 Bahn)" : ` (${laneCount} Bahnen)`;
  switch (mode) {
    case "gruppenstaffette":
      return `Gruppenstafette${lanes}`;
    case "loeschangriff":
      return `Löschangriff${lanes}`;
    default:
      return mode;
  }
}
