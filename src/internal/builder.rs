use bzip2::Compression as BzCompression;
use bzip2::write::BzEncoder;
use cpio;
use flate2::Compression as GzCompression;
use flate2::write::GzEncoder;
use internal::convert;
use internal::header::{FileInfo, HeaderSection};
use internal::lead::{LeadSection, PackageType};
use internal::signature::SignatureSection;
use md5;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::time::SystemTime;
use std::u32;
use xz2::write::XzEncoder;

// ========================================================================= //

/// A structure for building a new RPM package.
pub struct PackageBuilder {
    package_type: PackageType,
    header: HeaderSection,
}

impl PackageBuilder {
    /// Creates a builder for a package of the given type.
    pub fn new(package_type: PackageType) -> PackageBuilder {
        PackageBuilder {
            package_type,
            header: HeaderSection::new(),
        }
    }

    /// Sets the name of this package.
    pub fn set_package_name<S: Into<String>>(&mut self, name: S) {
        self.header.set_package_name(name.into());
    }

    /// Sets the version number string of this package.
    pub fn set_version_string<S: Into<String>>(&mut self, version: S) {
        self.header.set_version_string(version.into());
    }

    /// Sets the release string of this package.
    pub fn set_release_string<S: Into<String>>(&mut self, release: S) {
        self.header.set_release_string(release.into());
    }

    /// Sets the one-line description of this package.
    pub fn set_summary<S: Into<String>>(&mut self, summary: S) {
        self.header.set_summary(summary.into());
    }

    /// Sets the longer, multi-line description of this package.
    pub fn set_description<S: Into<String>>(&mut self, description: S) {
        self.header.set_description(description.into());
    }

    /// Sets the name of the author of the package.
    pub fn set_vendor_name<S: Into<String>>(&mut self, vendor: S) {
        self.header.set_vendor_name(vendor.into());
    }

    /// Sets the name of the license which applies to this package.
    pub fn set_license_name<S: Into<String>>(&mut self, license: S) {
        self.header.set_license_name(license.into());
    }

    /// Sets the URL for a page with more information about the package.
    pub fn set_homepage_url<S: Into<String>>(&mut self, url: S) {
        self.header.set_homepage_url(url.into());
    }

    /// Sets the architecture that the package is for (e.g. `"i386"`).
    pub fn set_architecture<S: Into<String>>(&mut self, arch: S) {
        self.header.set_architecture(arch.into());
    }

    /// Sets the compressor and compression level used to compress the Archive
    /// section of the package.  Currently supported values for `compressor`
    /// are `"gzip"`, `"bzip2"`, and `"xz"`.  The `level` value should be
    /// between 1 (fastest) and 9 (best) inclusive.
    pub fn set_payload_compression(&mut self, compression: &str, level: u32) {
        self.header.set_payload_compressor(compression.to_string());
        self.header.set_payload_compression_level(format!("{}", level));
    }

    /// Adds metadata about a file that will be installed by the package.  The
    /// data contents of the file will be supplied later to the
    /// `ArchiveBuilder`.
    pub fn add_file(&mut self, file_info: FileInfo) {
        self.header.add_file(file_info);
    }

    /// Sets the timestamp when the package was built.
    pub fn set_build_time(&mut self, timestamp: SystemTime) {
        self.header.set_build_time(timestamp);
    }

    /// Sets the timestamp when the package was built to now.
    pub fn set_build_time_to_now(&mut self) {
        self.set_build_time(SystemTime::now());
    }

    /// Locks in the package metadata and returns an `ArchiveBuilder` object
    /// for writing archive files into the package.
    pub fn build<W: Read + Write + Seek>(self, mut writer: W)
                                         -> io::Result<ArchiveBuilder<W>> {
        let full_name = format!("{}-{}-{}",
                                self.header.package_name(),
                                self.header.version_string(),
                                self.header.release_string());
        let lead = LeadSection::new(self.package_type,
                                    full_name.as_bytes().to_vec());
        lead.write(&mut writer)?;
        let signature_start = writer.seek(SeekFrom::Current(0))?;
        let signature = SignatureSection::placeholder();
        signature.write(&mut writer)?;
        let header_start = writer.seek(SeekFrom::Current(0))?;
        self.header.write(&mut writer)?;
        let file_infos = self.header.files().collect();
        let compressor = self.header.payload_compressor();
        let encoder = match compressor {
            "bzip2" => {
                let level = self.header.payload_compression_level();
                let level = match level.parse::<u32>() {
                    Ok(level) if level >= 1 && level <= 9 => {
                        // TODO: use specified bzip2 compression level
                        BzCompression::Default
                    }
                    _ => {
                        invalid_input!("Invalid bzip2 compression level \
                                        ({:?})",
                                       level);
                    }
                };
                ArchiveEncoder::Bzip2(BzEncoder::new(writer, level))
            }
            "gzip" => {
                let level = self.header.payload_compression_level();
                let level = match level.parse::<u32>() {
                    Ok(level) if level >= 1 && level <= 9 => {
                        GzCompression::new(level)
                    }
                    _ => {
                        invalid_input!("Invalid gzip compression level ({:?})",
                                       level);
                    }
                };
                ArchiveEncoder::Gzip(GzEncoder::new(writer, level), 0)
            }
            "xz" => {
                let level = self.header.payload_compression_level();
                let level = match level.parse::<u32>() {
                    Ok(level) if level >= 1 && level <= 9 => level,
                    _ => {
                        invalid_input!("Invalid xz compression level ({:?})",
                                       level);
                    }
                };
                ArchiveEncoder::Xz(XzEncoder::new(writer, level))
            }
            _ => {
                invalid_input!("Unsupported payload compressor ({:?})",
                               compressor);
            }
        };
        let archive = ArchiveBuilder {
            encoder: Some(encoder),
            signature_start,
            signature,
            header_start,
            file_infos,
            next_file_index: 0,
        };
        Ok(archive)
    }
}

// ========================================================================= //

/// A structure for writing archive file data into a new RPM package.
pub struct ArchiveBuilder<W: Read + Write + Seek> {
    encoder: Option<ArchiveEncoder<W>>,
    signature_start: u64,
    signature: SignatureSection,
    header_start: u64,
    file_infos: Vec<FileInfo>,
    next_file_index: usize,
}

impl<W: Read + Write + Seek> ArchiveBuilder<W> {
    /// Returns a `FileWriter` for the next file within the package archive
    /// that needs data to be written, or `None` if all files are now complete.
    pub fn next_file(&mut self) -> io::Result<Option<FileWriter<W>>> {
        if self.next_file_index >= self.file_infos.len() {
            return Ok(None);
        }
        let file_info = &self.file_infos[self.next_file_index];
        let cpio_writer =
            cpio::newc::Builder::new(file_info.name())
                .ino(file_info.inode())
                .mode(file_info.mode().into())
                .mtime(convert::system_time_to_u32(file_info.modified_time()))
                .write(self.encoder.as_mut().unwrap(), file_info.size());
        let file_writer = FileWriter {
            writer: Some(cpio_writer),
            file_info,
        };
        self.next_file_index += 1;
        Ok(Some(file_writer))
    }

    /// Finishes writing the package, and returns the underlying writer.
    pub fn finish(mut self) -> io::Result<W> { self.do_finish() }

    fn do_finish(&mut self) -> io::Result<W> {
        let mut encoder = self.encoder.take().unwrap();
        cpio::newc::trailer(&mut encoder)?;
        encoder.flush()?;
        let uncompressed_bytes = encoder.total_in();
        let mut writer = encoder.finish()?;
        let total_file_size = writer.seek(SeekFrom::Current(0))?;
        // TODO: Fill in MD5 digests for individual files in the Header section
        // TODO: Set header SHA1 in signature section
        let header_and_archive_size = total_file_size - self.header_start;
        let header_and_archive_md5 = {
            writer.seek(SeekFrom::Start(self.header_start))?;
            let mut context = md5::Context::new();
            io::copy(&mut io::Read::by_ref(&mut writer)
                         .take(header_and_archive_size),
                     &mut context)?;
            let md5::Digest(digest) = context.compute();
            digest
        };
        self.signature.set_uncompressed_archive_size(uncompressed_bytes);
        self.signature.set_header_and_archive_size(header_and_archive_size);
        self.signature.set_header_and_archive_md5(&header_and_archive_md5);
        writer.seek(SeekFrom::Start(self.signature_start))?;
        self.signature.write(&mut writer)?;
        writer.seek(SeekFrom::Start(total_file_size))?;
        Ok(writer)
    }
}

impl<W: Read + Write + Seek> Drop for ArchiveBuilder<W> {
    fn drop(&mut self) {
        if self.encoder.is_some() {
            let _ = self.do_finish();
        }
    }
}

// ========================================================================= //

enum ArchiveEncoder<W: Write> {
    Bzip2(BzEncoder<W>),
    Gzip(GzEncoder<W>, u64),
    Xz(XzEncoder<W>),
}

impl<W: Write> ArchiveEncoder<W> {
    fn total_in(&self) -> u64 {
        match *self {
            ArchiveEncoder::Bzip2(ref encoder) => encoder.total_in(),
            ArchiveEncoder::Gzip(_, total_in) => total_in,
            ArchiveEncoder::Xz(ref encoder) => encoder.total_in(),
        }
    }

    fn finish(self) -> io::Result<W> {
        match self {
            ArchiveEncoder::Bzip2(encoder) => encoder.finish(),
            ArchiveEncoder::Gzip(encoder, _) => encoder.finish(),
            ArchiveEncoder::Xz(encoder) => encoder.finish(),
        }
    }
}

impl<W: Write> Write for ArchiveEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            ArchiveEncoder::Bzip2(ref mut encoder) => encoder.write(buf),
            ArchiveEncoder::Gzip(ref mut encoder, ref mut total_in) => {
                let bytes_written = encoder.write(buf)?;
                *total_in += bytes_written as u64;
                Ok(bytes_written)
            }
            ArchiveEncoder::Xz(ref mut encoder) => encoder.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            ArchiveEncoder::Bzip2(ref mut encoder) => encoder.flush(),
            ArchiveEncoder::Gzip(ref mut encoder, _) => encoder.flush(),
            ArchiveEncoder::Xz(ref mut encoder) => encoder.flush(),
        }
    }
}

// ========================================================================= //

/// Allows writing data for a single archive file into a new RPM package.
pub struct FileWriter<'a, W: 'a + Write + Seek> {
    writer: Option<cpio::newc::Writer<&'a mut ArchiveEncoder<W>>>,
    file_info: &'a FileInfo,
}

impl<'a, W: Write + Seek> FileWriter<'a, W> {
    /// Returns the install path of the file being written.
    pub fn file_path(&self) -> &str { self.file_info.name() }
}

impl<'a, W: Write + Seek> Write for FileWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.as_mut().unwrap().flush()
    }
}

impl<'a, W: Write + Seek> Drop for FileWriter<'a, W> {
    fn drop(&mut self) { let _ = self.writer.take().unwrap().finish(); }
}

// ========================================================================= //
