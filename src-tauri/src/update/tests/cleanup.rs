use crate::update::cleanup::remove_stale_runs;
use std::time::{Duration, SystemTime};

const DAY: Duration = Duration::from_secs(24 * 3600);

#[test]
fn run_folders_untouched_for_a_day_are_removed_and_recent_ones_kept() {
    let root = std::env::temp_dir().join(format!("vislo-prune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (old, recent) = (root.join("old"), root.join("recent"));
    for dir in [&old, &recent] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("Vislo_0.2.0_x64-setup.exe"), b"x").unwrap();
    }
    let now = SystemTime::now();
    let aged = std::fs::OpenOptions::new().write(true).open(old.join("Vislo_0.2.0_x64-setup.exe")).unwrap();
    aged.set_modified(now - 2 * DAY).unwrap();

    remove_stale_runs(&root, now, DAY);

    assert!(!old.exists(), "a folder left behind by an old update must not stay on disk");
    assert!(recent.join("Vislo_0.2.0_x64-setup.exe").exists(), "a folder that may still be in use must stay");
    let _ = std::fs::remove_dir_all(&root);
}
