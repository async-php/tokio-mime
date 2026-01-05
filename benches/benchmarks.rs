use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use tokio_mime::*;
use std::io::Cursor;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

// Benchmark media type parsing
fn bench_parse_media_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_media_type");

    let test_cases = vec![
        ("simple", "text/html"),
        ("with_charset", "text/html; charset=utf-8"),
        ("complex", "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW"),
    ];

    for (name, input) in test_cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, &input| {
            b.iter(|| {
                parse_media_type(black_box(input))
            });
        });
    }

    group.finish();
}

// Benchmark media type formatting
fn bench_format_media_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_media_type");

    let mut params = std::collections::HashMap::new();
    params.insert("charset".to_string(), "utf-8".to_string());
    params.insert("boundary".to_string(), "----boundary".to_string());

    group.bench_function("with_params", |b| {
        b.iter(|| {
            format_media_type(black_box("multipart/form-data"), black_box(&params))
        });
    });

    group.finish();
}

// Benchmark encoded word operations
fn bench_encoded_word(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoded_word");

    let q_encoder = WordEncoder::QEncoding;
    let b_encoder = WordEncoder::BEncoding;
    let decoder = WordDecoder::new();

    let test_text = "Hello, 世界! This is a test string with mixed ASCII and Unicode characters.";

    group.bench_function("encode_q", |b| {
        b.iter(|| {
            q_encoder.encode(black_box("UTF-8"), black_box(test_text))
        });
    });

    group.bench_function("encode_b", |b| {
        b.iter(|| {
            b_encoder.encode(black_box("UTF-8"), black_box(test_text))
        });
    });

    let encoded = q_encoder.encode("UTF-8", test_text);
    group.bench_function("decode", |b| {
        b.iter(|| {
            decoder.decode(black_box(&encoded))
        });
    });

    group.finish();
}

// Benchmark MIME type lookups
fn bench_mime_type_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("mime_type_lookup");

    group.bench_function("by_extension", |b| {
        b.iter(|| {
            type_by_extension(black_box(".html"))
        });
    });

    group.bench_function("extensions_by_type", |b| {
        b.iter(|| {
            extensions_by_type(black_box("text/html"))
        });
    });

    group.finish();
}

// Benchmark quoted-printable encoding/decoding
fn bench_quoted_printable(c: &mut Criterion) {
    let mut group = c.benchmark_group("quoted_printable");

    // Different data sizes
    for size in [1_024, 10_240, 102_400].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let data = "A".repeat(*size);

        group.bench_with_input(BenchmarkId::new("encode", size), &data, |b, data| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut output = Vec::new();
                    let mut writer = quotedprintable::Writer::new(&mut output);
                    writer.write_all(black_box(data.as_bytes())).await.unwrap();
                    writer.close().await.unwrap();
                    output
                })
            });
        });

        let data_bytes = data.as_bytes();
        group.bench_with_input(BenchmarkId::new("decode", size), &data_bytes, |b, data| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let cursor = Cursor::new(black_box(data));
                    let mut reader = quotedprintable::Reader::new(cursor);
                    let mut output = Vec::new();
                    reader.read_to_end(&mut output).await.unwrap();
                    output
                })
            });
        });
    }

    group.finish();
}

// Benchmark multipart operations
fn bench_multipart(c: &mut Criterion) {
    let mut group = c.benchmark_group("multipart");

    // Benchmark writing multipart data
    for num_parts in [1, 5, 10].iter() {
        group.bench_with_input(BenchmarkId::new("write", num_parts), num_parts, |b, &num_parts| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut buffer = Vec::new();
                    let mut writer = multipart::Writer::new(&mut buffer);

                    for i in 0..num_parts {
                        writer.write_field(
                            &format!("field{}", i),
                            "test data content here"
                        ).await.unwrap();
                    }

                    writer.close().await.unwrap();
                    buffer
                })
            });
        });
    }

    // Benchmark reading multipart data
    let test_data = {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut buffer = Vec::new();
            let mut writer = multipart::Writer::new(&mut buffer);
            writer.set_boundary("test-boundary".to_string()).unwrap();

            for i in 0..5 {
                writer.write_field(&format!("field{}", i), "test data").await.unwrap();
            }

            writer.close().await.unwrap();
            buffer
        })
    };

    group.bench_function("read", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let cursor = Cursor::new(black_box(&test_data));
                let mut reader = multipart::Reader::new(cursor, "test-boundary");

                let mut parts = Vec::new();
                while let Some(part) = reader.next_part().await.unwrap() {
                    parts.push(part);
                }
                parts.len()
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_media_type,
    bench_format_media_type,
    bench_encoded_word,
    bench_mime_type_lookup,
    bench_quoted_printable,
    bench_multipart
);

criterion_main!(benches);
