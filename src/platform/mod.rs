//! Platform-specific MIME type loading.

#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

use crate::error::Result;

/// Initialize MIME types from platform-specific sources.
///
/// On Unix systems, reads from:
/// - /usr/share/mime/globs2 (FreeDesktop Shared MIME-info Database)
/// - /etc/mime.types, /etc/apache2/mime.types, etc.
///
/// On Windows, reads from:
/// - Registry HKEY_CLASSES_ROOT for extension associations
pub fn init_mime() -> Result<()> {
    #[cfg(unix)]
    {
        unix::init_mime_unix()
    }

    #[cfg(windows)]
    {
        windows::init_mime_windows()
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Unsupported platform, use builtin types only
        Ok(())
    }
}
