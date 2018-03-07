//! A library for encoding/decoding [RPM
//! packages](https://en.wikipedia.org/wiki/Rpm_(software)).

#![warn(missing_docs)]

extern crate byteorder;
extern crate bzip2;
extern crate cpio;
extern crate flate2;

mod internal;

pub use internal::header::{FileInfo, FileInfoIter, HeaderSection};
pub use internal::index::{IndexTable, IndexValue};
pub use internal::lead::{LeadSection, PackageType};
pub use internal::package::Package;
pub use internal::signature::SignatureSection;

// ========================================================================= //
