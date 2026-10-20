//! Per-user "default app" registration (HKCU, no admin rights).
//!
//! Windows 10/11 will not let an app make itself the default for a file type; the supported
//! path is to register the app (ProgID + Capabilities + RegisteredApplications) and send the
//! user to Settings > Default apps for a single confirmation.

use crate::album::EXTENSIONS;

pub const APP_NAME: &str = "Vislo";
pub const PROG_ID: &str = "Vislo.Image";
const DESCRIPTION: &str = "Fast, minimal image viewer";

/// One registry value to write; `name` empty means the key's default value.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub name: String,
    pub value: String,
}

fn entry(key: String, name: &str, value: &str) -> Entry {
    Entry { key, name: name.into(), value: value.into() }
}

/// Everything `register` writes, relative to HKCU. `classes` and `app_root` are parameters so
/// tests can target a scratch location instead of the real `Software\Classes`.
pub fn plan(exe: &str, classes: &str, app_root: &str) -> Vec<Entry> {
    let prog = format!(r"{classes}\{PROG_ID}");
    let mut out = vec![
        entry(prog.clone(), "", "Image file"),
        entry(format!(r"{prog}\DefaultIcon"), "", &format!("\"{exe}\",0")),
        entry(format!(r"{prog}\shell\open\command"), "", &format!("\"{exe}\" \"%1\"")),
        entry(format!(r"{app_root}\Capabilities"), "ApplicationName", APP_NAME),
        entry(format!(r"{app_root}\Capabilities"), "ApplicationDescription", DESCRIPTION),
    ];
    for ext in EXTENSIONS {
        out.push(entry(format!(r"{classes}\.{ext}\OpenWithProgids"), PROG_ID, ""));
        out.push(entry(format!(r"{app_root}\Capabilities\FileAssociations"), &format!(".{ext}"), PROG_ID));
    }
    out
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    pub const CLASSES: &str = r"Software\Classes";
    pub const APP_ROOT: &str = r"Software\Vislo";
    const REGISTERED_APPS: &str = r"Software\RegisteredApplications";

    pub fn apply(entries: &[Entry]) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for e in entries {
            let (key, _) = hkcu.create_subkey(&e.key).map_err(|err| format!("{}: {err}", e.key))?;
            key.set_value(&e.name, &e.value).map_err(|err| format!("{}: {err}", e.key))?;
        }
        Ok(())
    }

    pub fn register() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        apply(&plan(&exe.to_string_lossy(), CLASSES, APP_ROOT))?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (apps, _) = hkcu.create_subkey(REGISTERED_APPS).map_err(|e| e.to_string())?;
        apps.set_value(APP_NAME, &format!(r"{APP_ROOT}\Capabilities")).map_err(|e| e.to_string())
    }

    /// Removes only what `register` created; the user's chosen defaults for other apps stay.
    pub fn remove(classes: &str, app_root: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for ext in EXTENSIONS {
            if let Ok(k) = hkcu.open_subkey_with_flags(format!(r"{classes}\.{ext}\OpenWithProgids"), winreg::enums::KEY_SET_VALUE) {
                let _ = k.delete_value(PROG_ID);
            }
        }
        for tree in [format!(r"{classes}\{PROG_ID}"), app_root.to_string()] {
            match hkcu.delete_subkey_all(&tree) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{tree}: {e}")),
            }
        }
        Ok(())
    }

    pub fn unregister() -> Result<(), String> {
        remove(CLASSES, APP_ROOT)?;
        if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(REGISTERED_APPS, winreg::enums::KEY_SET_VALUE) {
            let _ = k.delete_value(APP_NAME);
        }
        Ok(())
    }

    /// Opens Settings > Default apps so the user can confirm Vislo for the image types.
    pub fn open_settings() -> Result<(), String> {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:defaultapps")
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn register() -> Result<(), String> { Err("default-app registration is Windows-only".into()) }
    pub fn unregister() -> Result<(), String> { Err("default-app registration is Windows-only".into()) }
    pub fn open_settings() -> Result<(), String> { Ok(()) }
}

pub use imp::{open_settings, register, unregister};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_registers_open_command_and_every_extension() {
        let p = plan(r"C:\Apps\Vislo\vislo.exe", r"Software\Classes", r"Software\Vislo");
        let cmd = p.iter().find(|e| e.key.ends_with(r"shell\open\command")).unwrap();
        assert_eq!(cmd.value, r#""C:\Apps\Vislo\vislo.exe" "%1""#);
        for ext in EXTENSIONS {
            assert!(p.iter().any(|e| e.key == format!(r"Software\Classes\.{ext}\OpenWithProgids") && e.name == PROG_ID), "{ext}");
            assert!(p.iter().any(|e| e.key.ends_with("FileAssociations") && e.name == format!(".{ext}")), "{ext}");
        }
    }

    #[test]
    fn plan_never_touches_the_default_value_of_an_extension_key() {
        // Taking over `.jpg` itself would clobber the user's current default app.
        let p = plan("x.exe", r"Software\Classes", r"Software\Vislo");
        assert!(p.iter().all(|e| !(e.key.starts_with(r"Software\Classes\.") && !e.key.ends_with("OpenWithProgids"))));
    }

    #[cfg(windows)]
    #[test]
    fn register_and_remove_round_trip_in_a_scratch_hive() {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let root = format!(r"Software\VisloTest-{}", std::process::id());
        let (classes, app) = (format!(r"{root}\Classes"), format!(r"{root}\App"));
        imp::apply(&plan(r"C:\x\vislo.exe", &classes, &app)).unwrap();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let cmd: String = hkcu.open_subkey(format!(r"{classes}\{PROG_ID}\shell\open\command")).unwrap().get_value("").unwrap();
        assert_eq!(cmd, r#""C:\x\vislo.exe" "%1""#);
        imp::remove(&classes, &app).unwrap();
        assert!(hkcu.open_subkey(format!(r"{classes}\{PROG_ID}")).is_err());
        assert!(hkcu.open_subkey(&app).is_err());
        imp::remove(&classes, &app).unwrap(); // idempotent
        let _ = hkcu.delete_subkey_all(&root);
    }
}
