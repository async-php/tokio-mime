#![no_main]

use libfuzzer_sys::fuzz_target;
use mime_rs::quotedprintable::Reader;
use tokio::runtime::Runtime;
use tokio::io::AsyncReadExt;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let cursor = Cursor::new(data);
        let mut reader = Reader::new(cursor);
        let mut output = Vec::new();

        // Try to read up to 1MB to avoid memory exhaustion
        let _ = reader.take(1024 * 1024).read_to_end(&mut output).await;
    });
});
