//! ANI checks: a two-frame file whose first frame was made and decoded by Pillow, plus hostile-input guards.

use crate::ani::decode_bytes;

const TWO: &[u8] = include_bytes!("test_data/ani-two-frames.ani");

#[test]
fn the_first_frame_is_shown_with_its_transparency() {
    let img = decode_bytes(TWO, 1 << 20).unwrap();
    assert_eq!((img.width(), img.height()), (16, 16));
    assert_eq!(img.to_rgba8().into_raw(), include_bytes!("test_data/ani-two-frames.raw"), "pixels as Pillow decodes the same frame");
}

#[test]
fn files_without_frames_or_with_bad_headers_are_errors() {
    assert!(decode_bytes(b"", 100).is_err());
    assert!(decode_bytes(b"RIFF\0\0\0\0WAVE", 100).is_err(), "a RIFF file that is not a cursor");
    assert!(decode_bytes(b"RIFF\x04\0\0\0ACON", 100).unwrap_err().contains("no frames"));
}

#[test]
fn truncated_and_corrupted_files_are_errors_not_panics() {
    // Cuts inside the first frame (it starts at byte 76). A cut that only clips a later frame still shows frame one.
    for cut in [11, 20, 60, 80, 120] {
        assert!(decode_bytes(&TWO[..cut], 1 << 20).is_err(), "cut at {cut}");
    }
    assert!(decode_bytes(&TWO[..TWO.len() - 5], 1 << 20).is_ok());
    for at in 0..TWO.len() {
        for flip in [0x01u8, 0x80, 0xFF] {
            let mut b = TWO.to_vec();
            b[at] ^= flip;
            let _ = decode_bytes(&b, 1 << 20);
        }
    }
}

#[test]
fn the_pixel_cap_applies() {
    assert_eq!(decode_bytes(TWO, 10).unwrap_err(), crate::decode::TOO_LARGE);
}
