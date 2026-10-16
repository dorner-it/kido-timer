use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::{parse_frame, FRAME_SIZE, HEADER};

pub enum SerialEvent {
    Frame(crate::protocol::TimerFrame),
    Error(String),
    Disconnected,
}

pub struct SerialReader {
    pub thread: JoinHandle<()>,
    pub write_port: Box<dyn serialport::SerialPort>,
    pub stop_flag: Arc<AtomicBool>,
}

pub fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default()
}

pub fn spawn_serial_reader(
    port_name: &str,
    baud_rate: u32,
    tx: Sender<SerialEvent>,
) -> Result<SerialReader, String> {
    let port_name_owned = port_name.to_string();

    let mut port = serialport::new(port_name, baud_rate)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("Failed to open {}: {}", port_name, e))?;

    let write_port = port
        .try_clone()
        .map_err(|e| format!("Failed to clone port handle: {}", e))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread_stop = stop_flag.clone();

    let handle = thread::spawn(move || {
        let mut buffer: Vec<u8> = Vec::with_capacity(256);
        let mut read_buf = [0u8; 128];

        loop {
            if thread_stop.load(Ordering::Relaxed) {
                let _ = tx.send(SerialEvent::Disconnected);
                return;
            }

            match port.read(&mut read_buf) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&read_buf[..n]);
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    let _ = tx.send(SerialEvent::Error(format!(
                        "Read error on {}: {}",
                        port_name_owned, e
                    )));
                    let _ = tx.send(SerialEvent::Disconnected);
                    return;
                }
            }

            loop {
                let header_pos = buffer.windows(HEADER.len()).position(|w| w == HEADER);
                let header_pos = match header_pos {
                    Some(pos) => pos,
                    None => {
                        if buffer.len() > 2 {
                            let keep_from = buffer.len() - 2;
                            buffer.drain(..keep_from);
                        }
                        break;
                    }
                };

                if header_pos > 0 {
                    buffer.drain(..header_pos);
                }

                if buffer.len() < FRAME_SIZE {
                    break;
                }

                let mut frame_bytes = [0u8; FRAME_SIZE];
                frame_bytes.copy_from_slice(&buffer[..FRAME_SIZE]);

                match parse_frame(&frame_bytes) {
                    Ok(frame) => {
                        if tx.send(SerialEvent::Frame(frame)).is_err() {
                            return;
                        }
                        buffer.drain(..FRAME_SIZE);
                    }
                    Err(e) => {
                        if tx
                            .send(SerialEvent::Error(format!("Parse error: {}", e)))
                            .is_err()
                        {
                            return;
                        }
                        buffer.drain(..HEADER.len());
                    }
                }
            }

            if buffer.len() > 4096 {
                let keep_from = buffer.len() - 256;
                buffer.drain(..keep_from);
            }
        }
    });

    Ok(SerialReader {
        thread: handle,
        write_port,
        stop_flag,
    })
}
