extern crate clap;
extern crate rpmpkg;

use clap::{App, Arg, SubCommand};
use std::fs;

// ========================================================================= //

fn main() {
    let matches = App::new("rpminfo")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Matthew D. Steele <mdsteele@alum.mit.edu>")
        .about("Inspects RPM package files")
        .subcommand(SubCommand::with_name("info")
                        .about("Prints basic information about a package")
                        .arg(Arg::with_name("rpm")
                                 .required(true)
                                 .help("Path to RPM package file")))
        .get_matches();
    if let Some(submatches) = matches.subcommand_matches("info") {
        let path = submatches.value_of("rpm").unwrap();
        let file = fs::File::open(path).unwrap();
        let package = rpmpkg::Package::read(file).unwrap();
        println!("Full name: {}",
                 String::from_utf8_lossy(package.lead().name()));
        println!("Type: {:?}", package.lead().package_type());
        println!("");
        if let Some(checksum) = package.signature().header_sha1() {
            println!("Header SHA1 checksum: {}", checksum);
        }
        println!("SIGNATURE TABLE");
        for (tag, value) in package.signature().table().map().iter() {
            println!("{} = {:?}", tag, value);
        }
        println!("");
        println!("Name: {}", package.header().package_name());
        println!("Version: {}", package.header().version_string());
        println!("Release: {}", package.header().release_string());
        println!("Files:");
        for file in package.header().files() {
            println!("  {} ({} bytes)", file.name(), file.size());
        }
    }
}

// ========================================================================= //
