use bzip2::read::BzDecoder;
use cpio::NewcReader;
use flate2::read::GzDecoder;
use internal::header::HeaderSection;
use internal::lead::LeadSection;
use internal::signature::SignatureSection;
use std::io::{self, Read, Seek, SeekFrom};
use xz2::read::XzDecoder;

// ========================================================================= //

/// An RPM package file.
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
    // TODO: Add a method to validate package signature.

    /// Reads files from the Archive section.
    pub fn read_archive(&mut self) -> io::Result<ArchiveSection<R>> {
        ArchiveSection::new(self.header.payload_compressor(), &mut self.reader)
    }
}

// ========================================================================= //

/// The "Archive" section of an RPM package file.
pub struct ArchiveSection<'p, R: 'p + Read + Seek> {
    decoder: ArchiveDecoder<'p, R>,
    start_offset: u64,
    done: bool,
}

impl<'p, R: 'p + Read + Seek> ArchiveSection<'p, R> {
    fn new(compressor: &str, reader: &'p mut R)
           -> io::Result<ArchiveSection<'p, R>> {
        let start_offset = reader.seek(SeekFrom::Current(0))?;
        let decoder = match compressor {
            "bzip2" => ArchiveDecoder::Bzip2(BzDecoder::new(reader)),
            "gzip" => ArchiveDecoder::Gzip(GzDecoder::new(reader)),
            "xz" => ArchiveDecoder::Xz(XzDecoder::new(reader)),
            _ => {
                invalid_data!("Unsupported payload compressor ({:?})",
                              compressor);
            }
        };
        Ok(ArchiveSection {
               decoder,
               start_offset,
               done: false,
           })
    }
}

impl<'a, 'p: 'a, R: 'p + Read + Seek> ArchiveSection<'p, R> {
    /// Returns a reader for the next file in the archive, if any.
    pub fn next_file(&'a mut self)
                     -> io::Result<Option<FileReader<'a, 'p, R>>> {
        if self.done {
            return Ok(None);
        }
        let reader = NewcReader::new(&mut self.decoder)?;
        if reader.entry().is_trailer() {
            self.done = true;
            return Ok(None);
        }
        Ok(Some(FileReader { reader: Some(reader) }))
    }
}

impl<'p, R: 'p + Read + Seek> Drop for ArchiveSection<'p, R> {
    fn drop(&mut self) {
        let _ = self.decoder.inner().seek(SeekFrom::Start(self.start_offset));
    }
}

// ========================================================================= //

enum ArchiveDecoder<'p, R: 'p + Read> {
    Bzip2(BzDecoder<&'p mut R>),
    Gzip(GzDecoder<&'p mut R>),
    Xz(XzDecoder<&'p mut R>),
}

impl<'p, R: Read> ArchiveDecoder<'p, R> {
    fn inner(&mut self) -> &mut R {
        match *self {
            ArchiveDecoder::Bzip2(ref mut decoder) => decoder.get_mut(),
            ArchiveDecoder::Gzip(ref mut decoder) => decoder.get_mut(),
            ArchiveDecoder::Xz(ref mut decoder) => decoder.get_mut(),
        }
    }
}

impl<'p, R: Read> Read for ArchiveDecoder<'p, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            ArchiveDecoder::Bzip2(ref mut decoder) => decoder.read(buf),
            ArchiveDecoder::Gzip(ref mut decoder) => decoder.read(buf),
            ArchiveDecoder::Xz(ref mut decoder) => decoder.read(buf),
        }
    }
}

// ========================================================================= //

/// Reads data for one file in a package.
pub struct FileReader<'a, 'p: 'a, R: 'p + Read> {
    reader: Option<NewcReader<&'a mut ArchiveDecoder<'p, R>>>,
}

impl<'a, 'p, R: Read> FileReader<'a, 'p, R> {
    /// Returns the install path of the file.
    pub fn file_path(&self) -> &str {
        self.reader.as_ref().unwrap().entry().name()
    }

    /// Returns the size of the file, in bytes.
    pub fn file_size(&self) -> u64 {
        self.reader.as_ref().unwrap().entry().file_size() as u64
    }
}

impl<'a, 'p, R: Read> Read for FileReader<'a, 'p, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.as_mut().unwrap().read(buf)
    }
}

impl<'a, 'p, R: Read> Drop for FileReader<'a, 'p, R> {
    fn drop(&mut self) { let _ = self.reader.take().unwrap().finish(); }
}

// ========================================================================= //
