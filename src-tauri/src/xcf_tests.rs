//! XCF decoding checks. Files are built here with an assembler for the format; one real GIMP file (1x1 violet,
//! version 0, from the xcf-rs test samples, Unlicense) pins the layout against what GIMP actually writes.

use crate::xcf::decode;
use image::{DynamicImage, GenericImageView};

struct Layer {
    w: usize,
    h: usize,
    kind: u32, // 0 RGB, 1 RGBA, 2 grey, 3 grey+alpha, 4 indexed
    off: (i32, i32),
    opacity: u32,
    visible: bool,
    group: bool,
    px: Vec<u8>,
    mask: Option<Vec<u8>>,
    mask_size: Option<(usize, usize)>,
    apply_mask: bool,
    skip_tiles: bool,
    path: Option<Vec<u32>>,
    hier_size: Option<(usize, usize)>,
}

fn layer(w: usize, h: usize, kind: u32, px: Vec<u8>) -> Layer {
    Layer { w, h, kind, off: (0, 0), opacity: 255, visible: true, group: false, px, mask: None, mask_size: None, apply_mask: true, skip_tiles: false, path: None, hier_size: None }
}

fn bpp(kind: u32) -> usize {
    [3, 4, 1, 2, 1][kind as usize]
}

struct Out {
    b: Vec<u8>,
    wide: bool,
}

impl Out {
    fn u32(&mut self, v: u32) {
        self.b.extend(v.to_be_bytes());
    }
    fn ptr(&mut self, v: usize) {
        if self.wide { self.b.extend((v as u64).to_be_bytes()) } else { self.u32(v as u32) }
    }
    fn patch(&mut self, at: usize, v: usize) {
        let bytes = if self.wide { (v as u64).to_be_bytes().to_vec() } else { (v as u32).to_be_bytes().to_vec() };
        self.b[at..at + bytes.len()].copy_from_slice(&bytes);
    }
    fn prop(&mut self, kind: u32, data: &[u8]) {
        self.u32(kind);
        self.u32(data.len() as u32);
        self.b.extend(data);
    }
    fn end(&mut self) {
        self.prop(0, &[]);
    }
    fn name(&mut self, n: &str) {
        self.u32(n.len() as u32 + 1);
        self.b.extend(n.as_bytes());
        self.b.push(0);
    }
}

fn rle(plane: &[u8]) -> Vec<u8> {
    if plane.len() > 1 && plane.iter().all(|b| *b == plane[0]) {
        return [&[127u8][..], &(plane.len() as u16).to_be_bytes(), &[plane[0]]].concat(); // one long run
    }
    if plane.len() > 150 {
        return [&[128u8][..], &(plane.len() as u16).to_be_bytes(), plane].concat(); // one long literal
    }
    plane.chunks(100).flat_map(|c| [&[(256 - c.len()) as u8][..], c].concat()).collect() // short literals
}

/// Writes the hierarchy of `px` (`w * h * bpp`, interleaved) and returns its offset.
fn hierarchy(o: &mut Out, w: usize, h: usize, bpp: usize, px: &[u8], compression: u8, skip_tiles: bool) -> usize {
    let at = o.b.len();
    for v in [w, h, bpp] {
        o.u32(v as u32);
    }
    let level_ptr = o.b.len();
    o.ptr(0);
    o.ptr(0); // list end for the levels
    let level = o.b.len();
    o.patch(level_ptr, level);
    o.u32(w as u32);
    o.u32(h as u32);
    let (across, down) = (w.div_ceil(64), h.div_ceil(64));
    let list = o.b.len();
    for _ in 0..across * down + 1 {
        o.ptr(0);
    }
    let step = if o.wide { 8 } else { 4 };
    for t in (0..across * down).filter(|_| !skip_tiles) {
        let (tx, ty) = ((t % across) * 64, (t / across) * 64);
        let (tw, th) = (64.min(w - tx), 64.min(h - ty));
        let tile: Vec<u8> = (0..th).flat_map(|y| px[((ty + y) * w + tx) * bpp..((ty + y) * w + tx + tw) * bpp].to_vec()).collect();
        o.patch(list + t * step, o.b.len());
        if compression == 0 {
            o.b.extend(&tile);
        } else if compression == 2 {
            use std::io::Write;
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(&tile).unwrap();
            o.b.extend(z.finish().unwrap());
        } else {
            for c in 0..bpp {
                let plane: Vec<u8> = tile.chunks(bpp).map(|p| p[c]).collect();
                o.b.extend(rle(&plane));
            }
        }
    }
    at
}

fn xcf(version: &str, base: u32, w: u32, h: u32, compression: u8, palette: &[[u8; 3]], layers: &[Layer]) -> Vec<u8> {
    let wide = version.starts_with("v0") && version[1..].parse::<u32>().unwrap() >= 11;
    let mut o = Out { b: format!("gimp xcf {version}\0").into_bytes(), wide };
    for v in [w, h, base] {
        o.u32(v);
    }
    if version != "file" && version[1..].parse::<u32>().unwrap() >= 4 {
        o.u32(150);
    }
    o.prop(17, &[compression]);
    if !palette.is_empty() {
        o.prop(1, &[(palette.len() as u32).to_be_bytes().to_vec(), palette.concat()].concat());
    }
    o.end();
    let list = o.b.len();
    for _ in 0..layers.len() + 2 {
        o.ptr(0);
    }
    let step = if wide { 8 } else { 4 };
    for (i, l) in layers.iter().enumerate() {
        o.patch(list + i * step, o.b.len());
        for v in [l.w as u32, l.h as u32, l.kind] {
            o.u32(v);
        }
        o.name("layer");
        o.prop(6, &l.opacity.to_be_bytes());
        o.prop(8, &u32::from(l.visible).to_be_bytes());
        o.prop(15, &[l.off.0.to_be_bytes(), l.off.1.to_be_bytes()].concat());
        if l.group {
            o.prop(29, &[]);
        }
        if !l.apply_mask {
            o.prop(11, &0u32.to_be_bytes());
        }
        if let Some(path) = &l.path {
            o.prop(30, &path.iter().flat_map(|n| n.to_be_bytes()).collect::<Vec<u8>>());
        }
        o.end();
        let ptrs = o.b.len();
        o.ptr(0);
        o.ptr(0);
        let (hw, hh) = l.hier_size.unwrap_or((l.w, l.h));
        let hier = hierarchy(&mut o, hw, hh, bpp(l.kind), &l.px, compression, l.skip_tiles);
        o.patch(ptrs, hier);
        if let Some(m) = &l.mask {
            let at = o.b.len();
            o.u32(l.mask_size.map_or(l.w, |m| m.0) as u32);
            o.u32(l.mask_size.map_or(l.h, |m| m.1) as u32);
            o.name("mask");
            o.end();
            let mp = o.b.len();
            o.ptr(0);
            let (mw, mh_) = l.mask_size.unwrap_or((l.w, l.h));
            let mh = hierarchy(&mut o, mw, mh_, 1, m, compression, false);
            o.patch(mp, mh);
            o.patch(ptrs + step, at);
        }
    }
    o.b
}

fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vislo-xcf-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.xcf"), bytes).unwrap();
    decode(&d.join("a.xcf"), max)
}

fn px(img: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
    img.to_rgba8().get_pixel(x, y).0
}

fn solid(w: usize, h: usize, kind: u32, c: &[u8]) -> Layer {
    layer(w, h, kind, c.iter().copied().cycle().take(w * h * c.len()).collect())
}

#[test]
fn a_real_gimp_file_decodes_to_its_violet_pixel() {
    let img = run(include_bytes!("test_data/xcf-1x1-violet-legacy.xcf"), 100).unwrap();
    assert_eq!((img.dimensions(), px(&img, 0, 0)), ((1, 1), [158, 36, 222, 255]));
}

#[test]
fn layers_are_composed_bottom_up_with_opacity_offset_and_clipping_in_every_pointer_width_and_compression() {
    for (version, compression) in [("file", 0), ("v003", 1), ("v011", 0), ("v011", 1), ("v011", 2)] {
        let base = solid(4, 4, 0, &[255, 0, 0]);
        let mut top = solid(2, 2, 1, &[0, 255, 0, 255]);
        top.off = (1, 1);
        top.opacity = 128;
        let mut spill = solid(3, 3, 1, &[0, 0, 255, 255]);
        spill.off = (-2, 3); // mostly outside the canvas
        // The file lists layers top to bottom.
        let img = run(&xcf(version, 0, 4, 4, compression, &[], &[spill, top, base]), 100).unwrap();
        let (corner, blend, clipped) = (px(&img, 0, 0), px(&img, 1, 1), px(&img, 0, 3));
        let tag = format!("{version}/{compression}");
        assert_eq!(corner, [255, 0, 0, 255], "{tag}");
        assert!((120..136).contains(&blend[0]) && (120..136).contains(&blend[1]) && blend[2] == 0, "half-transparent green over red: {blend:?} {tag}");
        assert_eq!(clipped, [0, 0, 255, 255], "{tag}: the one visible pixel of the off-canvas layer");
    }
}

#[test]
fn hidden_group_and_transparent_layers_draw_nothing_and_masks_cut_the_layer() {
    let mut hidden = solid(4, 4, 0, &[0, 0, 255]);
    hidden.visible = false;
    let mut group = solid(4, 4, 0, &[0, 0, 255]);
    group.group = true;
    let mut clear = solid(4, 4, 0, &[0, 0, 255]);
    clear.opacity = 0;
    let mut masked = solid(2, 1, 1, &[0, 255, 0, 255]);
    masked.mask = Some(vec![0, 255]);
    let img = run(&xcf("v011", 0, 4, 4, 0, &[], &[hidden, group, clear, masked, solid(4, 4, 0, &[255, 0, 0])]), 100).unwrap();
    assert_eq!(px(&img, 3, 3), [255, 0, 0, 255], "none of the blue layers show");
    assert_eq!(px(&img, 0, 0), [255, 0, 0, 255], "mask 0 keeps the layer out");
    assert_eq!(px(&img, 1, 0), [0, 255, 0, 255], "mask 255 lets it through");
}

#[test]
fn grey_grey_alpha_and_indexed_layers_and_multi_tile_levels() {
    let palette = [[0, 0, 0], [200, 100, 50]];
    let img = run(&xcf("v011", 2, 70, 3, 1, &palette, &[layer(70, 3, 4, (0..210).map(|i| (i % 70 >= 65) as u8).collect())]), 1000).unwrap();
    assert_eq!((px(&img, 0, 0), px(&img, 69, 2)), ([0, 0, 0, 255], [200, 100, 50, 255]), "indexed colours across a 64 px tile edge");
    let grey = run(&xcf("v011", 1, 2, 1, 0, &[], &[layer(2, 1, 3, vec![10, 255, 20, 0])]), 100).unwrap();
    assert_eq!((px(&grey, 0, 0), px(&grey, 1, 0)), ([10, 10, 10, 255], [0, 0, 0, 0]));
}

#[test]
fn junk_wide_precision_huge_truncated_and_runaway_files_are_errors() {
    assert!(run(b"definitely not a gimp file", 100).is_err());
    let ok = xcf("v011", 0, 4, 4, 0, &[], &[solid(4, 4, 0, &[1, 2, 3])]);
    assert!(run(&ok[..ok.len() - 20], 100).unwrap_err().contains("truncated"));
    let mut deep = ok.clone();
    deep[26..30].copy_from_slice(&200u32.to_be_bytes()); // 16-bit precision
    assert!(run(&deep, 100).unwrap_err().contains("8-bit"));
    assert_eq!(run(&ok, 10).unwrap_err(), crate::decode::TOO_LARGE);
    let mut zip3 = ok.clone();
    let at = zip3.windows(8).position(|w| w == [0, 0, 0, 17, 0, 0, 0, 1]).unwrap() + 8;
    zip3[at] = 3;
    assert!(run(&zip3, 100).unwrap_err().contains("compression"));
    let mut many = xcf("v011", 0, 4, 4, 0, &[], &[]);
    let list = many.len() - 16;
    many.truncate(list);
    many.extend(std::iter::repeat_n([0u8, 0, 0, 0, 0, 0, 0, 9], 600).flatten());
    assert!(run(&many, 100).unwrap_err().contains("too many layers"), "a pointer list with no end is cut at the layer cap");
}

#[test]
fn a_layer_whose_hierarchy_or_mask_disagrees_with_its_header_is_refused() {
    let mut small = solid(2, 2, 0, &[1, 2, 3]);
    small.hier_size = Some((1, 1));
    small.px = vec![1, 2, 3];
    assert!(run(&xcf("v011", 0, 4, 4, 0, &[], &[small]), 100).unwrap_err().contains("not a valid XCF"));
    let mut thin_mask = solid(2, 1, 1, &[0, 255, 0, 255]);
    thin_mask.mask = Some(vec![255]);
    thin_mask.mask_size = Some((1, 1));
    assert!(run(&xcf("v011", 0, 4, 4, 0, &[], &[thin_mask]), 100).unwrap_err().contains("not a valid XCF"));
}

#[test]
fn a_giant_layer_off_the_canvas_costs_nothing_and_a_missing_tile_list_stays_transparent() {
    // 1 000 000 x 1 000 000 px announced, a single pixel stored: refused by size, never iterated.
    let mut giant = solid(1, 1, 1, &[0, 255, 0, 255]);
    giant.w = 1_000_000;
    giant.h = 1_000_000;
    giant.hier_size = Some((1, 1));
    giant.off = (i32::MAX, 0);
    assert!(run(&xcf("v011", 0, 4, 4, 0, &[], &[giant]), 100).is_err());
    let mut unwritten = solid(4, 4, 1, &[0, 255, 0, 255]);
    unwritten.skip_tiles = true;
    let img = run(&xcf("v011", 0, 4, 4, 0, &[], &[unwritten, solid(4, 4, 0, &[255, 0, 0])]), 100).unwrap();
    assert_eq!(px(&img, 2, 2), [255, 0, 0, 255], "tiles that were never written do not paint");
}

#[test]
fn zlib_tiles_that_are_truncated_or_short_are_errors() {
    let ok = xcf("v011", 0, 4, 4, 2, &[], &[solid(4, 4, 0, &[1, 2, 3])]);
    assert_eq!(px(&run(&ok, 100).unwrap(), 1, 1), [1, 2, 3, 255]);
    assert!(run(&ok[..ok.len() - 6], 100).is_err(), "cut inside the zlib stream");
}

#[test]
fn a_mask_can_be_switched_off_and_hidden_groups_hide_their_children() {
    let mut off = solid(1, 1, 1, &[0, 255, 0, 255]);
    off.mask = Some(vec![0]);
    off.apply_mask = false;
    let img = run(&xcf("v011", 0, 1, 1, 0, &[], &[off]), 100).unwrap();
    assert_eq!(px(&img, 0, 0), [0, 255, 0, 255], "PROP_APPLY_MASK = 0 ignores the mask");
    // File order is top to bottom: child (path [0, 0]) inside group (top-level index 0), over a red background.
    let mut child = solid(1, 1, 0, &[0, 0, 255]);
    child.path = Some(vec![0, 0]);
    let mut group = solid(1, 1, 0, &[0, 0, 0]);
    group.group = true;
    group.visible = false;
    let hidden = run(&xcf("v011", 0, 1, 1, 0, &[], &[child, group, solid(1, 1, 0, &[255, 0, 0])]), 100).unwrap();
    assert_eq!(px(&hidden, 0, 0), [255, 0, 0, 255], "the child of a hidden group does not show");
    let mut child = solid(1, 1, 0, &[0, 0, 255]);
    child.path = Some(vec![0, 0]);
    let mut group = solid(1, 1, 0, &[0, 0, 0]);
    group.group = true;
    group.opacity = 0; // group opacity multiplies into its children
    let faded = run(&xcf("v011", 0, 1, 1, 0, &[], &[child, group, solid(1, 1, 0, &[255, 0, 0])]), 100).unwrap();
    assert_eq!(px(&faded, 0, 0), [255, 0, 0, 255]);
    let mut child = solid(1, 1, 0, &[0, 0, 255]);
    child.path = Some(vec![0, 0]);
    let mut group = solid(1, 1, 0, &[0, 0, 0]);
    group.group = true;
    let shown = run(&xcf("v011", 0, 1, 1, 0, &[], &[child, group, solid(1, 1, 0, &[255, 0, 0])]), 100).unwrap();
    assert_eq!(px(&shown, 0, 0), [0, 0, 255, 255], "a visible group lets the child through");
}
