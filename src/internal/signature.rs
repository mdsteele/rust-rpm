use internal::index::{IndexTable, IndexType};
use std::io::{self, Read};

// ========================================================================= //

/// Required tag for the combined size of the Header and Payload sections.
const TAG_SIZE: i32 = 1000;
/// Optional tag for the uncompressed size of the Payload archive, including
/// the cpio headers.
const TAG_PAYLOAD_SIZE: i32 = 1007;

/// Optional tag for the SHA1 checksum of the Header section.
const TAG_SHA1: i32 = 269;
/// Required tag for the 128-bit MD5 checksum of the Header and Archive
/// sections.
const TAG_MD5: i32 = 1004;

// Known index entires for Signature section.  The bool indicates whether the
// entry is required (true) or optional (false).
#[cfg_attr(rustfmt, rustfmt_skip)]
const ENTRIES: &[(bool, &str, i32, IndexType, Option<usize>)] = &[
    (true,  "SIZE",         TAG_SIZE,         IndexType::Int32,  Some(1)),
    (false, "PAYLOAD_SIZE", TAG_PAYLOAD_SIZE, IndexType::Int32,  Some(1)),
    (false, "SHA1",         TAG_SHA1,         IndexType::String, None),
    (true,  "MD5",          TAG_MD5,          IndexType::Binary, Some(16)),
    // TODO: Add tags for DSA/RSA/PGP/GPG
];

// ========================================================================= //

/// The "Signature" section of an RPM package file.
pub struct SignatureSection {
    table: IndexTable,
}

impl SignatureSection {
    pub(crate) fn read<R: Read>(reader: R) -> io::Result<SignatureSection> {
        let table = IndexTable::read(reader)?;
        for &(required, name, tag, itype, count) in ENTRIES.iter() {
            table.validate("Signature", required, name, tag, itype, count)?;
        }
        Ok(SignatureSection { table: table })
    }

    /// Returns the raw underlying index table.
    pub fn table(&self) -> &IndexTable { &self.table }

    /// Returns the expected SHA1 checksum of the package's Header section, if
    /// any.
    pub fn header_sha1(&self) -> Option<&str> {
        self.table.get_string(TAG_SHA1)
    }

    /// Returns the expected MD5 checksum of the package's Header and Archive
    /// sections.
    pub fn header_and_payload_md5(&self) -> &[u8] {
        self.table.get_binary(TAG_MD5).unwrap()
    }
}

// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::ENTRIES;
    use std::collections::HashSet;

    #[test]
    fn tags_are_unique() {
        let mut tags = HashSet::new();
        for &(_, _, tag, _, _) in ENTRIES.iter() {
            assert!(!tags.contains(&tag));
            tags.insert(tag);
        }
    }
}

// ========================================================================= //
