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
        println!("Name: {}", String::from_utf8_lossy(package.lead().name()));
        println!("Type: {:?}", package.lead().package_type());
        println!("OS num: {}", package.lead().osnum());
        println!("");
        println!("SIGNATURE");
        for (tag, value) in package.signature().map().iter() {
            println!("{} = {:?}", tag, value);
        }
        println!("");
        println!("HEADER");
        for (tag, value) in package.header().map().iter() {
            println!("{} = {:?}", tag, value);
        }
    }
}

// ========================================================================= //
