//! Benchmarks for unhwp parsing performance.
//!
//! Run with: cargo bench
//!
//! These benchmarks test parsing performance at various document sizes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::io::Cursor;

/// Creates a synthetic HWPX document with the given number of paragraphs.
fn create_test_hwpx(paragraph_count: usize) -> Vec<u8> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let mut buffer = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buffer));

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // mimetype
    zip.start_file("mimetype", options).unwrap();
    zip.write_all(b"application/hwp+zip").unwrap();

    // META-INF/container.xml
    zip.start_file("META-INF/container.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
  <rootfiles>
    <rootfile full-path="Contents/content.hpf"/>
  </rootfiles>
</container>"#).unwrap();

    // Contents/content.hpf
    zip.start_file("Contents/content.hpf", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<opf:package xmlns:opf="http://www.idpf.org/2007/opf" version="2.0">
  <opf:metadata>
    <dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Benchmark Document</dc:title>
    <dc:creator xmlns:dc="http://purl.org/dc/elements/1.1/">Test</dc:creator>
  </opf:metadata>
  <opf:manifest>
    <opf:item id="section0" href="section0.xml" media-type="application/xml"/>
  </opf:manifest>
</opf:package>"#).unwrap();

    // Contents/header.xml
    zip.start_file("Contents/header.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head">
</hh:head>"#).unwrap();

    // Generate section content
    let mut section_content = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<hp:sec xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">"#);

    for i in 0..paragraph_count {
        section_content.push_str(&format!(
            r#"<hp:p paraPrIDRef="0" styleIDRef="0">
  <hp:run charPrIDRef="0">
    <hp:t>This is paragraph {} with some test content for benchmarking purposes. 한글 테스트 내용도 포함합니다.</hp:t>
  </hp:run>
</hp:p>"#,
            i
        ));
    }

    section_content.push_str("</hp:sec>");

    zip.start_file("Contents/section0.xml", options).unwrap();
    zip.write_all(section_content.as_bytes()).unwrap();

    zip.finish().unwrap();
    drop(zip);
    buffer
}

/// Benchmark HWPX parsing at various sizes.
fn bench_hwpx_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hwpx_parsing");

    for para_count in [10, 100, 500, 1000].iter() {
        let data = create_test_hwpx(*para_count);
        let size = data.len() as u64;

        group.throughput(Throughput::Bytes(size));
        group.bench_with_input(
            BenchmarkId::new("paragraphs", para_count),
            &data,
            |b, data| {
                b.iter(|| {
                    let cursor = Cursor::new(black_box(data.as_slice()));
                    let _ = unhwp::parse_reader(cursor);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark document rendering to Markdown.
fn bench_markdown_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_rendering");

    for para_count in [10, 100, 500].iter() {
        let data = create_test_hwpx(*para_count);
        let cursor = Cursor::new(data.as_slice());
        let document = unhwp::parse_reader(cursor).unwrap();

        group.bench_with_input(
            BenchmarkId::new("paragraphs", para_count),
            &document,
            |b, doc| {
                b.iter(|| {
                    let options = unhwp::RenderOptions::default();
                    let _ = unhwp::render::render_markdown(black_box(doc), &options);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark text extraction.
fn bench_text_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_extraction");

    for para_count in [10, 100, 500, 1000].iter() {
        let data = create_test_hwpx(*para_count);
        let cursor = Cursor::new(data.as_slice());
        let document = unhwp::parse_reader(cursor).unwrap();

        group.bench_with_input(
            BenchmarkId::new("paragraphs", para_count),
            &document,
            |b, doc| {
                b.iter(|| {
                    let _ = black_box(doc).plain_text();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark format detection.
fn bench_format_detection(c: &mut Criterion) {
    let hwpx_data = create_test_hwpx(10);

    // OLE header for HWP5
    let hwp5_header: [u8; 16] = [
        0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    c.bench_function("detect_hwpx", |b| {
        b.iter(|| {
            unhwp::detect_format_from_bytes(black_box(&hwpx_data)).unwrap()
        });
    });

    c.bench_function("detect_hwp5", |b| {
        b.iter(|| {
            unhwp::detect_format_from_bytes(black_box(&hwp5_header)).unwrap()
        });
    });
}

criterion_group!(
    benches,
    bench_format_detection,
    bench_hwpx_parsing,
    bench_markdown_rendering,
    bench_text_extraction,
);
criterion_main!(benches);
