//! Windows-specific MIME type loading.
//!
//! Reads file extension associations from the Windows registry.

use crate::error::Result;
use crate::mime_type::set_extension_type_skip_existing;
use winreg::enums::*;
use winreg::RegKey;

/// Initialize MIME types from Windows registry.
///
/// Reads HKEY_CLASSES_ROOT for extension associations and their Content-Type values.
pub(super) fn init_mime_windows() -> Result<()> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    
    // Get all subkey names from HKEY_CLASSES_ROOT
    for name_result in hkcr.enum_keys() {
        let name = match name_result {
            Ok(n) => n,
            Err(_) => continue,
        };
        
        // Only process extension keys (start with ".")
        if name.len() < 2 || !name.starts_with('.') {
            continue;
        }
        
        // Open the extension key
        let key = match hkcr.open_subkey_with_flags(&name, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };
        
        // Read the "Content Type" value
        let content_type: String = match key.get_value("Content Type") {
            Ok(v) => v,
            Err(_) => continue,
        };
        
        // Special handling for .js extension (Windows registry bug)
        // See Go issue #32350: Windows sometimes incorrectly sets .js to text/plain
        if name == ".js" && (content_type == "text/plain" || content_type == "text/plain; charset=utf-8") {
            continue;
        }
        
        // Add the extension type (skip if already exists to preserve builtins)
        let _ = set_extension_type_skip_existing(&name, &content_type);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_mime_windows() {
        // Should not panic and complete without error
        let result = init_mime_windows();
        assert!(result.is_ok());
    }
}
