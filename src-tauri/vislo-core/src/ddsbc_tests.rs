//! BC4-BC7 and DX10-header DDS checks. The fixtures are 10x6 (the right and bottom blocks are cropped) with random but
//! valid blocks written by a script; the `.raw` files are Pillow's decoding (bcdec) of the same bytes, which this
//! decoder reproduces exactly. BGRA8 has no Pillow reader, so its `.raw` is the channel swap made by the script.

use crate::ddsbc::{decode_bytes, wants};

const BIG: u64 = 1 << 20;

macro_rules! fixture {
    ($name:ident, $file:literal, $why:literal) => {
        #[test]
        fn $name() {
            let img = decode_bytes(include_bytes!(concat!("test_data/", $file, ".dds")), BIG).unwrap();
            assert_eq!((img.width(), img.height()), (10, 6));
            assert_eq!(img.to_rgba8().into_raw(), include_bytes!(concat!("test_data/", $file, ".raw")), $why);
        }
    };
}

fixture!(bc1_with_one_bit_alpha, "dds-dx10-bc1", "c0 <= c1 makes index 3 transparent");
fixture!(bc2_explicit_alpha_always_four_colours, "dds-dx10-bc2", "endpoint order must not switch BC2 to three colours");
fixture!(bc3_interpolated_alpha_always_four_colours, "dds-dx10-bc3", "both alpha ramps and the four-colour rule");
fixture!(bc4_through_the_ati1_four_cc, "dds-bc4-ati1", "one channel shown as grey");
fixture!(bc4_in_a_dx10_header, "dds-dx10-bc4", "same data as the DXGI format 80");
fixture!(bc5_through_the_ati2_four_cc, "dds-bc5-ati2", "red and green");
fixture!(bc5_in_a_dx10_header, "dds-dx10-bc5", "red and green");
fixture!(bc7_all_modes_in_the_sample, "dds-dx10-bc7", "random valid blocks");
fixture!(uncompressed_rgba8, "dds-dx10-rgba8", "four bytes per pixel as stored");
fixture!(uncompressed_bgra8, "dds-dx10-bgra8", "blue and red swapped");

#[test]
fn only_the_formats_image_cannot_read_are_claimed() {
    let dxt1 = [b"DDS ".as_slice(), &[0; 80], b"DXT1"].concat();
    assert!(!wants(&dxt1), "DXT1-5 stay with the image crate");
    for cc in [b"DX10", b"ATI1", b"ATI2", b"BC4U", b"BC5U"] {
        assert!(wants(&[b"DDS ".as_slice(), &[0; 80], cc].concat()));
    }
    assert!(!wants(b"DDS "), "a short header is not claimed");
}

#[test]
fn the_dispatch_opens_a_dx10_file_by_path() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/test_data/dds-dx10-bc7.dds");
    let img = crate::decode::load(&path).unwrap();
    assert_eq!((img.width(), img.height()), (10, 6));
}

#[test]
fn unsupported_formats_say_which() {
    let mut f = include_bytes!("test_data/dds-dx10-bc7.dds").to_vec();
    f[128..132].copy_from_slice(&2u32.to_le_bytes()); // DXGI_FORMAT_R32G32B32A32_FLOAT
    assert!(decode_bytes(&f, BIG).unwrap_err().contains("DXGI 2"));
    let mut g = include_bytes!("test_data/dds-bc4-ati1.dds").to_vec();
    g[84..88].copy_from_slice(b"BC4S");
    assert!(decode_bytes(&g, BIG).unwrap_err().contains("signed"));
    for dxgi in [94u32, 95, 96] {
        f[128..132].copy_from_slice(&dxgi.to_le_bytes());
        assert!(decode_bytes(&f, BIG).unwrap_err().contains("BC6H"), "DXGI {dxgi}: the crate's BC6H decoder is wrong");
    }
}

fn all() -> [&'static [u8]; 4] {
    [
        include_bytes!("test_data/dds-dx10-bc7.dds"),
        include_bytes!("test_data/dds-dx10-bc3.dds"),
        include_bytes!("test_data/dds-bc5-ati2.dds"),
        include_bytes!("test_data/dds-dx10-bgra8.dds"),
    ]
}

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    for f in all() {
        for n in 0..f.len() {
            assert!(decode_bytes(&f[..n], BIG).is_err(), "{n} of {} bytes decoded", f.len());
        }
    }
}

#[test]
fn flipped_bytes_never_panic() {
    for f in all() {
        for i in 0..f.len() {
            for x in [0xFFu8, 0x80, 0x01] {
                let mut g = f.to_vec();
                g[i] ^= x;
                let _ = decode_bytes(&g, BIG);
            }
        }
    }
}

#[test]
fn a_tiny_file_cannot_claim_a_huge_picture() {
    let mut f = include_bytes!("test_data/dds-dx10-bc7.dds").to_vec();
    f[12..20].copy_from_slice(&[0xFF; 8]);
    assert_eq!(decode_bytes(&f, 150_000_000).unwrap_err(), crate::decode::TOO_LARGE, "4G x 4G");
    f[12..20].copy_from_slice(&[0x40, 0x1F, 0, 0, 0x40, 0x1F, 0, 0]); // 8000 x 8000 under every cap
    assert!(decode_bytes(&f, 150_000_000).unwrap_err().contains("truncated"), "checked before allocating");
    assert_eq!(decode_bytes(include_bytes!("test_data/dds-dx10-bc7.dds"), 4).unwrap_err(), crate::decode::TOO_LARGE);
}
