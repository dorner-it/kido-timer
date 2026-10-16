use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::cloud::CloudState;
use crate::dto::{ConnectionEvent, FrameDto};
use crate::file_reader::{spawn_file_reader, FileReader};
use crate::serial_reader::{spawn_serial_reader, SerialEvent, SerialReader};

const RESET_CMD: &[u8] = b"RST\r";

/// Active connection: either a real serial port (with a write handle) or a
/// demo file replayer.
pub enum Connection {
    Serial { reader: SerialReader, source: String },
    Demo { reader: FileReader, source: String },
}

impl Connection {
    pub fn source(&self) -> &str {
        match self {
            Connection::Serial { source, .. } => source,
            Connection::Demo { source, .. } => source,
        }
    }

    pub fn stop(self) {
        match self {
            Connection::Serial { reader, .. } => {
                reader.stop_flag.store(true, Ordering::Relaxed);
                let _ = reader.thread.join();
            }
            Connection::Demo { reader, .. } => {
                reader.stop_flag.store(true, Ordering::Relaxed);
                let _ = reader.thread.join();
            }
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub connection: Mutex<Option<Connection>>,
    pub cloud: CloudState,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Disconnect the current connection (if any). Idempotent.
    pub fn disconnect(&self) {
        if let Some(conn) = self.connection.lock().take() {
            conn.stop();
        }
    }

    /// Connect to a serial port. Replaces any existing connection.
    pub fn connect_serial(
        &self,
        app_handle: AppHandle,
        port_name: String,
        baud: u32,
    ) -> Result<(), String> {
        self.disconnect();

        let (tx, rx) = mpsc::channel::<SerialEvent>();
        let reader = spawn_serial_reader(&port_name, baud, tx)?;
        let source = format!("{} @ {} baud", port_name, baud);

        spawn_event_bridge(app_handle.clone(), rx);

        // Emit connected immediately so the UI updates.
        let _ = app_handle.emit(
            "kido://event",
            ConnectionEvent::Connected {
                source: source.clone(),
            },
        );

        *self.connection.lock() = Some(Connection::Serial { reader, source });
        Ok(())
    }

    /// Start the demo file replayer.
    pub fn start_demo(
        &self,
        app_handle: AppHandle,
        path: String,
        speed: f64,
    ) -> Result<(), String> {
        self.disconnect();

        let (tx, rx) = mpsc::channel::<SerialEvent>();
        let reader = spawn_file_reader(&path, speed, tx)?;
        let source = format!(
            "demo: {} @ {}x",
            std::path::Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&path),
            if speed == 0.0 {
                "max".to_string()
            } else {
                format!("{}", speed)
            }
        );

        spawn_event_bridge(app_handle.clone(), rx);

        let _ = app_handle.emit(
            "kido://event",
            ConnectionEvent::Connected {
                source: source.clone(),
            },
        );

        *self.connection.lock() = Some(Connection::Demo { reader, source });
        Ok(())
    }

    /// Send the RST\r command to the connected device. Demo mode is a no-op.
    pub fn send_reset(&self) -> Result<(), String> {
        let mut guard = self.connection.lock();
        match guard.as_mut() {
            Some(Connection::Serial { reader, .. }) => reader
                .write_port
                .write_all(RESET_CMD)
                .map_err(|e| format!("Write failed: {}", e)),
            Some(Connection::Demo { .. }) => Ok(()),
            None => Err("Not connected".to_string()),
        }
    }
}

/// Spawn a thread that bridges SerialEvents from the reader thread to Tauri
/// events fired on the main app handle.
fn spawn_event_bridge(app_handle: AppHandle, rx: mpsc::Receiver<SerialEvent>) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let payload = match event {
                SerialEvent::Frame(frame) => ConnectionEvent::Frame {
                    frame: FrameDto::from(&frame),
                },
                SerialEvent::Error(message) => ConnectionEvent::Error { message },
                SerialEvent::Disconnected => ConnectionEvent::Disconnected,
            };
            let is_terminal = matches!(payload, ConnectionEvent::Disconnected);
            let _ = app_handle.emit("kido://event", payload);
            if is_terminal {
                break;
            }
        }
    });
}

