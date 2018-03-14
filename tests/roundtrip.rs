extern crate rpmpkg;

use rpmpkg::{FileInfo, Package, PackageBuilder, PackageType};
use std::io::{Cursor, Read, Write};

// ========================================================================= //

#[test]
fn create_and_read_package() {
    let mut builder = PackageBuilder::new(PackageType::Binary);
    builder.set_package_name("hello");
    builder.set_version_string("0.1.2");
    builder.set_release_string("debug");
    builder.set_summary("A test package");
    builder.set_license_name("MIT");
    builder.set_payload_compression("bzip2", 6);
    builder.add_file(FileInfo::new("/usr/lib/hi.txt", 44));
    builder.add_file(FileInfo::new("/usr/lib/bye.txt", 45));
    let mut builder = builder.build(Cursor::new(Vec::new())).unwrap();
    while let Some(mut writer) = builder.next_file().unwrap() {
        let contents = format!("Hello, {:?}!\nNice to meet you.\n",
                               writer.file_path());
        writer.write_all(contents.as_bytes()).unwrap();
    }
    let package_file = Cursor::new(builder.finish().unwrap().into_inner());

    let mut package = Package::read(package_file).unwrap();
    package.validate().unwrap();
    assert_eq!(package.lead().package_type(), PackageType::Binary);
    assert_eq!(package.lead().name(), "hello-0.1.2-debug".as_bytes());
    assert_eq!(package.header().package_name(), "hello");
    assert_eq!(package.header().version_string(), "0.1.2");
    assert_eq!(package.header().release_string(), "debug");
    assert_eq!(package.header().summary(), "A test package");
    assert_eq!(package.header().license_name(), "MIT");
    assert_eq!(package.header().payload_compressor(), "bzip2");
    assert_eq!(package.header().payload_compression_level(), "6");
    let files: Vec<FileInfo> = package.header().files().collect();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].name(), "/usr/lib/hi.txt");
    assert_eq!(files[1].name(), "/usr/lib/bye.txt");
    let mut archive = package.read_archive().unwrap();
    {
        let mut file = archive.next_file().unwrap().unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents.as_str(),
                   "Hello, \"/usr/lib/hi.txt\"!\n\
                    Nice to meet you.\n");
    }
    {
        let mut file = archive.next_file().unwrap().unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents.as_str(),
                   "Hello, \"/usr/lib/bye.txt\"!\n\
                    Nice to meet you.\n");
    }
    assert!(archive.next_file().unwrap().is_none());
}

// TODO: Add tests for gzip and xz archives as well

// ========================================================================= //
