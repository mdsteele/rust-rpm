//! A library for encoding/decoding [RPM
//! packages](https://en.wikipedia.org/wiki/Rpm_(software)).

#![warn(missing_docs)]

extern crate byteorder;
extern crate cpio;

mod internal;

pub use internal::index::{IndexTable, IndexValue};
pub use internal::lead::{LeadSection, PackageType};
pub use internal::package::Package;
pub use internal::signature::SignatureSection;

// ========================================================================= //
