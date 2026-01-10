# yamine

[![Crates.io](https://img.shields.io/crates/v/yamine.svg)](https://crates.io/crates/yamine)
[![Documentation](https://docs.rs/yamine/badge.svg)](https://docs.rs/yamine)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/workflow/status/async-php/yamine/CI)](https://github.com/async-php/yamine/actions)

> Complete Rust port of Go's `mime` package with async-first design

A comprehensive MIME handling library for Rust, providing full support for MIME type detection, media type parsing, multipart messages, quoted-printable encoding, and RFC 2047 encoded-words. Built with async/await and tokio for modern Rust applications.

## Features

- 🎯 **MIME Type Detection** - File extension to MIME type mapping with 1000+ built-in types
- 📝 **Media Type Parsing** - RFC 2045/2616/2231 compliant media type handling
- 📮 **Multipart Messages** - Full multipart/form-data and multipart/mixed support
- 🔤 **Encoded Words** - RFC 2047 encoded-word encoding/decoding for email headers
- ✉️ **Quoted-Printable** - RFC 2045 quoted-printable encoding/decoding
- ⚡ **Async First** - Built on tokio for high-performance async I/O
- 🦀 **Pure Rust** - No unsafe code, fully type-safe
- 🧪 **Well Tested** - 121+ tests with 73.78% code coverage

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
yamine = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### MIME Type Detection

```rust
use yamine::type_by_extension;

// Get MIME type by file extension
let mime_type = type_by_extension(".html");
assert_eq!(mime_type, Some("text/html; charset=utf-8".to_string()));

let mime_type = type_by_extension(".jpg");
assert_eq!(mime_type, Some("image/jpeg".to_string()));
```

### Media Type Parsing

```rust
use yamine::parse_media_type;

let (media_type, params) = parse_media_type("text/html; charset=utf-8").unwrap();
assert_eq!(media_type, "text/html");
assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
```

### Multipart Form Data

```rust
use yamine::multipart::Writer;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut writer = Writer::new(&mut output);

    // Add a text field
    writer.write_field("username", "john_doe").await?;

    // Add a file
    let mut file_writer = writer.create_form_file("avatar", "photo.jpg").await?;
    file_writer.write_all(b"image data here").await?;

    writer.close().await?;

    println!("Multipart data created: {} bytes", output.len());
    Ok(())
}
```

### Reading Multipart Data

```rust
use yamine::multipart::Reader;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = b"--boundary\r\n\
Content-Disposition: form-data; name=\"field\"\r\n\
\r\n\
value\r\n\
--boundary--\r\n";

    let mut reader = Reader::new(&data[..], "boundary");

    while let Some(mut part) = reader.next_part().await? {
        println!("Field name: {:?}", part.form_name());

        let mut content = String::new();
        part.read_to_string(&mut content).await?;
        println!("Content: {}", content);
    }

    Ok(())
}
```

### Quoted-Printable Encoding

```rust
use yamine::quotedprintable::Writer;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut writer = Writer::new(&mut output);

    writer.write_all(b"Hello, World! Special chars: =").await?;
    writer.close().await?;

    println!("Encoded: {}", String::from_utf8_lossy(&output));
    // Output: "Hello, World! Special chars: =3D"

    Ok(())
}
```

### Encoded Words (Email Headers)

```rust
use yamine::{WordEncoder, WordDecoder};

// Encoding
let encoder = WordEncoder::QEncoding;
let encoded = encoder.encode("UTF-8", "Hello, 世界!");
println!("Encoded: {}", encoded);
// Output: =?UTF-8?q?Hello,_=E4=B8=96=E7=95=8C!?=

// Decoding
let decoder = WordDecoder::new();
let decoded = decoder.decode(&encoded).unwrap();
assert_eq!(decoded, "Hello, 世界!");
```

## API Overview

### Core Modules

- **`mime_type`** - MIME type detection and extension mapping
- **`media_type`** - Media type parsing and formatting (RFC 2045/2616/2231)
- **`multipart`** - Multipart message handling (RFC 2046/2388)
  - `Reader` - Parse multipart messages
  - `Writer` - Create multipart messages
  - `Form` - Multipart form data support
- **`quotedprintable`** - Quoted-printable encoding (RFC 2045)
  - `Reader` - Decode quoted-printable
  - `Writer` - Encode quoted-printable
- **`encoded_word`** - RFC 2047 encoded-word support
  - `WordEncoder` - Encode headers
  - `WordDecoder` - Decode headers
- **`error`** - Error types and result definitions

### Platform Support

The library includes platform-specific MIME type loading:
- **Unix/Linux/macOS** - Loads from `/etc/mime.types` and other standard locations
- **Windows** - Reads from Windows Registry

## Performance

The library includes comprehensive benchmarks using Criterion:

```bash
cargo bench
```

Example benchmark results:
- Media type parsing: ~125 ns per operation
- Quoted-printable encoding (1KB): ~45 µs (22 MiB/s)
- Multipart writing (5 parts): ~150 µs

## Testing

### Run Tests

```bash
# All tests (121+ tests)
cargo test

# Unit tests only
cargo test --lib

# Integration tests
cargo test --test integration_tests

# With coverage
cargo tarpaulin --out Html
```

### Test Coverage

Current test coverage: **73.78%** (695/942 lines)

- ✅ **error.rs**: 100%
- ✅ **mime_type.rs**: 97%
- ✅ **encoded_word.rs**: ~88%
- ✅ **quotedprintable/writer.rs**: ~85%
- ✅ **multipart/writer.rs**: 81%

See [TESTING_GUIDE.md](TESTING_GUIDE.md) for detailed testing documentation.

### Fuzzing

The project includes fuzz tests using cargo-fuzz:

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Run fuzzing (examples)
cargo fuzz run fuzz_parse_media_type -- -max_total_time=60
cargo fuzz run fuzz_multipart_reader -- -max_total_time=60
cargo fuzz run fuzz_encoded_word -- -max_total_time=60
cargo fuzz run fuzz_quoted_printable -- -max_total_time=60
```

## Examples

See the `examples/` directory for more complete examples:

- **`basic_mime_type.rs`** - MIME type detection
- **`multipart_form.rs`** - Creating multipart forms
- **`email_headers.rs`** - Encoding email headers
- **`file_upload.rs`** - Handling file uploads

Run examples:

```bash
cargo run --example basic_mime_type
```

## Comparison with Other Libraries

| Feature | yamine | mime | mailparse |
|---------|---------|------|-----------|
| MIME type detection | ✅ | ❌ | ❌ |
| Media type parsing | ✅ | ✅ | ✅ |
| Multipart messages | ✅ | ❌ | ✅ |
| Quoted-printable | ✅ | ❌ | ✅ |
| Encoded words (RFC 2047) | ✅ | ❌ | ✅ |
| Async/await | ✅ | ❌ | ❌ |
| Form data | ✅ | ❌ | ❌ |
| Writing support | ✅ | ❌ | ❌ |

## Requirements

- **Rust**: 1.70 or later
- **Tokio**: 1.35 or later (async runtime)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/async-php/yamine.git
cd yamine

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy -- -D warnings
```

### Guidelines

1. Write tests for new features
2. Maintain or improve code coverage
3. Follow Rust naming conventions
4. Add documentation for public APIs
5. Run `cargo fmt` before committing

## License

This project is dual-licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## Acknowledgments

- Inspired by Go's `mime` package
- Built with [tokio](https://tokio.rs/) async runtime
- Tested with [Criterion](https://github.com/bheisler/criterion.rs) benchmark framework
- Fuzzed with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)

## Resources

- [Documentation](https://docs.rs/yamine)
- [Crates.io](https://crates.io/crates/yamine)
- [Repository](https://github.com/async-php/yamine)
- [Issue Tracker](https://github.com/async-php/yamine/issues)
- [Author](https://github.com/hackmasker)

### RFCs Implemented

- [RFC 2045](https://tools.ietf.org/html/rfc2045) - MIME Part One: Format of Internet Message Bodies
- [RFC 2046](https://tools.ietf.org/html/rfc2046) - MIME Part Two: Media Types
- [RFC 2047](https://tools.ietf.org/html/rfc2047) - MIME Part Three: Message Header Extensions
- [RFC 2231](https://tools.ietf.org/html/rfc2231) - MIME Parameter Value and Encoded Word Extensions
- [RFC 2388](https://tools.ietf.org/html/rfc2388) - Returning Values from Forms: multipart/form-data
- [RFC 2616](https://tools.ietf.org/html/rfc2616) - HTTP/1.1 (Media Type handling)

---

**Made with ❤️ in Rust**
