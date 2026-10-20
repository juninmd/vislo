//! EXIF access: orientation, the embedded preview thumbnail, and the rows of the info panel.

use exif::{Exif, In, Reader, Tag, Value};
use serde::Serialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub fn read(path: &Path) -> Option<Exif> {
    let file = File::open(path).ok()?;
    Reader::new().read_from_container(&mut BufReader::new(file)).ok()
}

/// EXIF orientation 1..=8; anything missing or out of range counts as "upright".
pub fn orientation(exif: &Exif) -> u8 {
    exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|v| (1..=8).contains(v))
        .map_or(1, |v| v as u8)
}

/// The JPEG thumbnail stored in the EXIF block, if it is fully inside the block.
pub fn thumbnail(exif: &Exif) -> Option<Vec<u8>> {
    let uint = |tag| exif.get_field(tag, In::THUMBNAIL)?.value.get_uint(0).map(|v| v as usize);
    let (off, len) = (uint(Tag::JPEGInterchangeFormat)?, uint(Tag::JPEGInterchangeFormatLength)?);
    exif.buf().get(off..off.checked_add(len)?).map(<[u8]>::to_vec)
}

fn coordinate(exif: &Exif, value: Tag, reference: Tag) -> Option<f64> {
    let Value::Rational(parts) = &exif.get_field(value, In::PRIMARY)?.value else { return None };
    let [d, m, s] = parts.as_slice() else { return None };
    let deg = d.to_f64() + m.to_f64() / 60.0 + s.to_f64() / 3600.0;
    let Value::Ascii(r) = &exif.get_field(reference, In::PRIMARY)?.value else { return None };
    let negative = matches!(r.first().and_then(|b| b.first()), Some(b'S' | b'W'));
    Some(if negative { -deg } else { deg })
}

pub fn gps(exif: &Exif) -> Option<(f64, f64)> {
    Some((
        coordinate(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef)?,
        coordinate(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef)?,
    ))
}

fn text(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let s = field.display_value().with_unit(exif).to_string();
    let s = s.trim().trim_matches('"').trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Field set mirrors Picasa's `properties.xml`: file facts first, then camera data.
#[derive(Debug, Serialize, Default)]
pub struct Info {
    pub name: String,
    pub folder: String,
    pub size: u64,
    pub modified_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub rows: Vec<(String, String)>,
}

pub fn info(path: &Path) -> Info {
    let meta = std::fs::metadata(path).ok();
    let mut info = Info {
        name: crate::album::display_name(path),
        folder: path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        size: meta.as_ref().map_or(0, |m| m.len()),
        modified_ms: meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64),
        ..Info::default()
    };
    if let Ok((w, h)) = image::image_dimensions(path) {
        (info.width, info.height) = (Some(w), Some(h));
    }
    let Some(exif) = read(path) else { return info };
    let camera = [Tag::Make, Tag::Model].iter().filter_map(|t| text(&exif, *t)).collect::<Vec<_>>().join(" ");
    let mut push = |label: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            info.rows.push((label.into(), v));
        }
    };
    push("Camera", Some(camera));
    push("Lens", text(&exif, Tag::LensModel));
    push("Focal length", text(&exif, Tag::FocalLength));
    push("Exposure", text(&exif, Tag::ExposureTime));
    push("Aperture", text(&exif, Tag::FNumber));
    push("ISO", text(&exif, Tag::PhotographicSensitivity));
    push("Taken", text(&exif, Tag::DateTimeOriginal));
    push("Flash", text(&exif, Tag::Flash));
    push("Software", text(&exif, Tag::Software));
    push("GPS", gps(&exif).map(|(lat, lon)| format!("{lat:.5}, {lon:.5}")));
    info
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use exif::experimental::Writer;
    use exif::Field;
    use std::io::Cursor;

    pub fn field(tag: Tag, ifd: In, value: Value) -> Field {
        Field { tag, ifd_num: ifd, value }
    }

    /// Serialises fields (and an optional thumbnail) as a TIFF-structured Exif block.
    pub fn tiff_block(fields: &[Field], thumb: Option<&[u8]>) -> Vec<u8> {
        let mut w = Writer::new();
        for f in fields {
            w.push_field(f);
        }
        if let Some(t) = thumb {
            w.set_jpeg(t, In::THUMBNAIL);
        }
        let mut out = Cursor::new(Vec::new());
        w.write(&mut out, false).unwrap();
        out.into_inner()
    }

    /// Splices an Exif APP1 segment into a JPEG right after SOI.
    pub fn jpeg_with_exif(jpeg: &[u8], tiff: &[u8]) -> Vec<u8> {
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(tiff);
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend(((payload.len() + 2) as u16).to_be_bytes());
        out.extend(payload);
        out.extend(&jpeg[2..]);
        out
    }

    fn parse(tiff: Vec<u8>) -> Exif {
        Reader::new().read_raw(tiff).unwrap()
    }

    #[test]
    fn orientation_defaults_to_upright_and_rejects_garbage() {
        let rot = parse(tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], None));
        assert_eq!(orientation(&rot), 6);
        let bad = parse(tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![99]))], None));
        assert_eq!(orientation(&bad), 1);
        let none = parse(tiff_block(&[field(Tag::Make, In::PRIMARY, Value::Ascii(vec![b"X".to_vec()]))], None));
        assert_eq!(orientation(&none), 1);
    }

    #[test]
    fn thumbnail_round_trips_through_the_exif_block() {
        let thumb = [0xFF, 0xD8, 0xFF, 0xD9, 1, 2, 3];
        let exif = parse(tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![1]))], Some(&thumb)));
        assert_eq!(thumbnail(&exif).as_deref(), Some(&thumb[..]));
    }

    #[test]
    fn gps_south_and_west_are_negative() {
        let dms = |d| Value::Rational(vec![(d, 1).into(), (30, 1).into(), (0, 1).into()]);
        let exif = parse(tiff_block(
            &[
                field(Tag::GPSLatitudeRef, In::PRIMARY, Value::Ascii(vec![b"S".to_vec()])),
                field(Tag::GPSLatitude, In::PRIMARY, dms(23)),
                field(Tag::GPSLongitudeRef, In::PRIMARY, Value::Ascii(vec![b"W".to_vec()])),
                field(Tag::GPSLongitude, In::PRIMARY, dms(46)),
            ],
            None,
        ));
        let (lat, lon) = gps(&exif).unwrap();
        assert!((lat + 23.5).abs() < 1e-9 && (lon + 46.5).abs() < 1e-9, "{lat} {lon}");
    }
}
