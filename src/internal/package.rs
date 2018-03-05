use internal::index::IndexTable;
use internal::lead::LeadSection;
use internal::signature::SignatureSection;
use std::io::{self, Read};

// ========================================================================= //

/// An RPM package file.
#[allow(dead_code)]
pub struct Package<R: Read> {
    reader: R,
    lead: LeadSection,
    signature: SignatureSection,
    header: IndexTable,
}

impl<R: Read> Package<R> {
    /// Reads in an existing RPM package file.
    pub fn read(mut reader: R) -> io::Result<Package<R>> {
        let lead = LeadSection::read(reader.by_ref())?;
        let signature = SignatureSection::read(reader.by_ref())?;
        let header = IndexTable::read(reader.by_ref())?;
        Ok(Package {
               reader,
               lead,
               signature,
               header,
           })
    }

    /// Returns the lead section.
    pub fn lead(&self) -> &LeadSection { &self.lead }

    /// Returns the table for the signature section.
    pub fn signature(&self) -> &SignatureSection { &self.signature }

    /// Returns the table for the header section.
    pub fn header(&self) -> &IndexTable { &self.header }
}

// ========================================================================= //
