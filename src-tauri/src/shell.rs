//! Hands a file to Windows: reveal it in Explorer or ask which app should open it.
//! Callers pass a path that already went through `Allowed::resolve`.

use std::path::Path;

/// Explorer parses `/select,"path"` itself; the quoted raw form is what it expects.
fn select_arg(path: &Path) -> String {
    format!("/select,\"{}\"", path.display())
}

#[cfg(windows)]
pub fn reveal(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer.exe")
        .raw_arg(select_arg(path))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
pub fn open_with(path: &Path) -> Result<(), String> {
    std::process::Command::new("rundll32.exe")
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn reveal(_path: &Path) -> Result<(), String> {
    Err("showing a file in its folder is Windows-only".into())
}

#[cfg(not(windows))]
pub fn open_with(_path: &Path) -> Result<(), String> {
    Err("the Open With dialog is Windows-only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_argument_quotes_paths_with_spaces() {
        assert_eq!(select_arg(Path::new("C:/My Photos/a b.jpg")), "/select,\"C:/My Photos/a b.jpg\"");
    }
}
