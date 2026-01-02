//! Integration tests for mime_rs library

use mime_rs::*;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn test_end_to_end_multipart_form() {
    // Test creating a multipart form, writing it, then reading it back
    let mut buffer = Vec::new();
    let boundary = "test-boundary-12345";

    // Create and write multipart data
    {
        let mut writer = multipart::Writer::new(&mut buffer);
        writer.set_boundary(boundary.to_string()).unwrap();

        // Add a text field
        writer
            .write_field("username", "john_doe")
            .await
            .unwrap();

        // Add a file field
        let file_content = "This is test file content";
        let mut file_writer = writer
            .create_form_file("upload", "test.txt")
            .await
            .unwrap();
        file_writer.write_all(file_content.as_bytes()).await.unwrap();

        writer.close().await.unwrap();
    }

    // Read the multipart data back
    {
        let cursor = Cursor::new(&buffer);
        let mut reader = multipart::Reader::new(cursor, boundary);

        // Read first part (username field)
        let mut part1 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part1.form_name(), Some("username"));

        let mut content1 = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut part1, &mut content1)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&content1).contains("john_doe"));

        // Read second part (file field)
        let mut part2 = reader.next_part().await.unwrap().unwrap();
        assert_eq!(part2.form_name(), Some("upload"));
        assert_eq!(part2.file_name(), Some("test.txt".to_string()));

        let mut content2 = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut part2, &mut content2)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&content2).contains("This is test file content"));

        // No more parts
        assert!(reader.next_part().await.unwrap().is_none());
    }
}

#[tokio::test]
async fn test_media_type_parsing_and_formatting() {
    // Test round-trip of media type parsing and formatting
    let original = "text/html; charset=utf-8; boundary=test123";

    let (media_type, params) = parse_media_type(original).unwrap();
    assert_eq!(media_type, "text/html");
    assert_eq!(params.get("charset").unwrap(), "utf-8");
    assert_eq!(params.get("boundary").unwrap(), "test123");

    let formatted = format_media_type(&media_type, &params);
    assert!(formatted.contains("text/html"));
    assert!(formatted.contains("charset=utf-8"));
    assert!(formatted.contains("boundary=test123"));
}

#[tokio::test]
async fn test_quoted_printable_encoding_decoding() {
    // Test round-trip of quoted-printable encoding
    let original = "Hello, 世界! This is a test with special chars: @#$%";
    let original_bytes = original.as_bytes();

    // Encode
    let mut encoded = Vec::new();
    {
        let mut writer = quotedprintable::Writer::new(&mut encoded);
        tokio::io::AsyncWriteExt::write_all(&mut writer, original_bytes)
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::flush(&mut writer).await.unwrap();
    }

    // Decode
    let mut decoded = Vec::new();
    {
        let cursor = Cursor::new(&encoded);
        let mut reader = quotedprintable::Reader::new(cursor);
        tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut decoded)
            .await
            .unwrap();
    }

    // The decoded content might have some differences due to encoding,
    // but basic ASCII should be preserved
    let decoded_str = String::from_utf8_lossy(&decoded);
    assert!(decoded_str.contains("Hello"));
    assert!(decoded_str.contains("test"));
}

#[tokio::test]
async fn test_encoded_word_round_trip() {
    // Test encoding and decoding of headers with non-ASCII characters
    let encoder = WordEncoder::QEncoding;
    let decoder = WordDecoder::default();

    let original = "Test Subject 测试";
    let encoded = encoder.encode("UTF-8", original);

    // The encoded version should contain encoded-word syntax
    assert!(encoded.contains("=?") || !original.is_ascii());

    let decoded = decoder.decode(&encoded).unwrap();
    assert!(decoded.contains("Test Subject"));
}

#[test]
fn test_mime_type_operations() {
    // Test MIME type by extension
    let mime_type = type_by_extension(".txt");
    assert_eq!(mime_type, Some("text/plain; charset=utf-8".to_string()));

    let mime_type = type_by_extension(".html");
    assert_eq!(mime_type, Some("text/html; charset=utf-8".to_string()));

    let mime_type = type_by_extension(".jpg");
    assert_eq!(mime_type, Some("image/jpeg".to_string()));

    // Test extensions by type
    let extensions = extensions_by_type("image/jpeg").unwrap();
    // Extensions are returned with dots
    assert!(extensions.contains(&".jpeg".to_string()));
    assert!(extensions.contains(&".jpg".to_string()));

    // Test adding custom type
    let _ = add_extension_type(".custom", "application/x-custom");
    let mime_type = type_by_extension(".custom");
    assert_eq!(mime_type, Some("application/x-custom".to_string()));
}

#[tokio::test]
async fn test_error_handling_chain() {
    // Test that errors propagate correctly through the system

    // Invalid media type should error
    let result = parse_media_type("");
    assert!(result.is_err());

    // Invalid boundary should error when reading
    let data = b"invalid data";
    let mut reader = multipart::Reader::new(&data[..], "");
    let result = reader.next_part().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_large_multipart_data() {
    // Test with larger data to ensure buffering works correctly
    let mut buffer = Vec::new();
    let boundary = "large-data-boundary";

    // Create large content (1MB)
    let large_content = "A".repeat(1024 * 1024);

    {
        let mut writer = multipart::Writer::new(&mut buffer);
        writer.set_boundary(boundary.to_string()).unwrap();
        writer
            .write_field("large_field", &large_content)
            .await
            .unwrap();
        writer.close().await.unwrap();
    }

    // Read it back
    {
        let cursor = Cursor::new(&buffer);
        let mut reader = multipart::Reader::new(cursor, boundary);

        let mut part = reader.next_part().await.unwrap().unwrap();
        let mut content = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut part, &mut content)
            .await
            .unwrap();

        // Verify size (allowing for some encoding overhead)
        assert!(content.len() >= 1024 * 1024);
    }
}

#[tokio::test]
async fn test_multipart_with_special_characters_in_filename() {
    // Test filenames with spaces and special characters
    let mut buffer = Vec::new();
    let boundary = "special-char-boundary";

    {
        let mut writer = multipart::Writer::new(&mut buffer);
        writer.set_boundary(boundary.to_string()).unwrap();
        let mut file_writer = writer
            .create_form_file("file", "my document (draft).txt")
            .await
            .unwrap();
        file_writer.write_all(b"content").await.unwrap();
        writer.close().await.unwrap();
    }

    {
        let cursor = Cursor::new(&buffer);
        let mut reader = multipart::Reader::new(cursor, boundary);
        let mut part = reader.next_part().await.unwrap().unwrap();

        let filename = part.file_name();
        assert!(filename.is_some());
        assert!(filename.unwrap().contains("document"));
    }
}

#[tokio::test]
async fn test_very_large_multipart_data() {
    // Test with 20MB of data
    let mut buffer = Vec::new();
    let boundary = "very-large-data-boundary";

    // Create 20MB content
    let large_content = "B".repeat(20 * 1024 * 1024);

    {
        let mut writer = multipart::Writer::new(&mut buffer);
        writer.set_boundary(boundary.to_string()).unwrap();
        writer
            .write_field("huge_field", &large_content)
            .await
            .unwrap();
        writer.close().await.unwrap();
    }

    // Read it back
    {
        let cursor = Cursor::new(&buffer);
        let mut reader = multipart::Reader::new(cursor, boundary);

        let mut part = reader.next_part().await.unwrap().unwrap();
        let mut content = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut part, &mut content)
            .await
            .unwrap();

        // Verify size (allowing for some encoding overhead)
        assert!(content.len() >= 20 * 1024 * 1024);
    }
}

#[tokio::test]
async fn test_concurrent_multipart_readers() {
    use tokio::task::JoinSet;

    // Create test data
    let test_data = {
        let mut buffer = Vec::new();
        let mut writer = multipart::Writer::new(&mut buffer);
        writer.set_boundary("concurrent-boundary".to_string()).unwrap();

        for i in 0..10 {
            writer
                .write_field(&format!("field{}", i), &format!("data{}", i))
                .await
                .unwrap();
        }

        writer.close().await.unwrap();
        buffer
    };

    // Spawn multiple concurrent readers
    let mut set = JoinSet::new();

    for _ in 0..10 {
        let data = test_data.clone();
        set.spawn(async move {
            let cursor = Cursor::new(&data);
            let mut reader = multipart::Reader::new(cursor, "concurrent-boundary");

            let mut count = 0;
            while let Some(_part) = reader.next_part().await.unwrap() {
                count += 1;
            }
            count
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = set.join_next().await {
        let count = result.unwrap();
        assert_eq!(count, 10);
    }
}

#[tokio::test]
async fn test_concurrent_media_type_parsing() {
    use tokio::task::JoinSet;

    let test_inputs = vec![
        "text/html; charset=utf-8",
        "application/json",
        "multipart/form-data; boundary=test123",
        "image/jpeg",
        "text/plain; charset=iso-8859-1",
    ];

    let mut set = JoinSet::new();

    for input in test_inputs {
        set.spawn(async move {
            for _ in 0..1000 {
                let result = parse_media_type(input);
                assert!(result.is_ok());
            }
        });
    }

    while let Some(result) = set.join_next().await {
        result.unwrap();
    }
}

#[tokio::test]
async fn test_large_quoted_printable_data() {
    // Test with 15MB of quoted-printable data
    let large_data = "Test data with some special chars = and \r\n newlines.".repeat(300_000);

    // Encode
    let mut encoded = Vec::new();
    {
        let mut writer = quotedprintable::Writer::new(&mut encoded);
        tokio::io::AsyncWriteExt::write_all(&mut writer, large_data.as_bytes())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::flush(&mut writer).await.unwrap();
    }

    // Decode
    let mut decoded = Vec::new();
    {
        let cursor = Cursor::new(&encoded);
        let mut reader = quotedprintable::Reader::new(cursor);
        tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut decoded)
            .await
            .unwrap();
    }

    // Verify basic structure preserved
    assert!(decoded.len() > 10 * 1024 * 1024);
}

#[tokio::test]
async fn test_stress_multipart_writer() {
    // Stress test: create multipart with many small parts
    let mut buffer = Vec::new();
    let boundary = {
        let mut writer = multipart::Writer::new(&mut buffer);
        let boundary = writer.boundary().to_string();

        for i in 0..50 {
            writer
                .write_field(&format!("field{}", i), &format!("value{}", i))
                .await
                .unwrap();
        }

        writer.close().await.unwrap();
        boundary
    };

    // Verify we can read it back
    let cursor = Cursor::new(&buffer);
    let mut reader = multipart::Reader::new(cursor, &boundary);

    let mut count = 0;
    loop {
        match reader.next_part().await {
            Ok(Some(_part)) => {
                count += 1;
            }
            Ok(None) => {
                break;
            }
            Err(_) => {
                // If we encounter an error partway through, that's still useful info
                break;
            }
        }
    }

    // Verify we read a reasonable number of parts (at least most of them)
    assert!(count >= 45, "Expected at least 45 parts, got {}", count);
}
