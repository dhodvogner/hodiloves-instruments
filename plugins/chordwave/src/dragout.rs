//! Drag a file out of the plugin window into the host (FL Studio's piano roll, a channel, the Playlist...).
//!
//! On Windows this starts a regular shell file drag: the host sees the same drop as when a file is dragged
//! from Explorer. Other platforms report `false` and the editor falls back to saving the file.
//!
//! The drag runs on its **own thread**. The window library calls the editor's frame handler while it holds
//! an exclusive borrow of it; the drag blocks in a message loop until the drop, and that loop delivers more
//! window messages to the same handler. Doing the drag inside the handler therefore panics ("already
//! borrowed") inside a Windows callback, which aborts the whole process (and the host with it).

use std::path::Path;

/// Start dragging `path`. Returns whether a drag was started (it keeps running in the background until
/// the drop or cancel).
#[cfg(windows)]
pub fn drag_file(path: &Path) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    static BUSY: AtomicBool = AtomicBool::new(false);

    if BUSY.swap(true, Ordering::AcqRel) {
        return true; // a drag is already in progress
    }
    let path = path.to_path_buf();
    let started = std::thread::Builder::new()
        .name("chordwave-drag".into())
        .spawn(move || {
            // A panic must never leave the thread (and must never reach the host).
            let _ = std::panic::catch_unwind(|| run_drag(&path));
            BUSY.store(false, Ordering::Release);
        })
        .is_ok();
    if !started {
        BUSY.store(false, Ordering::Release);
    }
    started
}

/// The drag itself, on a thread of its own with its own OLE apartment.
#[cfg(windows)]
fn run_drag(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{OleInitialize, OleUninitialize, DROPEFFECT_COPY};
    use windows::Win32::UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop};

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 path that outlives the calls. OLE is initialised
    // on this thread (a fresh single-threaded apartment, which drag and drop requires) and the COM
    // objects are released before it is uninitialised.
    unsafe {
        let ole = OleInitialize(None).is_ok();
        if ole {
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None) {
                if let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) {
                    // Copy only: the file stays where it is.
                    let _ = SHDoDragDrop(None, &data, None, DROPEFFECT_COPY);
                }
            }
            OleUninitialize();
        }
    }
}

#[cfg(not(windows))]
pub fn drag_file(_path: &Path) -> bool {
    false
}

/// Whether `drag_file` can do anything on this platform.
pub const SUPPORTED: bool = cfg!(windows);
