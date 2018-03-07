use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use internal::header::HeaderSection;
use internal::lead::LeadSection;
use internal::signature::SignatureSection;
use std::io::{self, Read, Seek, SeekFrom, Write};

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
}

impl<R: Read + Seek> Package<R> {
    /// Extracts the CPIO archive from the package.
    pub fn decompress_archive<W: Write>(&mut self, mut writer: W)
                                        -> io::Result<()> {
        let position = self.reader.seek(SeekFrom::Current(0))?;
        let compressor = self.header.payload_compressor();
        match compressor {
            "bzip2" => {
                let mut decoder = BzDecoder::new(self.reader.by_ref());
                io::copy(&mut decoder, &mut writer)?;
            }
            "gzip" => {
                let mut decoder = GzDecoder::new(self.reader.by_ref());
                io::copy(&mut decoder, &mut writer)?;
            }
            // TODO: Support lzip/lzma/xz
            _ => {
                invalid_data!("Unsupported payload compressor ({:?})",
                              compressor);
            }
        }
        self.reader.seek(SeekFrom::Start(position))?;
        Ok(())
    }
}

// ========================================================================= //
