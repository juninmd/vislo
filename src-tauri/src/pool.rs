//! Small worker pool for image jobs. Newest request first (it is the one on screen), and the
//! backlog is bounded: when it overflows, the oldest job is told it was shed instead of piling up.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

type Job = Box<dyn FnOnce(bool) + Send>; // argument: true when the job was shed unrun

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    ready: Condvar,
    capacity: usize,
}

pub struct Pool(Arc<Shared>);

impl Pool {
    pub fn new(workers: usize, capacity: usize) -> Self {
        let shared = Arc::new(Shared { queue: Mutex::new(VecDeque::new()), ready: Condvar::new(), capacity });
        for _ in 0..workers.max(1) {
            let s = Arc::clone(&shared);
            std::thread::spawn(move || loop {
                let job = {
                    let mut q = s.queue.lock().unwrap_or_else(|e| e.into_inner());
                    loop {
                        if let Some(job) = q.pop_back() {
                            break job;
                        }
                        q = s.ready.wait(q).unwrap_or_else(|e| e.into_inner());
                    }
                };
                // A panicking job must not take its worker down with it.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(false)));
            });
        }
        Pool(shared)
    }

    pub fn spawn(&self, job: impl FnOnce(bool) + Send + 'static) {
        let shed = {
            let mut q = self.0.queue.lock().unwrap_or_else(|e| e.into_inner());
            q.push_back(Box::new(job));
            if q.len() > self.0.capacity { q.pop_front() } else { None }
        };
        self.0.ready.notify_one();
        if let Some(old) = shed {
            old(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_panicking_job_does_not_kill_the_worker() {
        let pool = Pool::new(1, 16);
        pool.spawn(|_| panic!("decoder bug"));
        let (tx, rx) = mpsc::channel();
        pool.spawn(move |_| tx.send(()).unwrap());
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "the only worker died");
    }

    #[test]
    fn runs_every_job() {
        let pool = Pool::new(2, 16);
        let (tx, rx) = mpsc::channel();
        for i in 0..8 {
            let tx = tx.clone();
            pool.spawn(move |shed| tx.send((i, shed)).unwrap());
        }
        let mut got: Vec<_> = (0..8).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        got.sort();
        assert!(got.iter().all(|(_, shed)| !shed));
        assert_eq!(got.len(), 8);
    }

    #[test]
    fn overflow_sheds_the_oldest_job_and_reports_it() {
        let pool = Pool::new(1, 2);
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let (started_tx, started_rx) = mpsc::channel();
        pool.spawn(move |_| {
            started_tx.send(()).unwrap();
            gate_rx.recv_timeout(Duration::from_secs(5)).unwrap(); // hold the only worker
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (tx, rx) = mpsc::channel();
        for i in 0..4 {
            let tx = tx.clone();
            pool.spawn(move |shed| tx.send((i, shed)).unwrap());
        }
        // capacity 2 and 4 queued: the two oldest (0, 1) are shed immediately
        let mut shed: Vec<_> = (0..2).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        shed.sort();
        assert_eq!(shed, vec![(0, true), (1, true)]);
        gate_tx.send(()).unwrap();
        let mut ran: Vec<_> = (0..2).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        ran.sort();
        assert_eq!(ran, vec![(2, false), (3, false)]);
    }
}
