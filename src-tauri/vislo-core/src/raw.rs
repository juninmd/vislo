//! RAW files (DNG, CR2, NEF, ARW, ORF, RW2) are TIFF containers that embed a full-size JPEG
//! preview. Viewers show that preview; Vislo does too and never develops the sensor data.

const MAX_CANDIDATES: usize = 64;

/// Length of the JPEG starting at `d[0]` (which must be `FF D8`), walking its marker segments.
fn jpeg_len(d: &[u8]) -> Option<usize> {
    let mut i = 2;
    loop {
        if i + 2 > d.len() || d[i] != 0xFF {
            return None;
        }
        let marker = d[i + 1];
        match marker {
            0xD9 => return Some(i + 2),
            0xD0..=0xD8 | 0x01 => i += 2,
            _ => {
                if i + 4 > d.len() {
                    return None;
                }
                let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
                i += 2 + len;
                if marker == 0xDA {
                    // entropy-coded data runs until the next real marker (not stuffing or RSTn)
                    while i + 1 < d.len() && !(d[i] == 0xFF && d[i + 1] != 0x00 && !(0xD0..=0xD7).contains(&d[i + 1])) {
                        i += 1;
                    }
                }
            }
        }
    }
}

/// The largest complete JPEG embedded in `data`, if any.
pub fn largest_embedded_jpeg(data: &[u8]) -> Option<&[u8]> {
    let mut best: Option<&[u8]> = None;
    let (mut pos, mut seen) = (0, 0);
    while pos + 3 <= data.len() && seen < MAX_CANDIDATES {
        if data[pos..pos + 3] != [0xFF, 0xD8, 0xFF] {
            pos += 1;
            continue;
        }
        seen += 1;
        match jpeg_len(&data[pos..]) {
            Some(len) => {
                if best.is_none_or(|b| len > b.len()) {
                    best = Some(&data[pos..pos + len]);
                }
                pos += len; // skip nested previews (e.g. a thumbnail inside APP1)
            }
            None => pos += 3,
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h * 3).map(|i| (i % 251) as u8).collect();
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 80).encode(&px, w, h, ExtendedColorType::Rgb8).unwrap();
        out
    }

    #[test]
    fn picks_the_largest_embedded_jpeg_among_noise() {
        let (small, big) = (jpeg(16, 16), jpeg(96, 64));
        let mut blob = vec![0x49, 0x49, 0x2A, 0x00, 0xFF, 0xD8, 0x00, 0x13]; // fake TIFF header + false SOI
        blob.extend(&small);
        blob.extend([0u8; 37]);
        blob.extend(&big);
        blob.extend([7u8; 20]);
        assert_eq!(largest_embedded_jpeg(&blob), Some(&big[..]));
    }

    #[test]
    fn returns_none_without_a_jpeg() {
        assert_eq!(largest_embedded_jpeg(&[1, 2, 3, 4, 5, 6]), None);
        assert_eq!(largest_embedded_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]), None); // truncated
        assert_eq!(largest_embedded_jpeg(&[]), None);
    }
}
