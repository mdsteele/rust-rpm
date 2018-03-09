extern crate chrono;
extern crate clap;
extern crate rpmpkg;

use chrono::NaiveDateTime;
use clap::{App, Arg, SubCommand};
use std::fs;
use std::io;
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
        .subcommand(SubCommand::with_name("extract")
                        .about("Extracts a file from the package's archive")
                        .arg(Arg::with_name("rpm")
                                 .required(true)
                                 .help("Path to RPM package file"))
                        .arg(Arg::with_name("name")
                                 .required(true)
                                 .help("The name of the file to extract")))
        .subcommand(SubCommand::with_name("info")
                        .about("Prints basic information about a package")
                        .arg(Arg::with_name("rpm")
                                 .required(true)
                                 .help("Path to RPM package file")))
        .subcommand(SubCommand::with_name("list")
                        .about("Lists the files in the package")
                        .arg(Arg::with_name("long")
                                 .short("l")
                                 .long("long")
                                 .help("Lists in long format"))
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
            println!("{}    {}", datetime.format("%Y-%m-%d"), entry.author());
            println!("{}", entry.description());
            println!();
        }
    } else if let Some(submatches) = matches.subcommand_matches("extract") {
        let path = submatches.value_of("rpm").unwrap();
        let file = fs::File::open(path).unwrap();
        let mut package = rpmpkg::Package::read(file).unwrap();
        let filename = submatches.value_of("name").unwrap();
        let mut archive = package.read_archive().unwrap();
        while let Some(mut reader) = archive.next_file().unwrap() {
            if reader.file_path() == filename {
                io::copy(&mut reader, &mut io::stdout()).unwrap();
                break;
            }
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
        if let Some(vendor) = package.header().vendor_name() {
            println!("Vendor: {}", vendor);
        }
        println!("License: {}", package.header().license_name());
        if let Some(time) = package.header().build_time() {
            println!("Built at: {}",
                     timestamp_datetime(time).format("%Y-%m-%d %H:%M:%S"));
        }
        println!("HEADER TABLE");
        for (tag, value) in package.header().table().map().iter() {
            println!("{} = {:?}", tag, value);
        }
    } else if let Some(submatches) = matches.subcommand_matches("list") {
        let long = submatches.is_present("long");
        let path = submatches.value_of("rpm").unwrap();
        let file = fs::File::open(path).unwrap();
        let package = rpmpkg::Package::read(file).unwrap();
        for file in package.header().files() {
            if !long {
                println!("{}", file.name());
                continue;
            }
            let mode = {
                let bits = file.mode();
                let mut string = String::new();
                string.push(if file.symlink_target().is_some() {
                                'l'
                            } else {
                                '-'
                            });
                string.push(if bits & 0o400 == 0 { '-' } else { 'r' });
                string.push(if bits & 0o200 == 0 { '-' } else { 'w' });
                string.push(if bits & 0o100 == 0 { '-' } else { 'x' });
                string.push(if bits & 0o040 == 0 { '-' } else { 'r' });
                string.push(if bits & 0o020 == 0 { '-' } else { 'w' });
                string.push(if bits & 0o010 == 0 { '-' } else { 'x' });
                string.push(if bits & 0o004 == 0 { '-' } else { 'r' });
                string.push(if bits & 0o002 == 0 { '-' } else { 'w' });
                string.push(if bits & 0o001 == 0 { '-' } else { 'x' });
                string
            };
            let size = if file.size() >= 100_000_000 {
                format!("{}M", file.size() / (1 << 20))
            } else if file.size() >= 1_000_000 {
                format!("{}K", file.size() / (1 << 10))
            } else {
                format!("{}B", file.size())
            };
            let mtime = timestamp_datetime(file.modified_time())
                .format("%Y-%m-%d %H:%M");
            let mut line = format!("{} {} {} {:>6} {} {}",
                                   mode,
                                   file.user_name(),
                                   file.group_name(),
                                   size,
                                   mtime,
                                   file.name());
            if let Some(target) = file.symlink_target() {
                line = format!("{} -> {}", line, target);
            }
            println!("{}", line);
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
