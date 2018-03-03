//! A library for encoding/decoding [RPM
//! packages](https://en.wikipedia.org/wiki/Rpm_(software)).

#![warn(missing_docs)]

extern crate byteorder;
extern crate cpio;

// ========================================================================= //

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

// ========================================================================= //
