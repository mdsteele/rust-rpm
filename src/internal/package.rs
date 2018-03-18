use bzip2::read::BzDecoder;
use cpio::NewcReader;
use flate2::read::GzDecoder;
use internal::convert::Sha1Writer;
use internal::header::{FileInfo, HeaderSection};
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

        // Check header SHA1, if present:
        if let Some(expected_header_sha1) = self.signature.header_sha1() {
            let actual_header_sha1 = {
                let header_size = self.archive_start - self.header_start;
                self.reader.seek(SeekFrom::Start(self.header_start))?;
                let mut context = Sha1Writer::new();
                io::copy(&mut self.reader.by_ref().take(header_size),
                         &mut context)?;
                context.digest()
            };
            if actual_header_sha1 != expected_header_sha1 {
                invalid_data!("Actual package header SHA1 digest ({}) does \
                               not match expected digest from package \
                               signature ({})",
                              actual_header_sha1,
                              expected_header_sha1);
            }
        }

        // TODO: check PGP/GPG signature, if present

        let opt_uncompressed_archive_size = self.signature
            .uncompressed_archive_size();

        // Check individual archive file sizes and MD5 checksums:
        let file_infos: Vec<FileInfo> = self.header.files().collect();
        let expected_total_install_size = self.header.total_install_size();
        let mut actual_total_install_size = 0;
        let mut file_index = 0;
        let mut archive = self.read_archive()?;
        while let Some(mut file) = archive.next_file()? {
            let file_info = &file_infos[file_index];
            if file.file_size() != file_info.size() {
                invalid_data!("Actual file size ({}) for {:?} does not match \
                               expected size from package metadata ({})",
                              file.file_size(),
                              file_info.name(),
                              file_info.size());
            }
            actual_total_install_size += file.file_size();
            if !file_info.md5_checksum().is_empty() {
                let actual_file_md5 = {
                    let mut context = md5::Context::new();
                    io::copy(&mut file, &mut context)?;
                    format!("{:x}", context.compute())
                };
                let expected_file_md5 =
                    file_info.md5_checksum().to_lowercase();
                if actual_file_md5 != expected_file_md5 {
                    invalid_data!("Actual file MD5 digest ({}) for {:?} does \
                                   not match expected digest from package \
                                   metadata ({})",
                                  actual_file_md5,
                                  file_info.name(),
                                  expected_file_md5);
                }
            }
            file_index += 1;
        }

        // Check total install size:
        if actual_total_install_size != expected_total_install_size {
            invalid_data!("Actual total install size ({}) does not match \
                           expected size from package header ({})",
                          actual_total_install_size,
                          expected_total_install_size);
        }

        // Check total archive uncompressed size, if present:
        if let Some(expected_uncompressed_archive_size) =
            opt_uncompressed_archive_size
        {
            let actual_uncompressed_archive_size = archive.decoder.total_out();
            if actual_uncompressed_archive_size !=
                expected_uncompressed_archive_size
            {
                invalid_data!("Actual uncompressed archive size ({}) does \
                               not match expected size from package signature \
                               ({})",
                              actual_uncompressed_archive_size,
                              expected_uncompressed_archive_size);
            }
        }

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
            "gzip" => ArchiveDecoder::Gzip(GzDecoder::new(reader), 0),
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
    Gzip(GzDecoder<&'p mut R>, u64),
    Xz(XzDecoder<&'p mut R>),
}

impl<'p, R: Read> ArchiveDecoder<'p, R> {
    fn total_out(&self) -> u64 {
        match *self {
            ArchiveDecoder::Bzip2(ref decoder) => decoder.total_out(),
            ArchiveDecoder::Gzip(_, total_out) => total_out,
            ArchiveDecoder::Xz(ref decoder) => decoder.total_out(),
        }
    }
}

impl<'p, R: Read> Read for ArchiveDecoder<'p, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            ArchiveDecoder::Bzip2(ref mut decoder) => decoder.read(buf),
            ArchiveDecoder::Gzip(ref mut decoder, ref mut total_out) => {
                let bytes_read = decoder.read(buf)?;
                *total_out += bytes_read as u64;
                Ok(bytes_read)
            }
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
    pub fn file_size(&self) -> u32 {
        self.reader.as_ref().unwrap().entry().file_size()
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
