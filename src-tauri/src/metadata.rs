//! The EXIF rows of the info panel. Each row carries a stable key (the UI translates the label), the
//! group it belongs to and, for enumerations, the numeric code (the UI translates the value too).
//! EXIF text is untrusted: it is stripped of control characters and capped before it leaves Rust.

use exif::{Exif, In, Tag, Value};
use serde::Serialize;

const MAX_TEXT: usize = 200;

#[derive(Debug, Serialize, PartialEq)]
pub struct Row {
    pub group: &'static str,
    pub key: &'static str,
    pub value: String,
    /// The enumeration value behind `value`, when the field is one (program, metering, orientation ...).
    pub code: Option<u32>,
}

/// Control characters dropped, whitespace collapsed, length capped; `None` when nothing is left.
fn clean(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(|c| !c.is_control() || c.is_whitespace()).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    Some(if s.chars().count() > MAX_TEXT { s.chars().take(MAX_TEXT).collect::<String>() + "…" } else { s.to_string() })
}

/// ASCII fields are read as bytes (the library's display form escapes control characters as text such as
/// ``, which cleaning cannot remove); multi-part values (copyright: photographer, editor) are joined.
fn text(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    if let Value::Ascii(parts) = &field.value {
        let joined: Vec<String> = parts.iter().filter_map(|b| clean(&String::from_utf8_lossy(b))).collect();
        return (!joined.is_empty()).then(|| joined.join(" · "));
    }
    clean(&field.display_value().with_unit(exif).to_string())
}

fn uint(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0)
}

fn number(exif: &Exif, tag: Tag) -> Option<f64> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        Value::SRational(v) => v.first().filter(|r| r.denom != 0).map(|r| r.to_f64()),
        _ => None,
    }
}

fn dpi(exif: &Exif) -> Option<String> {
    let x = number(exif, Tag::XResolution)?;
    let y = number(exif, Tag::YResolution).unwrap_or(x);
    let per_cm = uint(exif, Tag::ResolutionUnit) == Some(3); // 2 (inch) is the EXIF default
    let (x, y) = if per_cm { (x * 2.54, y * 2.54) } else { (x, y) };
    if !(x >= 1.0 && y >= 1.0) {
        return None;
    }
    let (x, y) = (x.round(), y.round());
    Some(if x == y { format!("{x} dpi") } else { format!("{x} × {y} dpi") })
}

fn altitude(exif: &Exif) -> Option<String> {
    let metres = number(exif, Tag::GPSAltitude)?;
    let below = uint(exif, Tag::GPSAltitudeRef) == Some(1);
    Some(format!("{:.0} m", if below { -metres } else { metres }))
}

/// Make and model, without repeating the make when the model already starts with it.
fn joined(exif: &Exif, make: Tag, model: Tag) -> Option<String> {
    match (text(exif, make), text(exif, model)) {
        (Some(a), Some(b)) if b.to_lowercase().starts_with(&a.to_lowercase()) => Some(b),
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (a, b) => a.or(b),
    }
}

pub fn rows(exif: &Exif) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut plain = |group, key, value: Option<String>| {
        if let Some(value) = value {
            out.push(Row { group, key, value, code: None });
        }
    };

    let taken = text(exif, Tag::DateTimeOriginal).map(|d| match text(exif, Tag::OffsetTimeOriginal) {
        Some(offset) => format!("{d} {offset}"),
        None => d,
    });
    plain("file", "taken", taken);
    plain("image", "resolution", dpi(exif));
    plain("image", "software", text(exif, Tag::Software));
    plain("camera", "camera", joined(exif, Tag::Make, Tag::Model));
    plain("camera", "lens", joined(exif, Tag::LensMake, Tag::LensModel));
    plain("exposure", "exposure", text(exif, Tag::ExposureTime));
    plain("exposure", "aperture", text(exif, Tag::FNumber));
    plain("exposure", "iso", text(exif, Tag::PhotographicSensitivity));
    plain("exposure", "bias", number(exif, Tag::ExposureBiasValue).map(|ev| if ev == 0.0 { "0 EV".to_string() } else { format!("{ev:+.1} EV") }));
    plain("exposure", "focal", text(exif, Tag::FocalLength));
    plain("exposure", "focal35", uint(exif, Tag::FocalLengthIn35mmFilm).filter(|n| *n > 0).map(|n| format!("{n} mm")));
    plain("authorship", "artist", text(exif, Tag::Artist));
    plain("authorship", "copyright", text(exif, Tag::Copyright));
    plain("authorship", "description", text(exif, Tag::ImageDescription));
    plain("location", "gps", crate::exif_info::gps(exif).map(|(lat, lon)| format!("{lat:.5}, {lon:.5}")));
    plain("location", "altitude", altitude(exif));
    plain("location", "direction", number(exif, Tag::GPSImgDirection).map(|d| format!("{d:.0}°")));

    // Enumerations: `value` is the library's English wording, `code` lets the UI translate it.
    let mut coded = |group, key, tag: Tag, code: Option<u32>| {
        if let Some(code) = code {
            out.push(Row { group, key, value: text(exif, tag).unwrap_or_else(|| code.to_string()), code: Some(code) });
        }
    };
    coded("image", "orientation", Tag::Orientation, uint(exif, Tag::Orientation).filter(|o| (2..=8).contains(o))); // upright is not worth a row
    coded("image", "colorSpace", Tag::ColorSpace, uint(exif, Tag::ColorSpace));
    coded("exposure", "program", Tag::ExposureProgram, uint(exif, Tag::ExposureProgram));
    coded("exposure", "metering", Tag::MeteringMode, uint(exif, Tag::MeteringMode));
    coded("exposure", "whiteBalance", Tag::WhiteBalance, uint(exif, Tag::WhiteBalance));
    coded("exposure", "flash", Tag::Flash, uint(exif, Tag::Flash).map(|f| f & 1)); // bit 0: fired
    out.sort_by_key(|r| GROUPS.iter().position(|g| *g == r.group)); // stable: emission order is kept inside a group
    out
}

const GROUPS: [&str; 6] = ["file", "image", "camera", "exposure", "authorship", "location"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif_info::tests::{field, tiff_block};
    use exif::{Reader, SRational};

    fn ascii(s: &str) -> Value {
        Value::Ascii(vec![s.as_bytes().to_vec()])
    }
    fn exif(fields: &[exif::Field]) -> Exif {
        Reader::new().read_raw(tiff_block(fields, None)).unwrap()
    }
    fn find<'a>(rows: &'a [Row], key: &str) -> &'a Row {
        rows.iter().find(|r| r.key == key).unwrap_or_else(|| panic!("no row {key} in {rows:?}"))
    }
    fn p(tag: Tag, value: Value) -> exif::Field {
        field(tag, In::PRIMARY, value)
    }

    #[test]
    fn exposure_rows_keep_their_group_and_enum_codes() {
        let e = exif(&[
            p(Tag::ExposureProgram, Value::Short(vec![3])),
            p(Tag::MeteringMode, Value::Short(vec![5])),
            p(Tag::WhiteBalance, Value::Short(vec![1])),
            p(Tag::ExposureBiasValue, Value::SRational(vec![SRational { num: -2, denom: 3 }])),
            p(Tag::FocalLengthIn35mmFilm, Value::Short(vec![52])),
            p(Tag::Flash, Value::Short(vec![25])), // fired, auto mode
        ]);
        let r = rows(&e);
        assert_eq!((find(&r, "program").group, find(&r, "program").code), ("exposure", Some(3)));
        assert_eq!(find(&r, "metering").code, Some(5));
        assert_eq!(find(&r, "whiteBalance").code, Some(1));
        assert_eq!(find(&r, "bias").value, "-0.7 EV");
        assert_eq!(find(&r, "focal35").value, "52 mm");
        assert_eq!(find(&r, "flash").code, Some(1));
    }

    #[test]
    fn resolution_orientation_and_colour_space() {
        let rat = |n| Value::Rational(vec![(n, 1).into()]);
        let e = exif(&[
            p(Tag::XResolution, rat(300)),
            p(Tag::YResolution, rat(300)),
            p(Tag::ResolutionUnit, Value::Short(vec![2])),
            p(Tag::Orientation, Value::Short(vec![6])),
            p(Tag::ColorSpace, Value::Short(vec![1])),
        ]);
        let r = rows(&e);
        assert_eq!(find(&r, "resolution").value, "300 dpi");
        assert_eq!(find(&r, "orientation").code, Some(6));
        assert_eq!(find(&r, "colorSpace").code, Some(1));
        let cm = exif(&[p(Tag::XResolution, rat(118)), p(Tag::YResolution, rat(59)), p(Tag::ResolutionUnit, Value::Short(vec![3]))]);
        assert_eq!(find(&rows(&cm), "resolution").value, "300 × 150 dpi");
    }

    #[test]
    fn upright_images_get_no_orientation_row() {
        let e = exif(&[p(Tag::Orientation, Value::Short(vec![1]))]);
        assert!(rows(&e).is_empty());
    }

    #[test]
    fn authorship_and_camera_text_is_cleaned_and_deduplicated() {
        let e = exif(&[
            p(Tag::Artist, ascii("Ana \u{7}Souza\n\tPhoto")),
            p(Tag::Copyright, ascii(&"c".repeat(500))),
            p(Tag::Make, ascii("FUJIFILM")),
            p(Tag::Model, ascii("Fujifilm X-T5")),
            p(Tag::LensMake, ascii("Sigma")),
            p(Tag::LensModel, ascii("18-35mm F1.8")),
        ]);
        let r = rows(&e);
        assert_eq!(find(&r, "artist").value, "Ana Souza Photo");
        let c = &find(&r, "copyright").value;
        assert_eq!((c.chars().count(), c.ends_with('…')), (MAX_TEXT + 1, true));
        assert_eq!(find(&r, "camera").value, "Fujifilm X-T5", "the make is not repeated");
        assert_eq!(find(&r, "lens").value, "Sigma 18-35mm F1.8");
    }

    #[test]
    fn location_rows_cover_altitude_below_sea_level_and_direction() {
        let rat = |n, d| Value::Rational(vec![(n, d).into()]);
        let e = exif(&[
            p(Tag::GPSAltitude, rat(2100, 10)),
            p(Tag::GPSAltitudeRef, Value::Byte(vec![1])),
            p(Tag::GPSImgDirection, rat(2706, 10)),
        ]);
        let r = rows(&e);
        assert_eq!(find(&r, "altitude").value, "-210 m");
        assert_eq!(find(&r, "direction").value, "271°");
        assert!(r.iter().all(|x| x.key != "gps"), "coordinates need both latitude and longitude");
    }

    #[test]
    fn a_zero_denominator_is_not_a_number() {
        let e = exif(&[p(Tag::GPSAltitude, Value::Rational(vec![(5, 0).into()]))]);
        assert!(rows(&e).is_empty());
    }

    #[test]
    fn rows_come_out_grouped_in_display_order() {
        let e = exif(&[
            p(Tag::Artist, ascii("A")),
            p(Tag::Orientation, Value::Short(vec![3])),
            p(Tag::Make, ascii("Canon")),
            p(Tag::DateTimeOriginal, ascii("2026:09:14 17:42:00")),
        ]);
        let groups: Vec<_> = rows(&e).iter().map(|r| r.group).collect();
        assert_eq!(groups, ["file", "image", "camera", "authorship"]);
    }

    #[test]
    fn clean_drops_control_characters_and_empty_strings() {
        assert_eq!(clean("  \"x\"  "), Some("x".into()));
        assert_eq!(clean("\u{1}\u{2}"), None);
        assert_eq!(clean("   "), None);
    }
}
