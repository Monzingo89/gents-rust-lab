//! Bounded newline framing; protocol lifecycle and tools are transport independent.
use crate::protocol::{error, Session};
use anyhow::{bail, Result};
use serde_json::Value;
use std::io::{BufRead, Write};
const MAX_FRAME: usize = 65536;
pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                bail!("unterminated stdio frame")
            };
        }
        let count = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |i| i + 1);
        if frame.len() + count > MAX_FRAME {
            bail!("stdio frame exceeds 64 KiB");
        }
        let done = bytes[count - 1] == b'\n';
        frame.extend_from_slice(&bytes[..count]);
        reader.consume(count);
        if done {
            return Ok(Some(frame));
        }
    }
}
pub fn serve(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    session: &mut Session<'_>,
) -> Result<()> {
    while let Some(frame) = read_frame(reader)? {
        let response = match serde_json::from_slice::<Value>(&frame) {
            Ok(request) => session.handle(request),
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(response) = response {
            let mut bytes = serde_json::to_vec(&response)?;
            if bytes.len() > MAX_FRAME - 1 {
                bytes = serde_json::to_vec(&error(
                    response["id"].clone(),
                    -32603,
                    "Tool output exceeds bounded frame",
                ))?;
            }
            writer.write_all(&bytes)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}
