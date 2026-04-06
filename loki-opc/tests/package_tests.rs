use loki_opc::Package;
use loki_opc::PartName;
use loki_opc::PartData;
use std::io::Read;
#[test]
fn test_package_round_trip() {
    let mut pkg = Package::new();
    pkg.set_part(PartName::new("/word/document.xml").unwrap(), PartData::xml(b"<doc />".to_vec()));
    
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
    assert!(pkg_read.part(&PartName::new("/word/document.xml").unwrap()).is_some());
}
