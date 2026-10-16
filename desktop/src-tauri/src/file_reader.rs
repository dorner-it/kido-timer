use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::{parse_frame, FRAME_SIZE, HEADER};
use crate::serial_reader::SerialEvent;

pub struct FileReader {
    pub thread: JoinHandle<()>,
    pub stop_flag: Arc<AtomicBool>,
}

/// Parse an xxd-style hex dump into raw bytes. Lines starting with `---`
/// or lacking hex data are skipped.
pub fn parse_hexdump(path: &str) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut bytes = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with("---") {
            continue;
        }

        let Some(hex_start) = trimmed.find("  ") else {
            continue;
        };

        let after_offset = &trimmed[hex_start + 2..];

        let hex_part = match after_offset.rfind("  |") {
            Some(pos) => &after_offset[..pos],
            None => after_offset,
        };

        for token in hex_part.split_whitespace() {
            if let Ok(byte) = u8::from_str_radix(token, 16) {
                bytes.push(byte);
            }
        }
    }

    Ok(bytes)
}

/// Spawn a thread that replays a hex dump file through the SerialEvent channel.
pub fn spawn_file_reader(
    path: &str,
    speed: f64,
    tx: Sender<SerialEvent>,
) -> Result<FileReader, String> {
    let raw_bytes =
        parse_hexdump(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread_stop = stop_flag.clone();

    let handle = thread::spawn(move || {
        let buffer = raw_bytes;
        let mut offset = 0;

        while offset < buffer.len() {
            if thread_stop.load(Ordering::Relaxed) {
                break;
            }

            let remaining = &buffer[offset..];
            let header_pos = remaining
                .windows(HEADER.len())
                .position(|w| w == HEADER);

            let header_pos = match header_pos {
                Some(pos) => pos,
                None => break,
            };

            offset += header_pos;

            if offset + FRAME_SIZE > buffer.len() {
                break;
            }

            let mut frame_bytes = [0u8; FRAME_SIZE];
            frame_bytes.copy_from_slice(&buffer[offset..offset + FRAME_SIZE]);

            match parse_frame(&frame_bytes) {
                Ok(frame) => {
                    if tx.send(SerialEvent::Frame(frame)).is_err() {
                        return;
                    }
                    offset += FRAME_SIZE;

                    if speed > 0.0 {
                        let delay_ms = (41.0 / speed) as u64;
                        // Sleep in small slices so we can react to stop_flag.
                        let mut remaining = delay_ms;
                        while remaining > 0 {
                            if thread_stop.load(Ordering::Relaxed) {
                                let _ = tx.send(SerialEvent::Disconnected);
                                return;
                            }
                            let slice = remaining.min(20);
                            thread::sleep(Duration::from_millis(slice));
                            remaining -= slice;
                        }
                    }
                }
                Err(e) => {
                    if tx
                        .send(SerialEvent::Error(format!("Parse error: {}", e)))
                        .is_err()
                    {
                        return;
                    }
                    offset += HEADER.len();
                }
            }
        }

        let _ = tx.send(SerialEvent::Disconnected);
    });

    Ok(FileReader {
        thread: handle,
        stop_flag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hexdump_basic() {
        let tmp = std::env::temp_dir().join("test_hexdump.txt");
        std::fs::write(
            &tmp,
            "--- Connected to COM7 at 9600 baud (8N1, hex) ---\n\
             00000000  52 57 3A 01 00 01 00 00  00 00 00 00 00 00 00 00  |RW:.............|",
        )
        .unwrap();
        let bytes = parse_hexdump(tmp.to_str().unwrap()).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[..3], &[0x52, 0x57, 0x3A]);
        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_parse_hexdump_skips_separators() {
        let tmp = std::env::temp_dir().join("test_hexdump_sep.txt");
        std::fs::write(
            &tmp,
            "--- some separator ---\n\
             --- another ---\n\
             00000000  AA BB CC  |...|",
        )
        .unwrap();
        let bytes = parse_hexdump(tmp.to_str().unwrap()).unwrap();
        assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC]);
        std::fs::remove_file(tmp).ok();
    }
}
