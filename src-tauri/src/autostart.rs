//! "Start ready when I sign in": a per-user Run entry that launches Vislo parked in the tray.

pub const VALUE: &str = "Vislo";

/// The Run-key command: quoted exe plus the flag that keeps the window hidden.
pub fn command(exe: &str) -> String {
    format!("\"{exe}\" --background")
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn apply_in(key: &str, exe: &str, enabled: bool) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if enabled {
            let (run, _) = hkcu.create_subkey(key).map_err(|e| e.to_string())?;
            run.set_value(VALUE, &command(exe))
                .map_err(|e| e.to_string())
        } else if let Ok(run) = hkcu.open_subkey_with_flags(key, winreg::enums::KEY_SET_VALUE) {
            match run.delete_value(VALUE) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    pub fn apply(enabled: bool) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        apply_in(RUN_KEY, &exe.to_string_lossy(), enabled)
    }

    #[cfg(test)]
    pub fn read_in(key: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(key)
            .ok()?
            .get_value(VALUE)
            .ok()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn apply(_enabled: bool) -> Result<(), String> {
        Ok(())
    }
}

pub use imp::apply;

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn enabling_writes_and_disabling_removes_the_run_value() {
        let key = format!(r"Software\VisloTestRun-{}", std::process::id());
        imp::apply_in(&key, r"C:\x\vislo.exe", true).unwrap();
        assert_eq!(
            imp::read_in(&key).as_deref(),
            Some(r#""C:\x\vislo.exe" --background"#)
        );
        imp::apply_in(&key, r"C:\x\vislo.exe", false).unwrap();
        assert_eq!(imp::read_in(&key), None);
        imp::apply_in(&key, r"C:\x\vislo.exe", false).unwrap(); // idempotent
        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).delete_subkey_all(&key);
    }
}
