//! Disk-backed thumbnail cache. Keys include the file's size and mtime, so edits invalidate.

use crate::{decode, resize};
use fast_image_resize::FilterType;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub const MIN_SIZE: u32 = 64;
pub const MAX_SIZE: u32 = 1024;

/// FNV-1a: tiny and stable across Rust versions, unlike `DefaultHasher`.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

pub fn cache_key(path: &Path, len: u64, mtime_ns: u128, size: u32) -> String {
    let raw = format!("{}|{len}|{mtime_ns}|{size}", path.to_string_lossy());
    format!("{:016x}", fnv1a(raw.as_bytes()))
}

fn cache_file(dir: &Path, src: &Path, size: u32) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    Ok(dir.join(format!("{}.jpg", cache_key(src, meta.len(), mtime, size))))
}

/// A cache entry is trusted only if it is a whole JPEG: a crash or full disk can leave a truncated file.
fn is_complete_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) && bytes.ends_with(&[0xFF, 0xD9])
}

/// JPEG thumbnail whose longest side is at most `size`, generated on first use.
pub fn thumbnail(cache_dir: &Path, src: &Path, size: u32) -> Result<Vec<u8>, String> {
    let size = size.clamp(MIN_SIZE, MAX_SIZE);
    let file = cache_file(cache_dir, src, size)?;
    if let Some(hit) = std::fs::read(&file).ok().filter(|b| is_complete_jpeg(b)) {
        return Ok(hit);
    }
    let small = resize::fit_owned(decode::load_fit(src, size, size)?, size, size, FilterType::CatmullRom)?;
    let jpeg = resize::to_jpeg(&small, 80)?;
    // The cache is an optimisation: a failed write must not fail the request, but is reported.
    if let Err(e) = write_atomic(&file, &jpeg) {
        eprintln!("vislo: thumbnail cache write failed: {e}");
    }
    Ok(jpeg)
}

/// Process-wide sequence: worker threads writing the same thumbnail must not share a temp file.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn tmp_path(file: &Path) -> PathBuf {
    file.with_extension(format!("tmp{}-{}", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed)))
}

fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = tmp_path(file);
    let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, file));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Deletes the oldest cached files until the directory is back under 80% of `max_bytes`.
pub fn evict(dir: &Path, max_bytes: u64) -> std::io::Result<usize> {
    let Ok(read) = std::fs::read_dir(dir) else { return Ok(0) };
    let mut files: Vec<_> = read
        .filter_map(Result::ok)
        .filter_map(|e| Some((e.path(), e.metadata().ok()?)))
        .filter(|(_, m)| m.is_file())
        .map(|(p, m)| (p, m.len(), m.modified().unwrap_or(UNIX_EPOCH)))
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= max_bytes {
        return Ok(0);
    }
    files.sort_by_key(|f| f.2);
    let target = max_bytes / 10 * 8;
    let mut removed = 0;
    for (path, len, _) in files {
        if total <= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-thumbs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn key_changes_with_size_mtime_and_path() {
        let base = cache_key(Path::new("a.jpg"), 10, 5, 256);
        assert_eq!(base, cache_key(Path::new("a.jpg"), 10, 5, 256));
        assert_ne!(base, cache_key(Path::new("a.jpg"), 11, 5, 256));
        assert_ne!(base, cache_key(Path::new("a.jpg"), 10, 6, 256));
        assert_ne!(base, cache_key(Path::new("a.jpg"), 10, 5, 512));
        assert_ne!(base, cache_key(Path::new("b.jpg"), 10, 5, 256));
    }

    #[test]
    fn fnv1a_matches_the_reference_vector() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn thumbnail_is_generated_once_then_served_from_disk() {
        let (src, cache) = (temp("src"), temp("cache"));
        let img = src.join("big.png");
        image::RgbImage::from_pixel(1200, 800, image::Rgb([10, 200, 90])).save(&img).unwrap();
        let first = thumbnail(&cache, &img, 256).unwrap();
        assert_eq!(image::load_from_memory(&first).unwrap().dimensions(), (256, 171));
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
        std::fs::remove_file(&img).unwrap(); // a hit must not need the source any more... but keys need metadata
        assert!(thumbnail(&cache, &img, 256).is_err(), "missing source is an error, not a stale hit");
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn corrupt_cache_entries_are_regenerated_not_served() {
        let (src, cache) = (temp("corrupt-src"), temp("corrupt-cache"));
        let img = src.join("photo.jpg");
        image::RgbImage::from_pixel(320, 240, image::Rgb([10, 200, 90])).save(&img).unwrap();
        let good = thumbnail(&cache, &img, 256).unwrap();
        let file = cache_file(&cache, &img, 256).unwrap();
        // Empty (crash after create), garbage, and truncated (SOI present, EOI missing) entries.
        for bad in [Vec::new(), b"not a jpeg".to_vec(), good[..good.len() / 2].to_vec()] {
            std::fs::write(&file, &bad).unwrap();
            assert_eq!(thumbnail(&cache, &img, 256).unwrap(), good, "a {}-byte entry must not be served", bad.len());
            assert_eq!(std::fs::read(&file).unwrap(), good, "the bad entry is overwritten on disk");
        }
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn temp_files_never_share_a_name() {
        // Shared temp names let two workers truncate each other's partial write.
        let file = Path::new("cache/abc.jpg");
        assert_ne!(tmp_path(file), tmp_path(file));
    }

    #[test]
    fn concurrent_misses_leave_a_decodable_cache_file() {
        let (src, cache) = (temp("race-src"), temp("race-cache"));
        let img = src.join("photo.jpg");
        let pixels = image::RgbImage::from_fn(320, 240, |x, y| image::Rgb([x as u8, y as u8, ((x ^ y) & 0xff) as u8]));
        pixels.save(&img).unwrap();
        for round in 0..50 {
            let _ = std::fs::remove_dir_all(&cache);
            std::fs::create_dir_all(&cache).unwrap();
            let barrier = std::sync::Barrier::new(16);
            std::thread::scope(|s| {
                for _ in 0..16 {
                    s.spawn(|| {
                        barrier.wait();
                        thumbnail(&cache, &img, 256).unwrap();
                    });
                }
            });
            let file = cache_file(&cache, &img, 256).unwrap();
            let bytes = std::fs::read(&file).unwrap_or_default();
            assert!(image::load_from_memory(&bytes).is_ok(), "round {round}: cache file is {} bytes and not a JPEG", bytes.len());
        }
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn evict_removes_oldest_files_until_under_budget() {
        let dir = temp("evict");
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let f = dir.join(format!("{name}.jpg"));
            std::fs::write(&f, vec![0u8; 100]).unwrap();
            let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            std::fs::File::options().write(true).open(&f).unwrap().set_modified(t).unwrap();
        }
        assert_eq!(evict(&dir, 1000).unwrap(), 0);
        assert_eq!(evict(&dir, 250).unwrap(), 1); // 300 > 250 -> target 200: the oldest alone is enough
        assert!(!dir.join("old.jpg").exists() && dir.join("mid.jpg").exists());
        assert_eq!(evict(&dir, 150).unwrap(), 1); // 200 > 150 -> target 120: next oldest goes, newest stays
        assert!(!dir.join("mid.jpg").exists() && dir.join("new.jpg").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
