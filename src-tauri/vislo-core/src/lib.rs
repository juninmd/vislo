//! Image decoding for Vislo, without any UI: format detection, decoders for every supported format, EXIF,
//! thumbnails and resizing. The Vislo app uses it, and other programs can depend on it to show the same images.

mod ani;
#[cfg(test)]
mod ani_tests;
mod bilevel;
mod blp;
pub mod codecs;
mod dcx;
mod ddsbc;
#[cfg(test)]
mod ddsbc_tests;
pub mod decode;
mod dicom;
mod djvu;
#[cfg(test)]
mod djvu_tests;
mod dpx;
pub mod exif_info;
#[cfg(test)]
mod extra_formats;
mod fits;
#[cfg(test)]
mod fixtures;
mod fli;
pub mod formats;
mod gbr;
mod gdip;
mod iff;
#[cfg(test)]
mod iff_tests;
mod jng;
#[cfg(test)]
mod jng_tests;
mod jpeg_scaled;
mod jpegls;
mod jpegls_codec;
#[cfg(test)]
mod jpegls_tests;
mod ktx;
#[cfg(test)]
mod ktx_tests;
mod layered;
pub mod metadata;
mod mng;
mod pict;
#[cfg(test)]
mod pict_tests;
mod psb;
mod raw;
pub mod resize;
#[cfg(test)]
mod retro_tests;
mod sunras;
pub mod svgz;
pub mod thumbs;
mod wic;
mod xcf;
#[cfg(test)]
mod xcf_tests;
mod xwd;
