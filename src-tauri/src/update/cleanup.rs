//! Removing installer folders that were left behind by earlier updates.

use super::install::updates_root;
use std::path::Path;
use std::time::{Duration, SystemTime};

const MAX_AGE: Duration = Duration::from_secs(24 * 3600);

/// Run at startup, off the UI thread: a failed or finished update may have left its folder behind.
pub fn remove_stale_runs_at_startup() {
    if let Ok(root) = updates_root() {
        remove_stale_runs(&root, SystemTime::now(), MAX_AGE);
    }
}

/// Deletes each folder under `root` whose newest entry is older than `max_age`. A folder that is still
/// locked (an installer running) cannot be removed and is skipped.
pub fn remove_stale_runs(root: &Path, now: SystemTime, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let dir = entry.path();
        let stale = last_used(&dir).is_some_and(|t| now.duration_since(t).is_ok_and(|age| age > max_age));
        if dir.is_dir() && stale {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

fn last_used(dir: &Path) -> Option<SystemTime> {
    let newest_file = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.metadata().and_then(|m| m.modified()).ok())
        .max();
    newest_file.or_else(|| dir.metadata().ok()?.modified().ok())
}
