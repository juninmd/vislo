//! BLP, DCX, GBR and FLI/FLC checks. Every fixture is a small synthetic picture written by an independent encoder
//! (`.raw` holds the pixels it was made from). Pillow decodes the palette BLP1 RGB, BLP2 palette, DXT, DCX and all FLI
//! fixtures to the same pixels; DXT rounding differs from Pillow's by a few levels, so those are compared with a bound.

use crate::{blp, dcx, fli, gbr};
use image::DynamicImage;

type Decode = fn(&[u8], u64) -> Result<DynamicImage, String>;

const BIG: u64 = 1 << 20;

macro_rules! fixture {
    ($name:ident, $decode:expr, $stem:literal, $ext:literal, $w:expr, $h:expr, $rgba:expr, $why:literal) => {
        #[test]
        fn $name() {
            let img = ($decode as Decode)(include_bytes!(concat!("test_data/", $stem, ".", $ext)), BIG).unwrap();
            assert_eq!((img.width(), img.height()), ($w, $h));
            let want = include_bytes!(concat!("test_data/", $stem, ".raw"));
            let got = if $rgba { img.to_rgba8().into_raw() } else { img.to_rgb8().into_raw() };
            assert_eq!(got, want, $why);
        }
    };
}

fixture!(blp1_palette_with_an_alpha_plane, blp::decode_bytes, "blp1-palette-a8", "blp", 6, 3, true, "8-bit alpha follows the indices");
fixture!(blp1_palette_with_nibble_alpha, blp::decode_bytes, "blp1-palette-a4", "blp", 6, 3, true, "low nibble first, scaled by 17");
fixture!(blp1_palette_with_bit_alpha, blp::decode_bytes, "blp1-palette-a1", "blp", 6, 3, true, "one bit per pixel, least significant first");
fixture!(blp2_palette, blp::decode_bytes, "blp2-palette", "blp", 6, 3, true, "BGRA palette entries");
fixture!(blp2_uncompressed_bgra, blp::decode_bytes, "blp2-raw", "blp", 5, 3, true, "channels are stored blue first");
fixture!(dcx_shows_the_first_page, dcx::decode_bytes, "dcx-two-pages", "dcx", 13, 7, false, "the second page is 9x5 and must not be used");
fixture!(dcx_page_one_keeps_its_own_palette, dcx::decode_bytes, "dcx-palette-pages", "dcx", 7, 5, false, "the PCX reader takes the palette from the end of its input, which is the last page unless the slice stops at the page");
fixture!(fli_byte_run_frame, fli::decode_bytes, "fli-brun", "fli", 9, 4, false, "COLOR_64 palette is scaled by 4; runs and literals");
fixture!(flc_copy_frame_with_a_256_palette, fli::decode_bytes, "fli-copy", "flc", 9, 4, false, "COLOR_256 values are used as they are");
fixture!(fli_line_compressed_delta_over_black, fli::decode_bytes, "fli-lc", "fli", 9, 4, false, "LC rewrites only the lines it names");
fixture!(flc_word_delta_with_skipped_lines, fli::decode_bytes, "fli-delta", "flc", 9, 4, false, "DELTA_FLC skips lines and writes word pairs");

#[test]
fn gbr_greyscale_and_colour_brushes() {
    let grey = gbr::decode_bytes(include_bytes!("test_data/gbr-v2-grey.gbr"), BIG).unwrap();
    assert_eq!(grey.to_luma8().into_raw(), include_bytes!("test_data/gbr-v2-grey.raw"), "version 2, one byte per pixel");
    let rgba = gbr::decode_bytes(include_bytes!("test_data/gbr-v1-rgba.gbr"), BIG).unwrap();
    assert_eq!(rgba.to_rgba8().into_raw(), include_bytes!("test_data/gbr-v1-rgba.raw"), "version 1, four bytes per pixel");
}

#[test]
fn dxt_blocks_match_a_reference_within_rounding() {
    let files: [(&[u8], &[u8]); 5] = [
        (include_bytes!("test_data/blp2-dxt1.blp"), include_bytes!("test_data/blp2-dxt1.raw")),
        (include_bytes!("test_data/blp2-dxt1a.blp"), include_bytes!("test_data/blp2-dxt1a.raw")),
        (include_bytes!("test_data/blp2-dxt3.blp"), include_bytes!("test_data/blp2-dxt3.raw")),
        (include_bytes!("test_data/blp2-dxt5.blp"), include_bytes!("test_data/blp2-dxt5.raw")),
        (include_bytes!("test_data/blp2-dxt5-ramp6.blp"), include_bytes!("test_data/blp2-dxt5-ramp6.raw")),
    ];
    for (i, (blp, want)) in files.iter().enumerate() {
        let img = blp::decode_bytes(blp, BIG).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (8, 5), "the bottom block row is cut: file {i}");
        let worst = img.as_raw().iter().zip(*want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 8, "file {i} differs by {worst} levels");
        // Alpha has no rounding freedom in DXT1/3: it must be exact there.
        if i <= 2 {
            assert!(img.as_raw().chunks(4).zip(want.chunks(4)).all(|(a, b)| a[3] == b[3]), "alpha of file {i}");
        }
    }
}

#[test]
fn a_width_that_cuts_a_dxt_block_is_the_wider_picture_cropped() {
    let full = include_bytes!("test_data/blp2-dxt5.blp");
    let mut narrow = full.to_vec();
    narrow[12..16].copy_from_slice(&5u32.to_le_bytes());
    let (wide, cut) = (blp::decode_bytes(full, BIG).unwrap().to_rgba8(), blp::decode_bytes(&narrow, BIG).unwrap().to_rgba8());
    assert_eq!(cut.dimensions(), (5, 5));
    assert!((0..5).all(|y| (0..5).all(|x| cut.get_pixel(x, y) == wide.get_pixel(x, y))));
}

#[test]
fn blp1_with_jpeg_data_says_it_is_not_supported() {
    let mut f = include_bytes!("test_data/blp1-palette-a8.blp").to_vec();
    f[4] = 0; // content type 0 is JPEG
    assert!(blp::decode_bytes(&f, BIG).unwrap_err().contains("JPEG"));
}

fn all() -> Vec<(&'static str, Decode, &'static [u8])> {
    vec![
        ("blp1", blp::decode_bytes, include_bytes!("test_data/blp1-palette-a4.blp")),
        ("blp2-dxt5", blp::decode_bytes, include_bytes!("test_data/blp2-dxt5.blp")),
        ("blp2-raw", blp::decode_bytes, include_bytes!("test_data/blp2-raw.blp")),
        ("dcx", dcx::decode_bytes, include_bytes!("test_data/dcx-two-pages.dcx")),
        ("gbr", gbr::decode_bytes, include_bytes!("test_data/gbr-v1-rgba.gbr")),
        ("fli-brun", fli::decode_bytes, include_bytes!("test_data/fli-brun.fli")),
        ("flc-delta", fli::decode_bytes, include_bytes!("test_data/fli-delta.flc")),
        ("fli-lc", fli::decode_bytes, include_bytes!("test_data/fli-lc.fli")),
    ]
}

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    for (name, decode, bytes) in all() {
        for n in 0..bytes.len() {
            // A cut inside trailing padding may still decode (BLP palette bytes, a page's tail); it must never panic.
            let _ = decode(&bytes[..n], BIG);
        }
        assert!(decode(&bytes[..bytes.len().min(3)], BIG).is_err(), "{name}");
    }
}

#[test]
fn flipped_bytes_never_panic() {
    for (_, decode, bytes) in all() {
        for i in 0..bytes.len() {
            for x in [0xFFu8, 0x80, 0x01] {
                let mut f = bytes.to_vec();
                f[i] ^= x;
                let _ = decode(&f, BIG);
            }
        }
    }
}

#[test]
fn a_small_pixel_cap_refuses_before_decoding() {
    for (name, decode, bytes) in all() {
        let e = decode(bytes, 4).unwrap_err();
        assert_eq!(e, crate::decode::TOO_LARGE, "{name}");
    }
}

#[test]
fn a_tiny_file_cannot_claim_a_huge_picture() {
    let mut blp = include_bytes!("test_data/blp2-raw.blp").to_vec();
    blp[12..20].copy_from_slice(&[0xFF; 8]);
    assert!(blp::decode_bytes(&blp, 150_000_000).is_err(), "BLP width/height of 4G x 4G");
    blp[12..20].copy_from_slice(&[0x40, 0x1F, 0, 0, 0x40, 0x1F, 0, 0]); // 8000 x 8000: under every cap, far more than the bytes held
    assert!(blp::decode_bytes(&blp, 150_000_000).unwrap_err().contains("truncated"), "BLP claims 64 MP over 1.2 KB");
    let mut gbr = include_bytes!("test_data/gbr-v1-rgba.gbr").to_vec();
    gbr[8..16].copy_from_slice(&[0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF]);
    assert!(gbr::decode_bytes(&gbr, 150_000_000).is_err(), "GBR 65535 x 65535");
    let mut fli = include_bytes!("test_data/fli-copy.flc").to_vec();
    fli[8..12].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(fli::decode_bytes(&fli, 150_000_000).is_err(), "FLI 65535 x 65535 is over the pixel cap");
}

#[test]
fn headers_that_do_not_belong_are_rejected_by_name() {
    assert!(blp::decode_bytes(b"BLP3", BIG).unwrap_err().contains("BLP"));
    assert!(dcx::decode_bytes(b"\xB1\x68\xDE\x3A\0\0\0\0", BIG).unwrap_err().contains("no pages"));
    let mut g = include_bytes!("test_data/gbr-v1-rgba.gbr").to_vec();
    g[19] = 2; // two bytes per pixel is not a brush
    assert!(gbr::decode_bytes(&g, BIG).unwrap_err().contains("GIMP brush"));
    let mut f = include_bytes!("test_data/fli-brun.fli").to_vec();
    f[144..148].copy_from_slice(&[2, 0, 0, 0]); // first chunk size below its own header
    assert!(fli::decode_bytes(&f, BIG).unwrap_err().contains("corrupt"));
}

fn with_chunk_at_128(base: &[u8], chunk: &[u8]) -> Vec<u8> {
    let mut f = base.to_vec();
    f.splice(128..128, chunk.iter().copied());
    let total = (f.len() as u32).to_le_bytes();
    f[..4].copy_from_slice(&total);
    f
}

#[test]
fn an_flc_prefix_chunk_before_the_first_frame_is_skipped() {
    let base = include_bytes!("test_data/fli-copy.flc");
    let want = fli::decode_bytes(base, BIG).unwrap();
    let prefix = [&16u32.to_le_bytes()[..], &[0x00, 0xF1], &[0; 10]].concat();
    let mut f = with_chunk_at_128(base, &prefix);
    assert_eq!(fli::decode_bytes(&f, BIG).unwrap(), want, "found by its 0xF100 type");
    f[80..84].copy_from_slice(&144u32.to_le_bytes());
    assert_eq!(fli::decode_bytes(&f, BIG).unwrap(), want, "found by the oframe1 field");
}

#[test]
fn high_colour_flc_files_are_refused_not_misread() {
    let mut f = include_bytes!("test_data/fli-copy.flc").to_vec();
    f[12] = 16;
    assert!(fli::decode_bytes(&f, BIG).unwrap_err().contains("colour depth"));
}

#[test]
fn a_flood_of_black_chunks_costs_time_in_proportion_to_the_file() {
    // 4000 x 4000 pixels and 20000 six-byte BLACK chunks: clearing the picture for each would take minutes.
    let black = [6u8, 0, 0, 0, 13, 0];
    let chunks = 20_000u16;
    let frame = [&(16 + 6 * u32::from(chunks)).to_le_bytes()[..], &[0xFA, 0xF1], &chunks.to_le_bytes(), &[0; 8], &black.repeat(usize::from(chunks))].concat();
    let mut f = include_bytes!("test_data/fli-copy.flc")[..128].to_vec();
    f[8..12].copy_from_slice(&[0xA0, 0x0F, 0xA0, 0x0F]);
    f.extend(frame);
    let started = std::time::Instant::now();
    let img = fli::decode_bytes(&f, BIG * 64).unwrap();
    assert_eq!((img.width(), img.height()), (4000, 4000));
    assert!(started.elapsed().as_secs() < 5, "took {:?}", started.elapsed());
}

#[test]
fn drawing_before_a_black_chunk_is_wiped_by_it() {
    let mut f = include_bytes!("test_data/fli-copy.flc").to_vec();
    f.extend_from_slice(&[6, 0, 0, 0, 13, 0]); // BLACK after the COPY chunk
    f[134..136].copy_from_slice(&3u16.to_le_bytes()); // chunks in the frame: palette, copy, black
    let img = fli::decode_bytes(&f, BIG).unwrap().to_rgb8();
    let first = &img.as_raw()[..3];
    assert!(img.as_raw().chunks(3).all(|p| p == first), "every pixel is palette entry 0");
}
