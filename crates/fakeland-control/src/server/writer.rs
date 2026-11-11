//! The writer thread of a connection. Lines go through a bounded queue, so a slow client slows
//! its own senders and never the compositor.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Duration;

use serde::Serialize;

use crate::wire::encode_line;

/// Lines that wait for the writer.
const QUEUE: usize = 1024;

/// A message for the writer thread.
#[derive(Debug)]
pub enum WriterMsg {
    /// One encoded line.
    Line(String),
    /// Signals the sender once every earlier line is written.
    Flush(std::sync::mpsc::Sender<()>),
}

/// The sending side of a connection.
#[derive(Debug, Clone)]
pub struct Outbox {
    tx: SyncSender<WriterMsg>,
}

impl Outbox {
    /// Queues a message as one line. Returns false when the connection has closed.
    pub fn send<T: Serialize>(&self, msg: &T) -> bool {
        match encode_line(msg) {
            Ok(line) => self.tx.send(WriterMsg::Line(line)).is_ok(),
            Err(e) => {
                tracing::warn!("cannot encode a control message: {e}");
                true
            }
        }
    }

    /// Waits until every queued line is written, up to a limit.
    pub fn flush(&self, limit: Duration) {
        let (ack, done) = std::sync::mpsc::channel();
        if self.tx.send(WriterMsg::Flush(ack)).is_ok() {
            let _ = done.recv_timeout(limit);
        }
    }
}

/// Starts the writer thread of a stream.
pub fn spawn_writer(stream: UnixStream) -> std::io::Result<Outbox> {
    let (tx, rx) = sync_channel(QUEUE);
    std::thread::Builder::new()
        .name("fakeland-ctl-write".into())
        .spawn(move || write_loop(stream, &rx))?;
    Ok(Outbox { tx })
}

fn write_loop(mut stream: UnixStream, rx: &Receiver<WriterMsg>) {
    while let Ok(msg) = rx.recv() {
        match msg {
            WriterMsg::Line(line) => {
                if stream.write_all(line.as_bytes()).is_err() {
                    break;
                }
            }
            WriterMsg::Flush(ack) => {
                let _ = stream.flush();
                let _ = ack.send(());
            }
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);
}
