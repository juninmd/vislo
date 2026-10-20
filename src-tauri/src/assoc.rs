//! Per-user "default app" registration (HKCU, no admin rights).
//!
//! Windows 10/11 will not let an app make itself the default for a file type; the supported
//! path is to register the app (ProgID + Capabilities + RegisteredApplications) and send the
//! user to Settings > Default apps for a single confirmation.

use crate::album::extensions;

/// What the user can pick in the settings panel; together they cover `extensions()` exactly.
pub const GROUPS: &[(&str, &[&str])] = &[
    ("jpeg", &["jpg", "jpeg", "jpe", "jfif"]),
    ("png", &["png"]),
    ("gif", &["gif"]),
    ("bmp", &["bmp"]),
    ("tiff", &["tif", "tiff"]),
    ("tga", &["tga"]),
    ("webp", &["webp"]),
    ("avif", &["avif"]),
    ("ico", &["ico"]),
    ("psd", &["psd"]),
    ("raw", &["dng", "cr2", "nef", "arw", "orf", "rw2"]),
];

/// Extensions of the named groups; no names means every group.
pub fn extensions_for(groups: &[String]) -> Result<Vec<&'static str>, String> {
    if groups.is_empty() {
        return Ok(extensions().collect());
    }
    let mut out = Vec::new();
    for name in groups {
        let (_, exts) = GROUPS.iter().find(|(g, _)| g == name).ok_or_else(|| format!("unknown format group \"{name}\""))?;
        out.extend(exts.iter().copied());
    }
    Ok(out)
}

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
pub fn plan(exe: &str, classes: &str, app_root: &str, exts: &[&str]) -> Vec<Entry> {
    let prog = format!(r"{classes}\{PROG_ID}");
    let mut out = vec![
        entry(prog.clone(), "", "Image file"),
        entry(format!(r"{prog}\DefaultIcon"), "", &format!("\"{exe}\",0")),
        entry(format!(r"{prog}\shell\open\command"), "", &format!("\"{exe}\" \"%1\"")),
        entry(format!(r"{app_root}\Capabilities"), "ApplicationName", APP_NAME),
        entry(format!(r"{app_root}\Capabilities"), "ApplicationDescription", DESCRIPTION),
    ];
    for ext in exts {
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

    /// Forgets a previous registration's extension list so a smaller selection really shrinks.
    fn clear_selection(classes: &str, app_root: &str) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for ext in extensions() {
            if let Ok(k) = hkcu.open_subkey_with_flags(format!(r"{classes}\.{ext}\OpenWithProgids"), winreg::enums::KEY_SET_VALUE) {
                let _ = k.delete_value(PROG_ID);
            }
        }
        let _ = hkcu.delete_subkey_all(format!(r"{app_root}\Capabilities\FileAssociations"));
    }

    /// Writes the registration for `groups` under the given roots (scratch ones in tests).
    pub fn register_in(exe: &str, groups: &[String], classes: &str, app_root: &str) -> Result<(), String> {
        let exts = extensions_for(groups)?;
        clear_selection(classes, app_root);
        apply(&plan(exe, classes, app_root, &exts))
    }

    pub fn register(groups: &[String]) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        register_in(&exe.to_string_lossy(), groups, CLASSES, APP_ROOT)?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (apps, _) = hkcu.create_subkey(REGISTERED_APPS).map_err(|e| e.to_string())?;
        apps.set_value(APP_NAME, &format!(r"{APP_ROOT}\Capabilities")).map_err(|e| e.to_string())
    }

    /// Removes only what `register` created; the user's chosen defaults for other apps stay.
    pub fn remove(classes: &str, app_root: &str) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for ext in extensions() {
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
    pub fn register(_groups: &[String]) -> Result<(), String> { Err("default-app registration is Windows-only".into()) }
    pub fn unregister() -> Result<(), String> { Err("default-app registration is Windows-only".into()) }
    pub fn open_settings() -> Result<(), String> { Ok(()) }
}

pub use imp::{open_settings, register, unregister};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_registers_open_command_and_every_extension() {
        let p = plan(r"C:\Apps\Vislo\vislo.exe", r"Software\Classes", r"Software\Vislo", &extensions().collect::<Vec<_>>());
        let cmd = p.iter().find(|e| e.key.ends_with(r"shell\open\command")).unwrap();
        assert_eq!(cmd.value, r#""C:\Apps\Vislo\vislo.exe" "%1""#);
        for ext in extensions() {
            assert!(p.iter().any(|e| e.key == format!(r"Software\Classes\.{ext}\OpenWithProgids") && e.name == PROG_ID), "{ext}");
            assert!(p.iter().any(|e| e.key.ends_with("FileAssociations") && e.name == format!(".{ext}")), "{ext}");
        }
    }

    #[test]
    fn plan_never_touches_the_default_value_of_an_extension_key() {
        // Taking over `.jpg` itself would clobber the user's current default app.
        let p = plan("x.exe", r"Software\Classes", r"Software\Vislo", &["jpg"]);
        assert!(p.iter().all(|e| !(e.key.starts_with(r"Software\Classes\.") && !e.key.ends_with("OpenWithProgids"))));
    }

    #[cfg(windows)]
    #[test]
    fn register_and_remove_round_trip_in_a_scratch_hive() {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let root = format!(r"Software\VisloTest-{}", std::process::id());
        let (classes, app) = (format!(r"{root}\Classes"), format!(r"{root}\App"));
        imp::apply(&plan(r"C:\x\vislo.exe", &classes, &app, &["jpg"])).unwrap();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let cmd: String = hkcu.open_subkey(format!(r"{classes}\{PROG_ID}\shell\open\command")).unwrap().get_value("").unwrap();
        assert_eq!(cmd, r#""C:\x\vislo.exe" "%1""#);
        imp::remove(&classes, &app).unwrap();
        assert!(hkcu.open_subkey(format!(r"{classes}\{PROG_ID}")).is_err());
        assert!(hkcu.open_subkey(&app).is_err());
        imp::remove(&classes, &app).unwrap(); // idempotent
        let _ = hkcu.delete_subkey_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn registering_fewer_groups_drops_the_ones_left_out() {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let root = format!(r"Software\VisloTestGroups-{}", std::process::id());
        let (classes, app) = (format!(r"{root}\Classes"), format!(r"{root}\App"));
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let has = |ext: &str| hkcu.open_subkey(format!(r"{classes}\.{ext}\OpenWithProgids")).and_then(|k| k.get_value::<String, _>(PROG_ID)).is_ok();
        imp::register_in("x.exe", &[], &classes, &app).unwrap();
        assert!(has("jpg") && has("png"));
        imp::register_in("x.exe", &["png".into()], &classes, &app).unwrap();
        assert!(has("png") && !has("jpg"));
        let listed: Vec<String> = hkcu.open_subkey(format!(r"{app}\Capabilities\FileAssociations")).unwrap().enum_values().map(|v| v.unwrap().0).collect();
        assert_eq!(listed, [".png"]);
        let _ = hkcu.delete_subkey_all(&root);
    }

    #[test]
    fn groups_cover_every_supported_extension_exactly_once() {
        let mut all: Vec<_> = GROUPS.iter().flat_map(|(_, e)| e.iter().copied()).collect();
        all.sort_unstable();
        let mut want: Vec<_> = extensions().collect();
        want.sort_unstable();
        assert_eq!(all, want);
    }

    #[test]
    fn selection_maps_groups_to_extensions() {
        assert_eq!(extensions_for(&["tiff".into(), "png".into()]).unwrap(), ["tif", "tiff", "png"]);
        assert_eq!(extensions_for(&[]).unwrap().len(), extensions().count());
        assert!(extensions_for(&["nope".into()]).is_err());
    }

    #[test]
    fn plan_only_lists_the_selected_extensions() {
        let p = plan("x.exe", r"Software\Classes", r"Software\Vislo", &["png"]);
        assert!(p.iter().any(|e| e.name == ".png"));
        assert!(!p.iter().any(|e| e.name == ".jpg" || e.key.contains(".jpg")));
    }
}
