//! Line framing: one JSON value per line, with a bound on the line length.

use std::io::{self, BufRead};

use serde::Serialize;

/// The longest accepted line in bytes. Scenarios with inline PNG references fit well below it.
pub const MAX_LINE: usize = 64 << 20;

/// Serializes a value as one line with a trailing newline.
pub fn encode_line<T: Serialize>(value: &T) -> serde_json::Result<String> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

/// Reads lines up to [`MAX_LINE`] bytes and skips blank lines.
#[derive(Debug)]
pub struct LineReader<R> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: BufRead> LineReader<R> {
    /// Wraps a buffered reader.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
        }
    }

    /// Returns the next non-blank line without its newline, or `None` at the end of the stream.
    /// A line longer than [`MAX_LINE`] is an `InvalidData` error.
    pub fn next_line(&mut self) -> io::Result<Option<String>> {
        loop {
            self.buf.clear();
            let n = <&mut R as std::io::Read>::take(&mut self.inner, MAX_LINE as u64 + 1)
                .read_until(b'\n', &mut self.buf)?;
            if n == 0 {
                return Ok(None);
            }
            if self.buf.len() > MAX_LINE {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
            }
            let text = std::str::from_utf8(&self.buf)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .trim();
            if !text.is_empty() {
                return Ok(Some(text.to_owned()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_lines_and_skips_blank_ones() {
        let data = b"{\"a\":1}\n\n  \n{\"b\":2}\r\n{\"c\":3}";
        let mut r = LineReader::new(&data[..]);
        assert_eq!(r.next_line().unwrap().as_deref(), Some("{\"a\":1}"));
        assert_eq!(r.next_line().unwrap().as_deref(), Some("{\"b\":2}"));
        assert_eq!(r.next_line().unwrap().as_deref(), Some("{\"c\":3}"));
        assert_eq!(r.next_line().unwrap(), None);
    }

    #[test]
    fn encodes_one_line() {
        let line = encode_line(&serde_json::json!({"x": "a\nb"})).unwrap();
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.ends_with('\n'));
    }
}
