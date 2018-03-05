use internal::header::HeaderTable;
use internal::lead::Lead;
use std::io::{self, Read};

// ========================================================================= //

/// An RPM package file.
#[allow(dead_code)]
pub struct Package<R: Read> {
    reader: R,
    lead: Lead,
    signature: HeaderTable,
    header: HeaderTable,
}

impl<R: Read> Package<R> {
    /// Reads in an existing RPM package file.
    pub fn read(mut reader: R) -> io::Result<Package<R>> {
        let lead = Lead::read(reader.by_ref())?;
        let signature = HeaderTable::read(reader.by_ref())?;
        let header = HeaderTable::read(reader.by_ref())?;
        Ok(Package {
               reader,
               lead,
               signature,
               header,
           })
    }

    /// Returns the lead section.
    pub fn lead(&self) -> &Lead { &self.lead }

    /// Returns the table for the signature section.
    pub fn signature(&self) -> &HeaderTable { &self.signature }

    /// Returns the table for the header section.
    pub fn header(&self) -> &HeaderTable { &self.header }
}

// ========================================================================= //
