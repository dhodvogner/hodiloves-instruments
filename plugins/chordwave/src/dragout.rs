//! Drag a file out of the plugin window into the host (FL Studio's piano roll, a channel, the Playlist...).
//!
//! On Windows this starts a regular shell file drag: the host sees the same drop as when a file is dragged
//! from Explorer. Other platforms report `false` and the editor falls back to saving the file.

use std::path::Path;

/// Start dragging `path`. Blocks until the drop (or cancel) is over. Returns whether a drag happened.
#[cfg(windows)]
pub fn drag_file(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Ole::{OleInitialize, DROPEFFECT_COPY};
    use windows::Win32::UI::Shell::{BHID_DataObject, SHCreateItemFromParsingName, SHDoDragDrop, IShellItem};
    use windows::Win32::System::Com::IDataObject;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 path that outlives the calls; the COM objects are
    // released when they go out of scope; all calls happen on the editor's UI thread.
    unsafe {
        // The host usually initialised OLE already; a second call just reports that.
        let _ = OleInitialize(None);
        let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None) else { return false };
        let Ok(data) = item.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) else { return false };
        // Copy only: the file stays where it is.
        SHDoDragDrop(None, &data, None, DROPEFFECT_COPY).is_ok()
    }
}

#[cfg(not(windows))]
pub fn drag_file(_path: &Path) -> bool {
    false
}

/// Whether `drag_file` can do anything on this platform.
pub const SUPPORTED: bool = cfg!(windows);
