//! Pure layout and timing maths for the splash, kept apart from Win32 so it can be tested.

/// Zoom-in shared with the page's own `appear` animation (src/anim.ts): grows from `START_SCALE`
/// while fading in from `START_ALPHA`.
pub const START_SCALE: f32 = 0.94;
pub const START_ALPHA: f32 = 0.4;
pub const DURATION_MS: u64 = 320;
/// Dimming of the desktop behind the picture: `#app.bg-transparent` in src/style.css.
pub const DIM_ALPHA: f32 = 0.55;

/// Top-left corner that centres `size` in the area starting at `origin` with extent `area`.
pub fn centred(size: (u32, u32), origin: (i32, i32), area: (u32, u32)) -> (i32, i32) {
    (
        origin.0 + (area.0 as i32 - size.0 as i32) / 2,
        origin.1 + (area.1 as i32 - size.1 as i32) / 2,
    )
}

/// Ease-out cubic of linear progress `t` in 0..=1.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Scale and opacity at eased progress `p`.
pub fn pose(p: f32) -> (f32, f32) {
    (
        START_SCALE + (1.0 - START_SCALE) * p,
        START_ALPHA + (1.0 - START_ALPHA) * p,
    )
}

/// Straight RGBA -> premultiplied BGRA, the layout `UpdateLayeredWindow` blends.
pub fn premultiplied_bgra(rgba: &[u8], out: &mut [u8]) {
    for (src, dst) in rgba.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let a = u32::from(src[3]);
        let mul = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
        dst.copy_from_slice(&[mul(src[2]), mul(src[1]), mul(src[0]), src[3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplies_and_swaps_to_bgra() {
        let mut out = [0u8; 8];
        premultiplied_bgra(&[200, 100, 50, 255, 200, 100, 50, 128], &mut out);
        assert_eq!(out, [50, 100, 200, 255, 25, 50, 100, 128]);
    }

    #[test]
    fn centres_inside_the_work_area() {
        assert_eq!(centred((1856, 1392), (0, 0), (2560, 1392)), (352, 0));
        assert_eq!(centred((200, 100), (100, 40), (1000, 500)), (500, 240));
    }

    #[test]
    fn the_zoom_starts_small_and_dim_and_ends_exactly_at_full_size() {
        assert_eq!(pose(0.0), (START_SCALE, START_ALPHA));
        assert_eq!(pose(1.0), (1.0, 1.0));
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_out(0.5) > 0.5, "ease-out is front-loaded");
        assert_eq!(ease_out(7.0), 1.0);
    }
}
