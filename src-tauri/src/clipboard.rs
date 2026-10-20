//! Puts the picture on the system clipboard as pixels (what Ctrl+C means in a viewer).

use std::{borrow::Cow, path::Path, time::Duration};

const RETRIES: u32 = 4;
const RETRY_PAUSE: Duration = Duration::from_millis(120);

pub fn copy_image(path: &Path) -> Result<(), String> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("avif")) {
        return Err("Copying AVIF images is not supported".into()); // no AVIF decoder in the backend
    }
    let rgba = crate::decode::load(path)?.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let pixels = rgba.into_raw();
    // Clipboard managers and remote sessions briefly hold the clipboard; a short retry rides that out.
    let mut last = String::new();
    for attempt in 0..RETRIES {
        if attempt > 0 {
            std::thread::sleep(RETRY_PAUSE);
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_image(arboard::ImageData { width, height, bytes: Cow::Borrowed(&pixels) })) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}
