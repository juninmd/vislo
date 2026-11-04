//! The Windows share sheet (Mail, Nearby sharing, apps that accept pictures) for one file.

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use std::sync::{Mutex, Once};
    use windows::core::{factory, Interface, HSTRING};
    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows_collections::IIterable;

    /// The file the next `DataRequested` event hands to the share sheet.
    static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

    /// A `StorageFile` is agile (usable from any apartment); the bindings just do not say so.
    pub struct Pending(StorageFile, HSTRING);
    // SAFETY: StorageFile is an agile WinRT class, and HSTRING is an immutable reference-counted string.
    unsafe impl Send for Pending {}
    static HOOKED: Once = Once::new();

    /// Resolves the file for the sheet. Blocks, so it must not run on the UI thread.
    pub fn open(path: &Path) -> Result<Pending, String> {
        let fail = |e: windows::core::Error| e.to_string();
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.to_string_lossy().as_ref()))
            .map_err(fail)?
            .join()
            .map_err(fail)?;
        let name = file.Name().map_err(fail)?;
        Ok(Pending(file, name))
    }

    /// Opens the sheet over `hwnd`. Must run on the thread that owns the window.
    pub fn show(hwnd: isize, item: Pending) -> Result<(), String> {
        let fail = |e: windows::core::Error| e.to_string();
        let interop: IDataTransferManagerInterop = factory::<DataTransferManager, _>().map_err(fail)?;
        let hwnd = HWND(hwnd as *mut _);
        // SAFETY: `hwnd` is the live viewer window, and we are on its thread.
        let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }.map_err(fail)?;
        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(item);
        // One handler per process: the manager of a window is shared, so adding one per share would fire them all.
        let mut hook = Ok(());
        HOOKED.call_once(|| {
            let handler = TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(|_, args| {
                let Some(args) = args.as_ref() else { return Ok(()) };
                let Some(Pending(file, name)) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take() else {
                    return Ok(());
                };
                let data = args.Request()?.Data()?;
                data.Properties()?.SetTitle(&name)?;
                // The bitmap serves targets that want pixels; the file keeps the original for those that want a file.
                data.SetBitmap(&RandomAccessStreamReference::CreateFromFile(&file)?)?;
                let items = IIterable::<IStorageItem>::from(vec![Some(file.cast::<IStorageItem>()?)]);
                data.SetStorageItems(&items, true)?;
                Ok(())
            });
            hook = manager.DataRequested(&handler).map(|_| ()).map_err(fail);
        });
        hook?;
        // SAFETY: as above.
        unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(fail)
    }
}

#[cfg(windows)]
pub use imp::{open, show};

/// Opens the system share sheet for `path` over the viewer window.
#[cfg(windows)]
pub fn share_file(window: &tauri::WebviewWindow, path: &std::path::Path) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let item = open(path)?; // resolving the file waits on WinRT: not on the UI thread
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .run_on_main_thread(move || {
            let _ = tx.send(show(hwnd, item)); // the sheet belongs to the window's own thread
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

#[cfg(not(windows))]
pub fn share_file(_window: &tauri::WebviewWindow, _path: &std::path::Path) -> Result<(), String> {
    Err("sharing is Windows-only".into())
}
