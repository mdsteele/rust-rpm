extern crate chrono;
extern crate clap;
extern crate rpmpkg;

use chrono::NaiveDateTime;
use clap::{App, Arg, SubCommand};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

// ========================================================================= //

fn main() {
    let matches = App::new("rpminfo")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Matthew D. Steele <mdsteele@alum.mit.edu>")
        .about("Inspects RPM package files")
        .subcommand(SubCommand::with_name("changelog")
                        .about("Outputs the package's changelog")
                        .arg(Arg::with_name("rpm")
                                 .required(true)
                                 .help("Path to RPM package file")))
        .subcommand(SubCommand::with_name("info")
                        .about("Prints basic information about a package")
                        .arg(Arg::with_name("rpm")
                                 .required(true)
                                 .help("Path to RPM package file")))
        .get_matches();
    if let Some(submatches) = matches.subcommand_matches("changelog") {
        let path = submatches.value_of("rpm").unwrap();
        let file = fs::File::open(path).unwrap();
        let package = rpmpkg::Package::read(file).unwrap();
        for entry in package.header().changelog() {
            let datetime = timestamp_datetime(entry.timestamp());
            println!("{}    {}",
                     datetime.date().format("%Y %b %d"),
                     entry.author());
            println!("{}", entry.description());
            println!();
        }
    } else if let Some(submatches) = matches.subcommand_matches("info") {
        let path = submatches.value_of("rpm").unwrap();
        let file = fs::File::open(path).unwrap();
        let package = rpmpkg::Package::read(file).unwrap();
        println!("Full name: {}",
                 String::from_utf8_lossy(package.lead().name()));
        println!("Type: {:?}", package.lead().package_type());
        println!();
        if let Some(checksum) = package.signature().header_sha1() {
            println!("Header SHA1 checksum: {}", checksum);
        }
        println!("SIGNATURE TABLE");
        for (tag, value) in package.signature().table().map().iter() {
            println!("{} = {:?}", tag, value);
        }
        println!();
        println!("Name: {}", package.header().package_name());
        println!("Version: {}", package.header().version_string());
        println!("Release: {}", package.header().release_string());
        println!("Files:");
        for file in package.header().files() {
            println!("  {} ({} bytes)", file.name(), file.size());
        }
        println!("HEADER TABLE");
        for (tag, value) in package.header().table().map().iter() {
            println!("{} = {:?}", tag, value);
        }
    }
}

// ========================================================================= //

fn timestamp_datetime(timestamp: SystemTime) -> NaiveDateTime {
    let seconds = if timestamp > UNIX_EPOCH {
        timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
    } else {
        -(UNIX_EPOCH.duration_since(timestamp).unwrap().as_secs() as i64)
    };
    NaiveDateTime::from_timestamp(seconds as i64, 0)
}

// ========================================================================= //
