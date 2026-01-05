#![no_main]

use libfuzzer_sys::fuzz_target;
use tokio_mime::WordDecoder;

fuzz_target!(|data: &[u8]| {
    // Convert bytes to string
    if let Ok(s) = std::str::from_utf8(data) {
        let decoder = WordDecoder::new();

        // Try to decode as an encoded word
        let _ = decoder.decode(s);

        // Try to decode as a header
        let _ = decoder.decode_header(s);
    }
});
