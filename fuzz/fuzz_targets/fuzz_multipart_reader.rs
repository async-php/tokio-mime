#![no_main]

use libfuzzer_sys::fuzz_target;
use mime_rs::multipart::Reader;
use tokio::runtime::Runtime;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    // Try to read the data as multipart with a random boundary
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let cursor = Cursor::new(data);
        let mut reader = Reader::new(cursor, "boundary");

        // Try to read up to 100 parts to avoid infinite loops
        for _ in 0..100 {
            match reader.next_part().await {
                Ok(Some(_part)) => {
                    // Successfully read a part
                }
                Ok(None) => {
                    // No more parts
                    break;
                }
                Err(_) => {
                    // Error reading part
                    break;
                }
            }
        }
    });
});
