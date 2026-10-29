//! Win32 side of the splash: one click-through layered window showing the decoded picture.

use super::{geometry, is_dismissed, update};
use crate::{decode, resize};
use fast_image_resize::FilterType;
use image::DynamicImage;
use std::path::Path;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetStockObject, ReleaseDC,
    SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLACK_BRUSH,
    BLENDFUNCTION, DIB_RGB_COLORS, HBRUSH, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW, PeekMessageW,
    RegisterClassExW, SetLayeredWindowAttributes, ShowWindow, SystemParametersInfoW,
    TranslateMessage, UpdateLayeredWindow, LWA_ALPHA, MSG, PM_REMOVE, SPI_GETWORKAREA,
    SW_SHOWNOACTIVATE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ULW_ALPHA, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// Safety net: a splash never outlives this, whatever the page does.
const MAX_LIFE: Duration = Duration::from_secs(5);

/// `tauri-plugin-single-instance` keeps a window with this title while an instance is running.
pub fn another_instance_running() -> bool {
    unsafe { FindWindowW(PCWSTR::null(), w!("dev.juninmd.vislo-siw")).is_ok() }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn work_area() -> Option<RECT> {
    let mut r = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut r as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()?;
    Some(r)
}

/// A click-through topmost popup of class `class` covering `rect` (the picture window starts 1x1
/// and is resized by every `UpdateLayeredWindow`).
fn create_window(class: PCWSTR, brush: HBRUSH, rect: RECT) -> Option<HWND> {
    unsafe {
        let hinst = GetModuleHandleW(None).ok()?.into();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst,
            lpszClassName: class,
            hbrBackground: brush,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let ex =
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TRANSPARENT;
        CreateWindowExW(
            ex,
            class,
            w!(""),
            WS_POPUP,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(hinst),
            None,
        )
        .ok()
    }
}

/// The darkening of the desktop behind the picture, as the page's transparent background does it.
fn create_backdrop(area: RECT) -> Option<HWND> {
    unsafe {
        let hwnd = create_window(
            w!("VisloSplashDim"),
            HBRUSH(GetStockObject(BLACK_BRUSH).0),
            area,
        )?;
        let alpha = (geometry::DIM_ALPHA * 255.0).round() as u8;
        SetLayeredWindowAttributes(hwnd, Default::default(), alpha, LWA_ALPHA).ok()?;
        Some(hwnd)
    }
}

/// Draws `img` (straight RGBA) with `alpha` (0..=1) so that its centre sits at `centre`.
fn present(hwnd: HWND, img: &image::RgbaImage, alpha: f32, centre: (i32, i32)) -> bool {
    let (w, h) = img.dimensions();
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let ok = match CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bmp) if !bits.is_null() => {
                geometry::premultiplied_bgra(
                    img.as_raw(),
                    std::slice::from_raw_parts_mut(bits.cast::<u8>(), (w * h * 4) as usize),
                );
                let old = SelectObject(mem, HGDIOBJ(bmp.0));
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let dst = POINT {
                    x: centre.0 - w as i32 / 2,
                    y: centre.1 - h as i32 / 2,
                };
                let size = SIZE {
                    cx: w as i32,
                    cy: h as i32,
                };
                let src = POINT::default();
                let drawn = UpdateLayeredWindow(
                    hwnd,
                    Some(screen),
                    Some(&dst),
                    Some(&size),
                    Some(mem),
                    Some(&src),
                    Default::default(),
                    Some(&blend),
                    ULW_ALPHA,
                )
                .is_ok();
                SelectObject(mem, old);
                let _ = DeleteObject(HGDIOBJ(bmp.0));
                drawn
            }
            _ => false,
        };
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        ok
    }
}

/// The real window is up and shows the same picture: the desktop dimming moves from this window to the
/// page over `FADE_MS`, the picture stays until the end.
fn fade_backdrop(backdrop: HWND) {
    let start = Instant::now();
    loop {
        let p = start.elapsed().as_secs_f32() / (geometry::FADE_MS as f32 / 1000.0);
        let alpha = (geometry::dim_alpha_at(p) * 255.0).round() as u8;
        unsafe {
            let _ = SetLayeredWindowAttributes(backdrop, Default::default(), alpha, LWA_ALPHA);
        }
        if p >= 1.0 {
            break;
        }
        pump();
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn pump() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Decodes `path`, plays the zoom-in, then holds the picture until the real window has taken over.
pub fn run(path: &Path) {
    unsafe {
        // The page lays out in device pixels; without this the splash would be scaled by Windows.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let Some(area) = work_area() else { return };
    let (aw, ah) = (
        (area.right - area.left).max(1) as u32,
        (area.bottom - area.top).max(1) as u32,
    );
    let Ok(img) = decode::load(path) else { return };
    if is_dismissed() {
        return;
    }
    let Ok(fitted) = resize::fit(&img, aw, ah, FilterType::Bilinear) else {
        return;
    };
    drop(img);
    let base = DynamicImage::ImageRgba8(fitted.to_rgba8());
    let (fw, fh) = (base.width(), base.height());
    let centre = geometry::centred((fw, fh), (area.left, area.top), (aw, ah));
    let centre = (centre.0 + fw as i32 / 2, centre.1 + fh as i32 / 2);
    let Some(backdrop) = create_backdrop(area) else {
        return;
    };
    let tiny = RECT {
        left: 0,
        top: 0,
        right: 1,
        bottom: 1,
    };
    let Some(hwnd) = create_window(w!("VisloSplash"), HBRUSH::default(), tiny) else {
        unsafe {
            let _ = DestroyWindow(backdrop);
        }
        return;
    };

    let born = Instant::now();
    let mut visible = false;
    loop {
        if is_dismissed() && !visible {
            break; // the real window won the race; nothing to hand over
        }
        let t = born.elapsed().as_secs_f32() / (geometry::DURATION_MS as f32 / 1000.0);
        let (scale, alpha) = geometry::pose(geometry::ease_out(t));
        let frame = if scale >= 0.999 {
            base.to_rgba8()
        } else {
            let (sw, sh) = (
                (fw as f32 * scale).round() as u32,
                (fh as f32 * scale).round() as u32,
            );
            match resize::fit(&base, sw.max(1), sh.max(1), FilterType::Bilinear) {
                Ok(s) => s.to_rgba8(),
                Err(_) => break,
            }
        };
        if !present(hwnd, &frame, alpha, centre) {
            break;
        }
        if !visible {
            visible = true;
            unsafe {
                let _ = ShowWindow(backdrop, SW_SHOWNOACTIVATE);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            update(|s| s.shown = true);
        }
        if t >= 1.0 {
            break;
        }
        pump();
        std::thread::sleep(Duration::from_millis(2));
    }
    update(|s| s.animation_done = true);
    if visible {
        while !is_dismissed() && born.elapsed() < MAX_LIFE {
            pump();
            std::thread::sleep(Duration::from_millis(2)); // the page starts its own fade right after the show
        }
        fade_backdrop(backdrop);
    }
    unsafe {
        let _ = DestroyWindow(hwnd);
        let _ = DestroyWindow(backdrop);
    }
    update(|s| s.shown = false);
}
