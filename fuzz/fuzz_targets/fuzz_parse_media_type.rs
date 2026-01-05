#![no_main]

use libfuzzer_sys::fuzz_target;
use tokio_mime::parse_media_type;

fuzz_target!(|data: &[u8]| {
    // Convert bytes to string
    if let Ok(s) = std::str::from_utf8(data) {
        // Try to parse the media type
        let _ = parse_media_type(s);
    }
});
