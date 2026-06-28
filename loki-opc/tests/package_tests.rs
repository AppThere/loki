// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

use loki_opc::Package;
use loki_opc::PartData;
use loki_opc::PartName;
use std::io::Read;
#[test]
fn test_package_round_trip() {
    let mut pkg = Package::new();
    pkg.set_part(
        PartName::new("/word/document.xml").unwrap(),
        PartData::xml(b"<doc />".to_vec()),
    );

    let cp = pkg.core_properties_mut();
    cp.title = Some("Test Title".to_string());

    let mut buffer = std::io::Cursor::new(Vec::new());
    pkg.write(&mut buffer).unwrap();

    buffer.set_position(0);
    let mut dbg_bytes = Vec::new();
    buffer.read_to_end(&mut dbg_bytes).unwrap();
    println!("DEBUG RAW ZIP ZIP: {:?}", dbg_bytes.len());

    buffer.set_position(0);
    let pkg_read = Package::open(&mut buffer).unwrap();

    println!("PROPS: {:#?}", pkg_read.core_properties());

    #[cfg(feature = "serde")]
    {
        let title = pkg_read.core_properties().unwrap().title.as_ref().unwrap();
        assert_eq!(title, "Test Title");
    }
    assert!(
        pkg_read
            .part(&PartName::new("/word/document.xml").unwrap())
            .is_some()
    );
}

#[test]
fn test_package_utf16_transcode() {
    use std::io::Cursor;
    use zip::write::FileOptions;
    use zip::write::ZipWriter;

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    let opts = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Stored);

    // 1. [Content_Types].xml (UTF-16 LE)
    let ct_str = r#"<?xml version="1.0" encoding="UTF-16"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/></Types>"#;
    let ct_utf16 = {
        let u16s: Vec<u16> = ct_str.encode_utf16().collect();
        let mut bytes = vec![0xFF, 0xFE]; // LE BOM
        for val in u16s {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        bytes
    };
    zip.start_file("[Content_Types].xml", opts).unwrap();
    std::io::Write::write_all(&mut zip, &ct_utf16).unwrap();

    // 2. /word/document.xml (UTF-16 BE)
    let doc_str = r#"<?xml version="1.0" encoding="UTF-16"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello UTF-16</w:t></w:r></w:p></w:body></w:document>"#;
    let doc_utf16 = {
        let u16s: Vec<u16> = doc_str.encode_utf16().collect();
        let mut bytes = vec![0xFE, 0xFF]; // BE BOM
        for val in u16s {
            bytes.extend_from_slice(&val.to_be_bytes());
        }
        bytes
    };
    zip.start_file("word/document.xml", opts).unwrap();
    std::io::Write::write_all(&mut zip, &doc_utf16).unwrap();

    zip.finish().unwrap();

    let mut cursor = Cursor::new(buf);
    let pkg = Package::open(&mut cursor).unwrap();

    let part_data = pkg
        .part(&PartName::new("/word/document.xml").unwrap())
        .unwrap();
    let utf8_str = String::from_utf8(part_data.bytes.clone()).unwrap();
    assert_eq!(utf8_str, doc_str);
}
