//! A library for encoding/decoding [RPM
//! packages](https://en.wikipedia.org/wiki/Rpm_(software)).

#![warn(missing_docs)]

extern crate byteorder;
extern crate cpio;

mod internal;

pub use internal::header::{HeaderTable, HeaderValue};
pub use internal::lead::{Lead, PackageType};
pub use internal::package::Package;

// ========================================================================= //
