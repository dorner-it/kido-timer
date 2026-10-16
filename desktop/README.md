# KiDo-Timer — Sports Timer Console

A desktop console for the **KiDo-Timer**, built with **Tauri 2** + **React + TypeScript + Tailwind**.
Connects directly to the timer over a serial port, parses the proprietary 41-byte frame protocol, and presents
a coach/operator workstation with four lane channels, software time correction, a confirmed-split run log, and
demo replay of captured hex dumps.

The app is a single self-contained Windows executable. There are no network dependencies at runtime.

## Features

- **Live serial connection** to the KiDo-Timer at 9600 baud (8N1) — port and baud rate selected from the UI.
- **Four-lane dashboard** with status-color glow (idle / running / captured / confirmed) and large tabular-numeric times.
- **Per-channel software corrections** in ±1 s and ±5 s steps, with live preview and clear.
- **Run log** of every confirmed split (lane + time + correction + timestamp).
- **Device reset** (`RST\r`) sent over the wire with a confirm step.
- **Demo Mode** — load a captured xxd-style hex dump and replay it at any speed; great for training, demos,
  and developing without the timer present.
- **Keyboard shortcuts**: `1`–`4` focus a lane, `+`/`−` adjust the focused lane by ±5 s, `C` clears the
  correction, `Esc` clears focus.

## Project layout

```
kido-timer-app/
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
│   │   └── dto.rs           # serializable shapes for the frontend
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
npm run tauri dev
```

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

### Backend tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

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
