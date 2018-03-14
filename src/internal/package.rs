use bzip2::read::BzDecoder;
use cpio::NewcReader;
use flate2::read::GzDecoder;
use internal::header::HeaderSection;
use internal::lead::LeadSection;
use internal::signature::SignatureSection;
use md5;
use std::io::{self, Read, Seek, SeekFrom};
use xz2::read::XzDecoder;

// ========================================================================= //

/// An RPM package file.
pub struct Package<R: Read + Seek> {
    reader: R,
    lead: LeadSection,
    signature: SignatureSection,
    header_start: u64,
    header: HeaderSection,
    archive_start: u64,
}

impl<R: Read + Seek> Package<R> {
    /// Reads in an existing RPM package file.
    pub fn read(mut reader: R) -> io::Result<Package<R>> {
        let lead = LeadSection::read(reader.by_ref())?;
        let signature = SignatureSection::read(reader.by_ref())?;
        let header_start = reader.seek(SeekFrom::Current(0))?;
        let header = HeaderSection::read(reader.by_ref())?;
        let archive_start = reader.seek(SeekFrom::Current(0))?;
        let package = Package {
            reader,
            lead,
            signature,
            header_start,
            header,
            archive_start,
        };
        Ok(package)
    }

    /// Returns the lead section.
    pub fn lead(&self) -> &LeadSection { &self.lead }

    /// Returns the signature section.
    pub fn signature(&self) -> &SignatureSection { &self.signature }

    /// Returns the header section.
    pub fn header(&self) -> &HeaderSection { &self.header }

    /// Reads files from the Archive section.
    pub fn read_archive(&mut self) -> io::Result<ArchiveSection<R>> {
        self.reader.seek(SeekFrom::Start(self.archive_start))?;
        ArchiveSection::new(self.header.payload_compressor(), &mut self.reader)
    }

    /// Validates the package checksums and signature; returns an error if any
    /// of the validation checks fail.
    pub fn validate(&mut self) -> io::Result<()> {
        // Check header and archive size:
        let archive_end = self.reader.seek(SeekFrom::End(0))?;
        let actual_header_and_archive_size = archive_end - self.header_start;
        let expected_header_and_archive_size = self.signature
            .header_and_archive_size();
        if actual_header_and_archive_size != expected_header_and_archive_size {
            invalid_data!("Actual package header/archive size ({}) does not \
                           match expected size from package signature ({})",
                          actual_header_and_archive_size,
                          expected_header_and_archive_size);
        }

        // Check header and archive MD5:
        let actual_header_and_archive_md5 = {
            self.reader.seek(SeekFrom::Start(self.header_start))?;
            let mut context = md5::Context::new();
            io::copy(&mut self.reader, &mut context)?;
            context.compute()
        };
        let expected_header_and_archive_md5 =
            md5::Digest(*self.signature.header_and_archive_md5());
        if actual_header_and_archive_md5 != expected_header_and_archive_md5 {
            invalid_data!("Actual package header/archive MD5 digest ({:x}) \
                           does not match expected digest from package \
                           signature ({:x})",
                          actual_header_and_archive_md5,
                          expected_header_and_archive_md5);
        }

        // TODO: check uncompressed payload size, if present in signature

        // TODO: check header SHA1, if present in signature

        // TODO: check PGP/GPG signature, if present

        Ok(())
    }
}

// ========================================================================= //

/// The "Archive" section of an RPM package file.
pub struct ArchiveSection<'p, R: 'p + Read + Seek> {
    decoder: ArchiveDecoder<'p, R>,
    done: bool,
}

impl<'p, R: 'p + Read + Seek> ArchiveSection<'p, R> {
    fn new(compressor: &str, reader: &'p mut R)
           -> io::Result<ArchiveSection<'p, R>> {
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

// ========================================================================= //

enum ArchiveDecoder<'p, R: 'p + Read> {
    Bzip2(BzDecoder<&'p mut R>),
    Gzip(GzDecoder<&'p mut R>),
    Xz(XzDecoder<&'p mut R>),
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
