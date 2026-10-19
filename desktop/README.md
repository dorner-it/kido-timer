# KiDo-Timer — Sports Timer Console

A desktop console for the **KiDo-Timer**, built with **Tauri 2** + **React + TypeScript + Tailwind**.
Connects directly to the timer over a serial port, parses the proprietary 41-byte frame protocol, and presents
a coach/operator workstation with four lane channels, software time correction, a confirmed-split run log, and
demo replay of captured hex dumps.

The app is a single self-contained Windows executable. Network access is only needed for the
optional live connection to ki-do.it (no web fonts, no CDN).

## Features

- **Live serial connection** to the KiDo-Timer at 9600 baud (8N1) — port and baud rate selected from the UI.
- **Four-lane dashboard** with status-color glow (idle / running / captured / confirmed) and large tabular-numeric times.
- **Per-channel software corrections** in ±1 s and ±5 s steps, with live preview and clear.
- **Run log** of every confirmed split (lane + time + correction + timestamp).
- **Device reset** (`RST\r`) sent over the wire with a confirm step.
- **Demo Mode** — load a captured xxd-style hex dump and replay it at any speed; great for training, demos,
  and developing without the timer present.
- **Wettkampf (schema v4, Gruppenstafette 2027)** — loads a discipline from ki-do.it or a signed
  `.kido` file; shows `lane_count` lanes (lane N = timer channel N), all current runs with their
  errors (SZ/FV, catalogue text, status) and the provisional run result.
- **Live / On-prem** — the web sets the default per discipline, the desktop can override it.
  Every recorded time lands in the local result store (JSON in the app data dir, restored after a
  restart); live mode additionally posts it. Switching back to live with diverging local changes
  asks "hochladen" or "verwerfen"; choosing "Live" switches the web to live first.
- **On-prem judge server (0.7.0)** — in on-prem mode the app serves the Strafrichter app to the
  judges' phones in the venue network (see [On-prem](#on-prem-strafrichter-im-lokalen-netz)).
  The organizer picks the current runs per lane, reviews errors (Bestätigen / Zurücknehmen,
  FV confirmation, "Übernehmen"), enters hand times and corrects lineups in the desktop, and
  finally uploads everything with "Ins Web hochladen" (or exports a `.kido`).
- **Ergebnisse & Druck** — WG rankings and Tagessieger computed locally with `kido-core`
  (identical to the web), print layout A4 (one WG per page). No web fonts — works offline.
- **Keyboard shortcuts**: `1`–`4` focus a lane, `+`/`−` adjust the focused lane by ±5 s, `C` clears the
  correction, `Esc` clears focus.

## Project layout

```
desktop/   (inside the kido-timer monorepo)
├── src/                 # React frontend (Vite + TS + Tailwind)
│   ├── components/      # TopBar, LaneGrid, LaneCard, ConnectionPanel, ActionPanel, RunHistory, ErrorBanner
│   ├── lib/             # Tauri bindings, types, useConnection hook, dev simulator
│   ├── App.tsx
│   └── styles.css
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── lib.rs           # Tauri builder + command registration
│   │   ├── commands.rs      # invoke handlers
│   │   ├── state.rs         # connection lifecycle, event bridge
│   │   ├── protocol.rs      # 41-byte frame parser
│   │   ├── serial_reader.rs # serial thread + sync
│   │   ├── file_reader.rs   # hex-dump replay
│   │   ├── dto.rs           # serializable shapes for the frontend
│   │   ├── cloud/           # web API client, live sync, .kido, upload (A2/A3)
│   │   └── local/           # local result store (format 2) + on-prem
│   │       ├── store.rs     #   base snapshot + overlays (timing, penalties, lineups, hand times)
│   │       ├── persist.rs   #   atomic JSON persistence, format 1 → 2 migration
│   │       ├── judge.rs     #   judge API handlers (pure, mirror the backend)
│   │       ├── organizer.rs #   review / Übernehmen / hand times / lineup / current runs
│   │       └── server/      #   axum server, embedded judge UI, rate limit, QR, LAN IPs
│   ├── windows/installer-hooks.nsh  # NSIS: firewall rule for TCP 8787–8797
│   ├── capabilities/        # Tauri 2 permission grants
│   ├── icons/               # bundle icons
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable, edition 2021+)
- [Node.js](https://nodejs.org/) 20+
- Platform Tauri prerequisites (see <https://tauri.app/start/prerequisites/>):
  on macOS this is just Xcode CLT; on Windows it's WebView2 + Microsoft VS Build Tools.

### Run the desktop app

```bash
npm install
(cd ../frontend && npm ci && npm run build:judge)   # judge UI for on-prem (optional)
npm run tauri dev
```

The judge UI (`frontend/dist-judge`, built with `npm run build:judge`) is embedded into the
binary at compile time by `src-tauri/build.rs`. Without it, `cargo build` still works and the
server shows a placeholder page ("Strafrichter-App nicht enthalten"); CI and the release
workflow always build it first. `KIDO_JUDGE_DIST=/path` overrides the folder.

The Vite dev server runs on `localhost:5173` and Tauri loads it inside a native webview window.

### Browser-only frontend dev (no Tauri)

```bash
npm run dev
```

When you load `http://localhost:5173` in a regular browser, the Tauri APIs are not available. The app
detects this and starts a synthetic frame simulator so the UI can be exercised end-to-end without
hardware. The simulator runs a 30-second cycle: standby → lanes 1+2 running → captured → confirmed → reset.

### Build for production

```bash
npm run tauri build
```

Outputs land under `src-tauri/target/release/bundle/`. On Windows that's an `.msi` and an `.nsis` installer.

### Tests

```bash
npm test                                   # vitest (pure TS logic)
cargo test --manifest-path src-tauri/Cargo.toml   # incl. an HTTP end-to-end test of the judge server
make desktop-check                         # from the repo root: judge build + all desktop gates
```

## On-prem: Strafrichter im lokalen Netz

When the effective mode of the loaded discipline is **On-prem** (web default `offline` or the
desktop override), the app starts a small HTTP server:

- binds `0.0.0.0:8787` (if busy: 8788 … 8797) and shows all LAN IPv4 addresses with a QR code
  `http://<laptop-ip>:<port>/judge/?t=<Zugangscode>&o=<owner>` on the screen
  "Strafrichter & Prüfung",
- serves the same judge app as the web (`/judge/`) and the judge API
  (`/api/catalogue`, `/api/decider/{owner}/{token}/…`) with the web's JSON shapes and rules,
- the access code (128 bit) is kept with the local store per discipline — phones keep working
  after a restart. "Neuer Zugangscode" rotates it (old phones get "Zugangscode ungültig"),
- plain HTTP in the LAN, 64 KiB body limit, per-IP rate limit; stops when leaving on-prem,
  closing the discipline or quitting the app.

Phones and laptop must be in the same network. Guest WLANs often isolate clients — use a
normal WLAN / a travel router. If no phone has connected one minute after the start, the
screen shows a hint (firewall / network).

After the competition: **"Ins Web hochladen"** (needs internet + paired account) sends all
locally changed runs (time, status, lineup, hand times) and all penalties incl. review status
in one request; uploading twice is harmless. Optionally switch to live right after. Without
internet: export a `.kido` file (contains all penalties and run fields) and import it on the web.

### Windows-Firewall

The NSIS installer (`*-setup.exe`) adds an inbound rule for the app on install and removes it
on uninstall:

```
netsh advfirewall firewall add rule name="KiDo-Timer On-prem (Strafrichter)" dir=in action=allow ^
  protocol=TCP localport=8787-8797 profile=private,public program="C:\Program Files\KiDo-Timer\KiDo-Timer.exe" enable=yes
```

**Manual fallback** (MSI install, rule deleted, or a third-party firewall): open an
administrator command prompt and run the command above with the path of the installed exe,
or in "Windows Defender Firewall mit erweiterter Sicherheit" → Eingehende Regeln → Neue Regel →
Programm → `KiDo-Timer.exe` → Verbindung zulassen → Privat + Öffentlich. Alternatively allow the
app when Windows asks on the first start of the on-prem server ("Zugriff zulassen", tick
private *and* public networks). To remove the rule:

```
netsh advfirewall firewall delete rule name="KiDo-Timer On-prem (Strafrichter)"
```

macOS asks once whether the app may accept incoming connections — choose "Erlauben".

## Protocol

The KiDo-Timer transmits fixed **41-byte frames** at 9600 baud (8N1):

```
Offset  Size  Description
------  ----  -----------
 0..2     3   Header: "RW:" (0x52 0x57 0x3A)
 3        1   Protocol version (0x01)
 4        1   Device mode: 0x00=Standby, 0x02=Active
 5        1   Lane number (1-based)
 6        1   State flag: 0x00=Measuring, 0x01=Armed
 7..22   16   Channels 1-4 (4 bytes each: time_lo, time_hi, reserved, status)
23..38   16   Reserved
39        1   Terminator: 0x0D (CR)
40        1   Trailing null: 0x00
```

Channel status: `0x00` Inactive, `0x81` Running, `0x82` Captured, `0x84` Confirmed.
Times are little-endian unsigned 16-bit milliseconds (max 65,535 ms).

The reset command (`RST\r`) is sent over the same serial connection.

## Demo / hex-dump file format

The replay feature accepts xxd-style hex dumps. Lines starting with `---` are treated as comments and skipped:

```
--- Connected to COM7 at 9600 baud (8N1, hex) ---
00000000  52 57 3A 01 00 01 00 00  00 00 00 00 00 00 00 00  |RW:.............|
00000010  00 00 00 00 00 00 00 00  00 00 00 00 00 00 00 00  |................|
```

A sample capture (`COM7_9600_20260210_183421.txt`) is included in the repo.

## Releases

Tagged `v*` pushes trigger a Windows build via GitHub Actions; the resulting `.msi` and `.exe`
installers are attached to the GitHub release.
