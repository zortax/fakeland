//! The bounded queue from the compositor thread to the writer thread. Each message carries a
//! frame with a repeat count, and the count of stale slots before it: slots whose frame did not
//! fit into the queue and that the writer fills with its last frame.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use fakeland_vision::Image;

/// The memory the queue may hold.
pub const QUEUE_BYTES: usize = 256 << 20;

/// Returns the queue length for frames of `frame_bytes` bytes: what fits into `QUEUE_BYTES`,
/// from 4 to 64 frames.
pub fn queue_frames(frame_bytes: usize) -> usize {
    (QUEUE_BYTES / frame_bytes.max(1)).clamp(4, 64)
}

/// A message to the writer thread.
#[derive(Debug)]
pub enum WriterMsg {
    /// Repeat the last frame `stale` times, then write `image` `count` times.
    Frame {
        /// Slots before the image that show the last frame.
        stale: u64,
        /// The new frame.
        image: Arc<Image>,
        /// The slots that show the new frame.
        count: u64,
    },
    /// Repeat the last frame `stale` times and close the output.
    Finish {
        /// Slots that show the last frame.
        stale: u64,
    },
}

/// The outcome of one push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Push {
    /// The frame is in the queue.
    Queued,
    /// The queue is full, so the slots repeat the last queued frame.
    Dropped,
    /// The writer ended.
    Closed,
}

/// The sending side of the queue.
#[derive(Debug)]
pub struct FrameQueue {
    tx: SyncSender<WriterMsg>,
    stale: u64,
    dropped: u64,
    closed: bool,
}

impl FrameQueue {
    /// Builds a queue of `frames` messages.
    pub fn new(frames: usize) -> (Self, Receiver<WriterMsg>) {
        let (tx, rx) = sync_channel(frames);
        (
            Self {
                tx,
                stale: 0,
                dropped: 0,
                closed: false,
            },
            rx,
        )
    }

    /// Queues a frame for `count` slots without waiting.
    pub fn push(&mut self, image: Arc<Image>, count: u64) -> Push {
        if self.closed {
            return Push::Closed;
        }
        if count == 0 {
            return Push::Queued;
        }
        let msg = WriterMsg::Frame {
            stale: self.stale,
            image,
            count,
        };
        match self.tx.try_send(msg) {
            Ok(()) => {
                self.stale = 0;
                Push::Queued
            }
            Err(TrySendError::Full(_)) => {
                self.stale += count;
                self.dropped += count;
                Push::Dropped
            }
            Err(TrySendError::Disconnected(_)) => {
                self.closed = true;
                Push::Closed
            }
        }
    }

    /// Returns the slots that repeat a stale frame because the queue was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Sends the end of the stream and waits for room in the queue. Call it off the compositor
    /// thread or at shutdown.
    pub fn finish(self) {
        let _ = self.tx.send(WriterMsg::Finish { stale: self.stale });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_queue_turns_frames_into_stale_slots() {
        let (mut q, rx) = FrameQueue::new(1);
        let img = Arc::new(Image::new(2, 2));
        assert_eq!(q.push(img.clone(), 1), Push::Queued);
        assert_eq!(q.push(img.clone(), 3), Push::Dropped);
        assert_eq!(q.dropped(), 3);
        let WriterMsg::Frame { stale, count, .. } = rx.recv().unwrap() else {
            panic!("frame");
        };
        assert_eq!((stale, count), (0, 1));
        assert_eq!(q.push(img.clone(), 2), Push::Queued);
        let WriterMsg::Frame { stale, count, .. } = rx.recv().unwrap() else {
            panic!("frame");
        };
        assert_eq!((stale, count), (3, 2));
        assert_eq!(q.push(img.clone(), 0), Push::Queued);
        q.finish();
        assert!(matches!(rx.recv().unwrap(), WriterMsg::Finish { stale: 0 }));
    }

    #[test]
    fn queue_length_follows_the_frame_size() {
        assert_eq!(queue_frames(1280 * 720 * 4), 64);
        assert_eq!(queue_frames(3840 * 2160 * 4), 8);
        assert_eq!(queue_frames(7680 * 4320 * 4), 4);
    }

    #[test]
    fn a_gone_writer_closes_the_queue() {
        let (mut q, rx) = FrameQueue::new(1);
        drop(rx);
        assert_eq!(q.push(Arc::new(Image::new(2, 2)), 1), Push::Closed);
        assert_eq!(q.push(Arc::new(Image::new(2, 2)), 1), Push::Closed);
    }
}
