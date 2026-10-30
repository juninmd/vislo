//! Streaming the installer to disk: no total time limit, but a stall is an error and a cancel is honoured.

use super::transport::{send, Source};
use serde::Serialize;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

const PIECE: usize = 64 * 1024;
/// How often the receive loop looks at the cancel flag and the idle clock.
const TICK: Duration = Duration::from_millis(100);
/// Progress reports are throttled to this interval.
const REPORT_EVERY: Duration = Duration::from_millis(250);

/// Bytes so far; `total` is the Content-Length when the server sends one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub received: u64,
    pub total: Option<u64>,
}

/// How the caller watches and stops a download.
pub struct Control<'a> {
    pub cancel: &'a AtomicBool,
    pub progress: &'a mut dyn FnMut(Progress),
}

/// Writes the body to `dest`. The body is read on its own thread so that silence on the socket can be
/// timed here: a blocked read cannot be interrupted, but a channel receive can.
pub(super) fn download(
    agent: &ureq::Agent,
    src: &Source,
    url: &str,
    dest: &Path,
    ctl: &mut Control,
) -> Result<(), String> {
    let response = send(agent, src, url, "application/octet-stream")?;
    let total = response.body().content_length();
    if total.is_some_and(|n| n > src.max_bytes) {
        return Err(too_large(src));
    }
    let (tx, rx) = mpsc::sync_channel(4);
    let body = response.into_body().into_reader();
    thread::spawn(move || pump(body, tx));

    let mut out = File::create(dest).map_err(|e| e.to_string())?;
    let mut received = 0u64;
    let mut idle_since = Instant::now();
    let mut reported: Option<Instant> = None;
    loop {
        if ctl.cancel.load(Ordering::SeqCst) {
            return Err("the update was cancelled".into());
        }
        match rx.recv_timeout(TICK) {
            Ok(Ok(piece)) => {
                idle_since = Instant::now();
                received += piece.len() as u64;
                if received > src.max_bytes {
                    return Err(too_large(src));
                }
                out.write_all(&piece).map_err(|e| e.to_string())?;
                if reported.is_none_or(|at| at.elapsed() >= REPORT_EVERY) {
                    (ctl.progress)(Progress { received, total });
                    reported = Some(Instant::now());
                }
            }
            Ok(Err(e)) => return Err(format!("the download was interrupted: {e}")),
            Err(RecvTimeoutError::Timeout) if idle_since.elapsed() >= src.timeouts.inactivity => {
                return Err("the download stalled: no data arrived for a while; check your connection and try again".into())
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    if total.is_some_and(|n| n != received) {
        return Err("the download was interrupted before it finished".into());
    }
    (ctl.progress)(Progress { received, total });
    out.flush().map_err(|e| e.to_string())
}

fn pump(mut body: impl Read, tx: SyncSender<io::Result<Vec<u8>>>) {
    loop {
        let mut piece = vec![0u8; PIECE];
        match body.read(&mut piece) {
            Ok(0) => return, // dropping `tx` tells the receiver the body is complete
            Ok(n) => {
                piece.truncate(n);
                if tx.send(Ok(piece)).is_err() {
                    return; // the receiver gave up
                }
            }
            Err(e) => {
                let _ = tx.send(Err(e));
                return;
            }
        }
    }
}

fn too_large(src: &Source) -> String {
    format!("the installer is larger than the {}-byte size limit", src.max_bytes)
}
