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
    confirmRunHint: "Bestätigte Bahnen an die Cloud melden",
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
    offlineBadge: "Offline",
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
    wgFilter: "Wertungsgruppe",
    wgFilterAll: "Alle",
    penaltyCount: (n: number) =>
      n === 1 ? "1 Strafe" : `${n} Strafen`,
    dq: "DQ",
    penaltyHint:
      "Strafen werden im Web-Frontend des Organisators bestätigt (Bulk-Approve pro Lauf).",
  },

  banner: {
    activeCompetition: "Aktueller Wettkampf",
    currentRun: "Aktueller Lauf",
    none: "Kein Lauf ausgewählt",
    runOf: "Lauf",
    lane: "Bahn",
    runner: "Läufer:in",
    syncLive: "Live-Übertragung an ki-do.it",
    syncOffline: "Offline-Modus — keine Übertragung",
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
      return "Jungen + Mädchen 8-10 (gemischt)";
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

/** Label einer Strafrichter-Station. */
export function stationLabelDE(station: string): string {
  switch (station) {
    case "start":
      return "Start";
    case "verteiler":
      return "Verteiler";
    case "strahlrohr":
      return "Strahlrohr";
    case "knoten":
      return "Knoten";
    case "kuebelspritze":
      return "Kübelspritze";
    case "podest":
      return "Podest";
    case "strahlrohrlinie":
      return "Strahlrohrlinie";
    default:
      return station;
  }
}

/**
 * Disziplin-Bezeichnung für die UI. Mappt sowohl die neuen
 * disziplinspezifischen Werte (`gruppenstaffette`, `loeschangriff`) als auch
 * die historischen TimerMode-Werte aus dem Web-Backend, solange der
 * Schema-Bump dort noch nicht ausgerollt ist.
 */
export function modeLabelDE(mode: string): string {
  switch (mode) {
    case "gruppenstaffette":
      return "Gruppenstaffette (2 Bahnen)";
    case "loeschangriff":
      return "Löschangriff (4 Bahnen)";
    case "single_lane":
      return "Gruppenstaffette (Einzelbahn)";
    case "two_lane_parallel":
      return "Gruppenstaffette (2 Bahnen)";
    case "relay":
      return "Gruppenstaffette";
    case "individual":
      return "Löschangriff";
    default:
      return mode;
  }
}
