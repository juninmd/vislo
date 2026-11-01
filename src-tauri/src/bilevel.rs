//! Black-and-white document formats: JBIG2 (`.jb2`, `.jbig2`) and CALS raster (`.cals`, Group 4 fax data
//! behind a 2048-byte header). Both decoders are the pure-Rust `hayro` ones; sizes are checked before pixels exist.

use hayro_ccitt::{DecodeSettings, DecoderContext, EncodingMode};
use image::{DynamicImage, GrayImage, ImageDecoder};
use std::path::Path;

pub fn decode_jbig2(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let image = hayro_jbig2::Image::new(&bytes).map_err(|e| e.to_string())?;
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    DynamicImage::from_decoder(image).map_err(|e| e.to_string())
}

/// `(width, height)` from the `rpelcnt: 001728,002200` record of a CALS header.
fn cals_size(header: &[u8]) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(header).ok()?;
    let rec = text.as_bytes().chunks(128).filter_map(|r| std::str::from_utf8(r).ok()).find(|r| r.starts_with("rpelcnt:"))?;
    let (w, h) = rec["rpelcnt:".len()..].trim().split_once(',')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

struct Pixels {
    buf: Vec<u8>,
    pos: usize,
    lines: u32,
}

impl Pixels {
    fn put(&mut self, white: bool, n: usize) {
        let end = (self.pos + n).min(self.buf.len());
        self.buf[self.pos.min(end)..end].fill(if white { 255 } else { 0 });
        self.pos = end;
    }
}

impl hayro_ccitt::Decoder for Pixels {
    fn push_pixels(&mut self, white: bool, count: u32) {
        self.put(white, count as usize);
    }
    fn next_line(&mut self) {
        self.lines += 1;
    }
}

pub fn decode_cals(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid CALS raster file".to_string();
    let header = bytes.get(..2048).filter(|h| h.starts_with(b"srcdocid:")).ok_or_else(bad)?;
    let (w, h) = cals_size(header).ok_or_else(bad)?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let settings = DecodeSettings { columns: w, rows: h, end_of_block: true, end_of_line: false, rows_are_byte_aligned: false, encoding: EncodingMode::Group4, invert_black: false };
    let mut pixels = Pixels { buf: vec![0; w as usize * h as usize], pos: 0, lines: 0 };
    hayro_ccitt::decode(&bytes[2048..], &mut pixels, &mut DecoderContext::new(settings)).map_err(|e| e.to_string())?;
    if pixels.lines < h {
        return Err(format!("truncated CALS data: {} of {h} rows", pixels.lines));
    }
    GrayImage::from_raw(w, h, pixels.buf).map(DynamicImage::ImageLuma8).ok_or_else(bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    // JBIG2 sequential file, 32x23, from the hayro-jbig2 test suite (Apache-2.0 OR MIT).
    const JBIG2: &str = "974a42320d0a1a0a01000000010000000030000100000013000000200000001700001ec200001ec2010000000000012700010000005300000020000000170000000000000000000003fffdff02fefefef6c9823073fddc0ff22f8847f28db10ed7a5570a595f337aa158e2931167f012f290576b6399f83ae50619061110f04f99db33c8995a2bffac0000000231000100000000";

    fn file(name: &str, data: &[u8]) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-bilevel-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(name), data).unwrap();
        d.join(name)
    }

    fn bits(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| *c == '0' || *c == '1').collect();
        s.as_bytes().chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | (u8::from(*b == b'1') << (7 - i)))).collect()
    }

    fn cals(w: u32, h: u32, g4: &[u8]) -> Vec<u8> {
        let mut head = Vec::new();
        for rec in ["srcdocid: NONE".to_string(), "rtype: 1".to_string(), format!("rpelcnt: {w:06},{h:06}")] {
            head.extend(format!("{rec:<128}").into_bytes());
        }
        head.resize(2048, b' ');
        head.extend(g4);
        head
    }

    #[test]
    fn jbig2_decodes_a_standalone_file() {
        let bytes = crate::codecs::tests::bytes(JBIG2);
        let img = decode_jbig2(&file("a.jb2", &bytes), 10_000).unwrap();
        assert_eq!(img.dimensions(), (32, 23));
        assert!(img.to_luma8().pixels().any(|p| p.0[0] == 0) && img.to_luma8().pixels().any(|p| p.0[0] == 255), "both ink and paper");
        assert_eq!(decode_jbig2(&file("b.jb2", &bytes), 100).unwrap_err(), crate::decode::TOO_LARGE);
        assert!(decode_jbig2(&file("c.jb2", b"not a jbig2 stream"), 100).is_err());
    }

    #[test]
    fn cals_group4_rows_decode_to_black_and_white() {
        // Row 1: horizontal mode, white run 0, black run 3, then V0 to the end; row 2 repeats it with V0 x3; then EOFB.
        let g4 = bits("001 00110101 10 1  111  000000000001 000000000001");
        let img = decode_cals(&file("a.cal", &cals(8, 2, &g4)), 100).unwrap();
        assert_eq!(img.dimensions(), (8, 2));
        let g = img.to_luma8();
        for y in 0..2 {
            let row: Vec<u8> = (0..8).map(|x| g.get_pixel(x, y).0[0]).collect();
            assert_eq!(row, [0, 0, 0, 255, 255, 255, 255, 255], "row {y}");
        }
    }

    #[test]
    fn cals_junk_missing_size_and_huge_headers_are_errors() {
        assert!(decode_cals(&file("a.cal", b"nope"), 100).is_err());
        let mut no_size = cals(8, 2, &[]);
        no_size[256..384].fill(b' ');
        assert!(decode_cals(&file("b.cal", &no_size), 100).is_err());
        assert_eq!(decode_cals(&file("c.cal", &cals(60_000, 60_000, &[])), 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
        assert!(decode_cals(&file("d.cal", &cals(8, 50, &[0xff])), 1_000).is_err(), "8 rows of data for a 50 row header");
    }
}
