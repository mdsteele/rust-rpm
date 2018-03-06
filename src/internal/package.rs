use internal::header::HeaderSection;
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
    header: HeaderSection,
}

impl<R: Read> Package<R> {
    /// Reads in an existing RPM package file.
    pub fn read(mut reader: R) -> io::Result<Package<R>> {
        let lead = LeadSection::read(reader.by_ref())?;
        let signature = SignatureSection::read(reader.by_ref())?;
        let header = HeaderSection::read(reader.by_ref())?;
        Ok(Package {
               reader,
               lead,
               signature,
               header,
           })
    }

    /// Returns the lead section.
    pub fn lead(&self) -> &LeadSection { &self.lead }

    /// Returns the signature section.
    pub fn signature(&self) -> &SignatureSection { &self.signature }

    /// Returns the header section.
    pub fn header(&self) -> &HeaderSection { &self.header }

    // TODO: Support reading Archive section
}

// ========================================================================= //
