//! The viewer thread: it connects to the host, binds the globals, reports the dmabuf formats of
//! the host, then waits in `poll` for host events and compositor messages until a window closes,
//! the host connection fails or the compositor stops it.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::thread::{self, JoinHandle};

use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::rustix::event::{PollFd, PollFlags, poll};
use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, EventQueue};

use super::app::{App, Globals};
use super::connect::connect;
use super::seat::AxisFrame;
use crate::viewer::mailbox::MailReceiver;
use crate::viewer::{FromViewer, ToViewer};

/// Starts the viewer thread. `display` names the host display; `None` takes `WAYLAND_DISPLAY`.
pub fn spawn(
    display: Option<String>,
    rx: MailReceiver<ToViewer>,
    tx: Sender<FromViewer>,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("fakeland-viewer".into())
        .spawn(move || {
            let end = run(display.as_deref(), &rx, tx.clone()).err();
            let _ = tx.send(FromViewer::Ended(end));
        })
}

fn run(
    display: Option<&str>,
    rx: &MailReceiver<ToViewer>,
    tx: Sender<FromViewer>,
) -> Result<(), String> {
    let conn = connect(display)?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let bind_err = |name: &str, e: wayland_client::globals::BindError| {
        format!("the host has no usable {name}: {e}")
    };
    let g = Globals {
        compositor: globals
            .bind(&qh, 4..=6, ())
            .map_err(|e| bind_err("wl_compositor", e))?,
        subcompositor: globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| bind_err("wl_subcompositor", e))?,
        wm_base: globals
            .bind(&qh, 1..=5, ())
            .map_err(|e| bind_err("xdg_wm_base", e))?,
        viewporter: globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| bind_err("wp_viewporter", e))?,
        shm: globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| bind_err("wl_shm", e))?,
        dmabuf: globals.bind(&qh, 3..=3, ()).ok(),
        decorations: globals.bind(&qh, 1..=1, ()).ok(),
    };
    let seat: Option<WlSeat> = globals.bind(&qh, 5..=8, ()).ok();
    if seat.is_none() {
        tracing::info!("the host has no seat; the viewer shows output only");
    }
    let mut app = App {
        tx,
        qh,
        globals: g,
        formats: Vec::new(),
        windows: HashMap::new(),
        surfaces: HashMap::new(),
        buffers: HashMap::new(),
        pointer: None,
        keyboard: None,
        pointer_on: None,
        pointer_surface: None,
        axis: AxisFrame::default(),
        key_output: None,
        keys: BTreeSet::new(),
        keymap: None,
        ended: None,
        next_shm: 0,
    };
    queue.roundtrip(&mut app).map_err(|e| e.to_string())?;
    app.send(FromViewer::Started {
        formats: app.formats.clone(),
    });
    loop {
        let (mail, closed) = rx.drain();
        for msg in mail {
            if !handle(&mut app, msg) {
                return Ok(());
            }
        }
        if closed {
            return Ok(());
        }
        if let Some(end) = app.ended.take() {
            return end.map_or(Ok(()), Err);
        }
        queue
            .dispatch_pending(&mut app)
            .map_err(|e| format!("host protocol error: {e}"))?;
        if let Some(end) = app.ended.take() {
            return end.map_or(Ok(()), Err);
        }
        wait(&conn, &mut queue, rx)?;
    }
}

/// Applies one compositor message. Returns false on stop.
fn handle(app: &mut App, msg: ToViewer) -> bool {
    match msg {
        ToViewer::Outputs(views) => {
            let ids: HashSet<_> = views.iter().map(|v| v.id).collect();
            let gone: Vec<_> = app
                .windows
                .keys()
                .filter(|id| !ids.contains(id))
                .copied()
                .collect();
            for id in gone {
                app.close_window(id);
            }
            for v in &views {
                if app.windows.contains_key(&v.id) {
                    app.update_window(v);
                } else {
                    app.open_window(v);
                }
            }
        }
        ToViewer::Frame { output, frame } => app.show(output, frame),
        ToViewer::Retire { output, ids } => app.retire(output, &ids),
        ToViewer::Title { output, title } => app.set_title(output, title),
        ToViewer::Stop => return false,
    }
    true
}

/// Flushes, then sleeps until the host or the compositor has something.
fn wait(
    conn: &Connection,
    queue: &mut EventQueue<App>,
    rx: &MailReceiver<ToViewer>,
) -> Result<(), String> {
    conn.flush().map_err(|e| format!("host connection: {e}"))?;
    let Some(guard) = queue.prepare_read() else {
        return Ok(());
    };
    let host_ready = {
        let (host_fd, mail_fd) = (guard.connection_fd(), rx.fd());
        let mut fds = [
            PollFd::new(&host_fd, PollFlags::IN),
            PollFd::new(&mail_fd, PollFlags::IN),
        ];
        match poll(&mut fds, None) {
            Ok(_) => {}
            Err(e) if e == smithay::reexports::rustix::io::Errno::INTR => return Ok(()),
            Err(e) => return Err(format!("poll: {e}")),
        }
        !fds[0].revents().is_empty()
    };
    if host_ready {
        match guard.read() {
            Ok(_) => {}
            Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(format!("host connection: {e}")),
        }
    } else {
        drop(guard);
    }
    Ok(())
}
