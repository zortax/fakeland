//! A byte ring buffer that keeps the newest output of a child stream.

use std::collections::VecDeque;

/// Keeps the last `capacity` bytes written to it.
#[derive(Debug, Clone)]
pub struct RingBuffer {
    data: VecDeque<u8>,
    capacity: usize,
}

impl RingBuffer {
    /// Builds an empty buffer.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity.min(64 * 1024)),
            capacity,
        }
    }

    /// Appends bytes and drops the oldest bytes beyond the capacity.
    pub fn write(&mut self, bytes: &[u8]) {
        if self.capacity == 0 {
            return;
        }
        let bytes = if bytes.len() > self.capacity {
            &bytes[bytes.len() - self.capacity..]
        } else {
            bytes
        };
        let overflow = (self.data.len() + bytes.len()).saturating_sub(self.capacity);
        self.data.drain(..overflow);
        self.data.extend(bytes);
    }

    /// Returns the kept bytes, oldest first.
    pub fn contents(&self) -> Vec<u8> {
        self.data.iter().copied().collect()
    }

    /// Returns the kept bytes as text. Invalid UTF-8 becomes U+FFFD.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.contents()).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_bytes() {
        let mut ring = RingBuffer::new(5);
        ring.write(b"abc");
        assert_eq!(ring.text(), "abc");
        ring.write(b"defg");
        assert_eq!(ring.text(), "cdefg");
        ring.write(b"0123456789");
        assert_eq!(ring.text(), "56789");
    }

    #[test]
    fn zero_capacity_keeps_nothing() {
        let mut ring = RingBuffer::new(0);
        ring.write(b"abc");
        assert!(ring.contents().is_empty());
    }
}
