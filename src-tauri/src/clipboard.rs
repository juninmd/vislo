//! Puts the picture on the system clipboard as pixels (what Ctrl+C means in a viewer).

use crate::resize;
use fast_image_resize::FilterType;
use image::DynamicImage;
use std::{borrow::Cow, path::Path, time::Duration};

const RETRIES: u32 = 4;
const RETRY_PAUSE: Duration = Duration::from_millis(120);

/// RGBA bytes allowed for the clipboard picture; the OS makes a second copy of it when it is set.
const BUDGET_BYTES: u64 = 256 << 20;
/// Longest side of a downscaled picture, which keeps it sharp enough to paste anywhere.
const MAX_SIDE: u32 = 8192;

#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    Original,
    Downscale { w: u32, h: u32 },
}

/// Whether a `width` x `height` picture goes to the clipboard as it is, and if not, the size it is shrunk to.
/// The target fits both the side cap and the byte budget, so the RGBA buffer never exceeds the budget.
pub fn copy_plan(width: u32, height: u32) -> Plan {
    let (w, h) = (width as u64, height as u64);
    // Nothing to shrink in an empty picture; the copy itself reports it.
    if w == 0 || h == 0 {
        return Plan::Original;
    }
    let bytes = w * h * 4;
    if w.max(h) <= MAX_SIDE as u64 && bytes <= BUDGET_BYTES {
        return Plan::Original;
    }
    // Area scales with the square of the factor, so the byte budget gives a square root.
    let by_side = MAX_SIDE as f64 / w.max(h) as f64;
    let by_budget = (BUDGET_BYTES as f64 / bytes as f64).sqrt();
    let k = by_side.min(by_budget);
    let (mut dw, mut dh) = (((w as f64 * k).round() as u64).clamp(1, w), ((h as f64 * k).round() as u64).clamp(1, h));
    // Rounding can leave one pixel too many on a side; trim until the budget holds exactly.
    while dw * dh * 4 > BUDGET_BYTES && (dw > 1 || dh > 1) {
        if dw >= dh {
            dw -= 1;
        } else {
            dh -= 1;
        }
    }
    Plan::Downscale { w: dw as u32, h: dh as u32 }
}

/// The picture to copy: the original, or a copy shrunk by `copy_plan`. Returns whether it was shrunk.
fn prepare(img: DynamicImage) -> Result<(DynamicImage, bool), String> {
    match copy_plan(img.width(), img.height()) {
        Plan::Original => Ok((img, false)),
        Plan::Downscale { w, h } => Ok((resize::fit_owned(img, w, h, FilterType::Lanczos3)?, true)),
    }
}

/// Copies the picture; returns its final size when it was downscaled to fit the clipboard budget.
pub fn copy_image(path: &Path) -> Result<Option<(u32, u32)>, String> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("avif")) {
        return Err("Copying AVIF images is not supported".into()); // no AVIF decoder in the backend
    }
    let (img, shrunk) = prepare(crate::decode::load(path)?)?;
    let (w, h) = (img.width(), img.height()); // the size actually produced, which can differ by a pixel from the plan
    let (width, height) = (w as usize, h as usize);
    let pixels = img.into_rgba8().into_raw();
    // Clipboard managers and remote sessions briefly hold the clipboard; a short retry rides that out.
    let mut last = String::new();
    for attempt in 0..RETRIES {
        if attempt > 0 {
            std::thread::sleep(RETRY_PAUSE);
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_image(arboard::ImageData { width, height, bytes: Cow::Borrowed(&pixels) })) {
            Ok(()) => return Ok(shrunk.then_some((w, h))),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pictures_are_not_planned_for_shrinking() {
        assert_eq!(copy_plan(0, 0), Plan::Original);
        assert_eq!(copy_plan(0, 5000), Plan::Original);
        assert_eq!(copy_plan(5000, 0), Plan::Original);
    }

    #[test]
    fn a_single_pixel_is_copied_as_it_is() {
        assert_eq!(copy_plan(1, 1), Plan::Original);
    }

    #[test]
    fn exactly_the_budget_and_the_side_cap_are_still_copied_whole() {
        assert_eq!(copy_plan(8192, 8192), Plan::Original); // 256 MiB of RGBA exactly
        assert_eq!(copy_plan(8192, 4096), Plan::Original);
        assert_eq!(copy_plan(1, 8192), Plan::Original);
    }

    #[test]
    fn one_pixel_over_the_side_cap_is_shrunk_below_it() {
        // 8193 px wide: the side cap alone decides, and the picture is far below the budget.
        assert_eq!(copy_plan(8193, 100), Plan::Downscale { w: 8192, h: 100 });
    }

    #[test]
    fn a_150_megapixel_picture_fits_the_budget_and_the_side_cap() {
        let Plan::Downscale { w, h } = copy_plan(15_000, 10_000) else { panic!("150 MP must shrink") };
        assert!(w <= MAX_SIDE && h <= MAX_SIDE);
        assert!(w as u64 * h as u64 * 4 <= BUDGET_BYTES);
        assert_eq!(w, 8192, "the side cap is the binding limit for a 3:2 picture of this size");
    }

    #[test]
    fn the_largest_square_is_halved_to_8192() {
        assert_eq!(copy_plan(16_384, 16_384), Plan::Downscale { w: 8192, h: 8192 });
    }

    #[test]
    fn extreme_aspect_ratios_keep_at_least_one_pixel_per_side() {
        assert_eq!(copy_plan(150_000_000, 1), Plan::Downscale { w: 8192, h: 1 });
        assert_eq!(copy_plan(1, 150_000_000), Plan::Downscale { w: 1, h: 8192 });
    }

    #[test]
    fn every_plan_stays_inside_both_limits() {
        for &(w, h) in &[(9000u32, 9000u32), (12_000, 10_000), (16_384, 16_384), (20_000, 7_500), (3, 60_000), (65_535, 2_300)] {
            if let Plan::Downscale { w: dw, h: dh } = copy_plan(w, h) {
                assert!(dw <= MAX_SIDE && dh <= MAX_SIDE, "{w}x{h} -> {dw}x{dh} exceeds the side cap");
                assert!(dw as u64 * dh as u64 * 4 <= BUDGET_BYTES, "{w}x{h} -> {dw}x{dh} exceeds the budget");
                assert!(dw >= 1 && dh >= 1);
            }
        }
    }

    /// Peak working set of the clipboard pipeline on a synthetic 120 MP picture (not run by default).
    /// `VISLO_PEAK_MODE=legacy` runs the old `to_rgba8` copy, `new` runs `prepare`, `none` only builds
    /// the source picture, so the difference against `none` is the pipeline's own cost.
    #[test]
    #[ignore]
    fn peak_probe_on_a_120_megapixel_picture() {
        let (w, h) = (12_000u32, 10_000u32);
        let raw = vec![0x7f_u8; w as usize * h as usize * 3];
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, raw).unwrap());
        let mode = std::env::var("VISLO_PEAK_MODE").unwrap_or_default();
        let started = std::time::Instant::now();
        let pixels = match mode.as_str() {
            "legacy" => img.to_rgba8().into_raw(),
            "new" => {
                let (out, shrunk) = prepare(img).unwrap();
                assert!(shrunk);
                out.into_rgba8().into_raw()
            }
            _ => {
                drop(img);
                Vec::new()
            }
        };
        println!("mode={mode} out_bytes={} elapsed_ms={}", pixels.len(), started.elapsed().as_millis());
    }
}
