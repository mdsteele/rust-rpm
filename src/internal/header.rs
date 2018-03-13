use internal::convert;
use internal::index::{IndexTable, IndexType, IndexValue};
use std::fs::Metadata;
use std::io::{self, Read, Seek, Write};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::time::SystemTime;

// ========================================================================= //

/// Required tag for the name of the package.
const TAG_NAME: i32 = 1000;
/// Required tag for the version number of the package.
const TAG_VERSION: i32 = 1001;
/// Required tag for the release number of the package.
const TAG_RELEASE: i32 = 1002;
/// Required tag for a one-line description of the package.
const TAG_SUMMARY: i32 = 1004;
/// Required tag for a longer, multi-line description of the package.
const TAG_DESCRIPTION: i32 = 1005;
/// Required tag for the sum of the sizes of the regular files in the archive.
const TAG_SIZE: i32 = 1009;
/// Optional tag for the author of the package.
const TAG_VENDOR: i32 = 1011;
/// Required tag for the license which applies to this package.
const TAG_LICENSE: i32 = 1014;
/// Required tag for the administrative group to which this package belongs.
const TAG_GROUP: i32 = 1016;
/// Optional tag for a URL with more information about the package.
const TAG_URL: i32 = 1020;
/// Required tag for the OS of the package.  The value must be `"linux"`.
const TAG_OS: i32 = 1021;
/// Required tag for the archetecture that the package is for.
const TAG_ARCH: i32 = 1022;
/// Optional tag for the uncompressed size of the Payload archive, including
/// the cpio headers.
const TAG_ARCHIVESIZE: i32 = 1046;
/// Required tag for the format of the Archive section.  The value must be
/// `"cpio"`.
const TAG_PAYLOADFORMAT: i32 = 1124;
/// Required tag for the compression used on the Archive section
/// (e.g. `"gzip"`).
const TAG_PAYLOADCOMPRESSOR: i32 = 1125;
/// Required tag for the compression level used for the Payload (e.g. `"9"`).
const TAG_PAYLOADFLAGS: i32 = 1126;

/// Optional tag for the preinstall script.
const TAG_PREIN: i32 = 1023;
/// Optional tag for the postinstall script.
const TAG_POSTIN: i32 = 1024;
/// Optional tag for the preuninstall script.
const TAG_PREUN: i32 = 1025;
/// Optional tag for the postuninstall script.
const TAG_POSTUN: i32 = 1026;
/// Optional tag for the preinstall script interpreter (e.g `"/bin/sh"`).
const TAG_PREINPROG: i32 = 1085;
/// Optional tag for the postinstall script interpreter (e.g `"/bin/sh"`).
const TAG_POSTINPROG: i32 = 1086;
/// Optional tag for the preuninstall script interpreter (e.g `"/bin/sh"`).
const TAG_PREUNPROG: i32 = 1087;
/// Optional tag for the postuninstall script interpreter (e.g `"/bin/sh"`).
const TAG_POSTUNPROG: i32 = 1088;

const TAG_OLDFILENAMES: i32 = 1027;
const TAG_FILESIZES: i32 = 1028;
const TAG_FILEMODES: i32 = 1030;
const TAG_FILERDEVS: i32 = 1033;
const TAG_FILEMTIMES: i32 = 1034;
const TAG_FILEMD5S: i32 = 1035;
const TAG_FILELINKTOS: i32 = 1036;
const TAG_FILEFLAGS: i32 = 1037;
const TAG_FILEUSERNAME: i32 = 1039;
const TAG_FILEGROUPNAME: i32 = 1040;
const TAG_FILEDEVICES: i32 = 1095;
const TAG_FILEINODES: i32 = 1096;
const TAG_FILELANGS: i32 = 1097;
const TAG_DIRINDEXES: i32 = 1116;
const TAG_BASENAMES: i32 = 1117;
const TAG_DIRNAMES: i32 = 1118;

/// Required tag for the names of the dependencies provided by this package.
const TAG_PROVIDENAME: i32 = 1047;
const TAG_PROVIDEFLAGS: i32 = 1112;
const TAG_PROVIDEVERSION: i32 = 1113;
/// Required tag for the names of the dependencies of this package.
const TAG_REQUIRENAME: i32 = 1049;
const TAG_REQUIREFLAGS: i32 = 1048;
const TAG_REQUIREVERSION: i32 = 1050;
/// Optional tag for the names of any dependencies that conflict with this
/// package.
const TAG_CONFLICTNAME: i32 = 1054;
const TAG_CONFLICTFLAGS: i32 = 1053;
const TAG_CONFLICTVERSION: i32 = 1055;
/// Optional tag for the names of any dependencies that obsolete with this
/// package.
const TAG_OBSOLETENAME: i32 = 1090;
const TAG_OBSOLETEFLAGS: i32 = 1114;
const TAG_OBSOLETEVERSION: i32 = 1115;

/// Optional tag for the timestamp (in seconds since the epoch) when the
/// package was built.
const TAG_BUILDTIME: i32 = 1006;
/// Optional tag for the hostname of the system on which which the package was
/// built.
const TAG_BUILDHOST: i32 = 1007;
/// Optional tag for the flags to control how files are to be verified after
/// install.
const TAG_FILEVERIFYFLAGS: i32 = 1045;
/// Optional tag for the timestamp for each changelog entry.
const TAG_CHANGELOGTIME: i32 = 1080;
/// Optional tag for the author name for each changelog entry.
const TAG_CHANGELOGNAME: i32 = 1081;
/// Optional tag for the description for each changelog entry.
const TAG_CHANGELOGTEXT: i32 = 1082;
/// Optional tag for the compiler flags used when building this package.
const TAG_OPTFLAGS: i32 = 1122;

// Known index entires for Header section.  The bool indicates whether the
// entry is required (true) or optional (false).
#[cfg_attr(rustfmt, rustfmt_skip)]
const ENTRIES: &[(bool, &str, i32, IndexType, Option<usize>)] = &[
    // Package information:
    (true,  "NAME",         TAG_NAME,         IndexType::String,     None),
    (true,  "VERSION",      TAG_VERSION,      IndexType::String,     None),
    (true,  "RELEASE",      TAG_RELEASE,      IndexType::String,     None),
    (true,  "SUMMARY",      TAG_SUMMARY,      IndexType::I18nString, None),
    (true,  "DESCRIPTION",  TAG_DESCRIPTION,  IndexType::I18nString, None),
    (true,  "SIZE",         TAG_SIZE,         IndexType::Int32,      Some(1)),
    (false, "VENDOR",       TAG_VENDOR,       IndexType::String,     None),
    (true,  "LICENSE",      TAG_LICENSE,      IndexType::String,     None),
    (true,  "GROUP",        TAG_GROUP,        IndexType::I18nString, None),
    (false, "URL",          TAG_URL,          IndexType::String,     None),
    (true,  "OS",           TAG_OS,           IndexType::String,     None),
    (true,  "ARCH",         TAG_ARCH,         IndexType::String,     None),
    (false, "ARCHIVESIZE",  TAG_ARCHIVESIZE,  IndexType::Int32,      Some(1)),
    (true,  "PAYLOADFORMAT", TAG_PAYLOADFORMAT, IndexType::String,   None),
    (true,  "PAYLOADCOMPRESSOR", TAG_PAYLOADCOMPRESSOR,
     IndexType::String, None),
    (true,  "PAYLOADFLAGS", TAG_PAYLOADFLAGS, IndexType::String,     None),
    // Installation information:
    (false, "PREIN",      TAG_PREIN,      IndexType::String, None),
    (false, "POSTIN",     TAG_POSTIN,     IndexType::String, None),
    (false, "PREUN",      TAG_PREUN,      IndexType::String, None),
    (false, "POSTUN",     TAG_POSTUN,     IndexType::String, None),
    (false, "PREINPROG",  TAG_PREINPROG,  IndexType::String, None),
    (false, "POSTINPROG", TAG_POSTINPROG, IndexType::String, None),
    (false, "PREUNPROG",  TAG_PREUNPROG,  IndexType::String, None),
    (false, "POSTUNPROG", TAG_POSTUNPROG, IndexType::String, None),
    // File information:
    (false, "OLDFILENAMES",  TAG_OLDFILENAMES,  IndexType::StringArray, None),
    (true,  "FILESIZES",     TAG_FILESIZES,     IndexType::Int32,       None),
    (true,  "FILEMODES",     TAG_FILEMODES,     IndexType::Int16,       None),
    (true,  "FILERDEVS",     TAG_FILERDEVS,     IndexType::Int16,       None),
    (true,  "FILEMTIMES",    TAG_FILEMTIMES,    IndexType::Int32,       None),
    (true,  "FILEMD5S",      TAG_FILEMD5S,      IndexType::StringArray, None),
    (true,  "FILELINKTOS",   TAG_FILELINKTOS,   IndexType::StringArray, None),
    (true,  "FILEFLAGS",     TAG_FILEFLAGS,     IndexType::Int32,       None),
    (true,  "FILEUSERNAME",  TAG_FILEUSERNAME,  IndexType::StringArray, None),
    (true,  "FILEGROUPNAME", TAG_FILEGROUPNAME, IndexType::StringArray, None),
    (true,  "FILEDEVICES",   TAG_FILEDEVICES,   IndexType::Int32,       None),
    (true,  "FILEINODES",    TAG_FILEINODES,    IndexType::Int32,       None),
    (true,  "FILELANGS",     TAG_FILELANGS,     IndexType::StringArray, None),
    (false, "DIRINDEXES",    TAG_DIRINDEXES,    IndexType::Int32,       None),
    (false, "BASENAMES",     TAG_BASENAMES,     IndexType::StringArray, None),
    (false, "DIRNAMES",      TAG_DIRNAMES,      IndexType::StringArray, None),
    // Dependency information:
    (true,  "PROVIDENAME",   TAG_PROVIDENAME,   IndexType::StringArray, None),
    (true,  "PROVIDEFLAGS",  TAG_PROVIDEFLAGS,  IndexType::Int32,       None),
    (true,  "PROVIDEVERSION",TAG_PROVIDEVERSION,IndexType::StringArray, None),
    (true,  "REQUIRENAME",   TAG_REQUIRENAME,   IndexType::StringArray, None),
    (true,  "REQUIREFLAGS",  TAG_REQUIREFLAGS,  IndexType::Int32,       None),
    (true,  "REQUIREVERSION",TAG_REQUIREVERSION,IndexType::StringArray, None),
    (false, "CONFLICTNAME",  TAG_CONFLICTNAME,  IndexType::StringArray, None),
    (false, "CONFLICTFLAGS", TAG_CONFLICTFLAGS, IndexType::Int32,       None),
    (false,"CONFLICTVERSION",TAG_CONFLICTVERSION,IndexType::StringArray,None),
    (false, "OBSOLETENAME",  TAG_OBSOLETENAME,  IndexType::StringArray, None),
    (false, "OBSOLETEFLAGS", TAG_OBSOLETEFLAGS, IndexType::Int32,       None),
    (false,"OBSOLETEVERSION",TAG_OBSOLETEVERSION,IndexType::StringArray,None),
    // Other information:
    (false, "BUILDTIME",     TAG_BUILDTIME,     IndexType::Int32,    Some(1)),
    (false, "BUILDHOST",     TAG_BUILDHOST,     IndexType::String,      None),
    (false, "FILEVERIFYFLAGS", TAG_FILEVERIFYFLAGS, IndexType::Int32,   None),
    (false, "CHANGELOGTIME", TAG_CHANGELOGTIME, IndexType::Int32,       None),
    (false, "CHANGELOGNAME", TAG_CHANGELOGNAME, IndexType::StringArray, None),
    (false, "CHANGELOGTEXT", TAG_CHANGELOGTEXT, IndexType::StringArray, None),
    (false, "OPTFLAGS",      TAG_OPTFLAGS,      IndexType::String,      None),
];

#[cfg_attr(rustfmt, rustfmt_skip)]
const INSTALLATION_ENTRIES: &[(&str, i32, &str, i32)] = &[
    ("PREIN",  TAG_PREIN,  "PREINPROG",  TAG_PREINPROG),
    ("POSTIN", TAG_POSTIN, "POSTINPROG", TAG_POSTINPROG),
    ("PREUN",  TAG_PREUN,  "PREUNPROG",  TAG_PREUNPROG),
    ("POSTUN", TAG_POSTUN, "POSTUNPROG", TAG_POSTUNPROG),
];

#[cfg_attr(rustfmt, rustfmt_skip)]
const FILE_ENTRIES: &[(&str, i32)] = &[
    ("FILESIZES",     TAG_FILESIZES),
    ("FILEMODES",     TAG_FILEMODES),
    ("FILERDEVS",     TAG_FILERDEVS),
    ("FILEMTIMES",    TAG_FILEMTIMES),
    ("FILEMD5S",      TAG_FILEMD5S),
    ("FILELINKTOS",   TAG_FILELINKTOS),
    ("FILEFLAGS",     TAG_FILEFLAGS),
    ("FILEUSERNAME",  TAG_FILEUSERNAME),
    ("FILEGROUPNAME", TAG_FILEGROUPNAME),
    ("FILEDEVICES",   TAG_FILEDEVICES),
    ("FILEINODES",    TAG_FILEINODES),
    ("FILELANGS",     TAG_FILELANGS),
];

// ========================================================================= //

/// The name of this section.
const SECTION: &str = "Header";

/// Can be listed under `TAG_REQUIRENAME` to indicate that we're not using
/// `TAG_OLDFILENAMES`.
const REQUIRE_COMPRESSED_FILE_NAMES: &str = "rpmlib(CompressedFileNames)";

/// The required value under `TAG_OS`.
const OS_STRING: &str = "linux";

/// The required value under `TAG_PAYLOADFORMAT`.
const PAYLOAD_FORMAT: &str = "cpio";

// ========================================================================= //

/// The "Header" section of an RPM package file.
pub struct HeaderSection {
    table: IndexTable,
    use_old_filenames: bool,
}

impl HeaderSection {
    pub(crate) fn new() -> HeaderSection {
        let mut table = IndexTable::new();
        table.set(TAG_SIZE, IndexValue::Int32(vec![0]));
        table.set(TAG_OS, IndexValue::String(OS_STRING.to_string()));
        table.set(TAG_PAYLOADFORMAT,
                  IndexValue::String(PAYLOAD_FORMAT.to_string()));
        table.set(TAG_PAYLOADCOMPRESSOR,
                  IndexValue::String("gzip".to_string()));
        table.set(TAG_PAYLOADFLAGS, IndexValue::String("9".to_string()));
        table.set(TAG_OLDFILENAMES, IndexValue::StringArray(Vec::new()));
        for &(required, _, tag, itype, _) in ENTRIES {
            if required && !table.has(tag) {
                table.set(tag, itype.default_value());
            }
        }
        HeaderSection {
            table,
            use_old_filenames: true,
        }
    }

    pub(crate) fn read<R: Read>(reader: R) -> io::Result<HeaderSection> {
        let table = IndexTable::read(reader, false)?;
        for &(required, name, tag, itype, count) in ENTRIES.iter() {
            table.validate(SECTION, required, name, tag, itype, count)?;
        }

        // Validate package information:
        table.expect_string_value(SECTION, "OS", TAG_OS, OS_STRING)?;
        table
            .expect_string_value(SECTION,
                                 "PAYLOADFORMAT",
                                 TAG_PAYLOADFORMAT,
                                 PAYLOAD_FORMAT)?;

        // Validate installation information:
        for &(name1, tag1, name2, tag2) in INSTALLATION_ENTRIES.iter() {
            if table.has(tag1) && !table.has(tag2) {
                invalid_data!("Missing {} entry (tag {}) in {} section \
                               (since using {})",
                              name2,
                              tag2,
                              SECTION,
                              name1);
            }
        }

        // Validate dependency information:
        {
            let requirename_count =
                table.get(TAG_REQUIRENAME).unwrap().count();
            let requireflags_count =
                table.get(TAG_REQUIREFLAGS).unwrap().count();
            let requireversion_count =
                table.get(TAG_REQUIREVERSION).unwrap().count();
            if requireflags_count != requirename_count {
                invalid_data!("Counts for REQUIRENAME and REQUIREFLAGS \
                               entries don't match ({} vs. {})",
                              requirename_count,
                              requireflags_count);
            }
            if requireversion_count != requirename_count {
                invalid_data!("Counts for REQUIRENAME and REQUIREVERSION \
                               entries don't match ({} vs. {})",
                              requirename_count,
                              requireversion_count);
            }
        }

        // Validate file information:
        let use_old_filenames =
            !table
                .get_string_array(TAG_REQUIRENAME)
                .unwrap()
                .contains(&REQUIRE_COMPRESSED_FILE_NAMES.to_string());
        if use_old_filenames {
            let file_count = match table.get(TAG_OLDFILENAMES) {
                Some(value) => value.count(),
                None => {
                    invalid_data!("Missing {} entry (tag {}) in {} section \
                                   (since not using {})",
                                  "OLDFILENAMES",
                                  TAG_OLDFILENAMES,
                                  SECTION,
                                  REQUIRE_COMPRESSED_FILE_NAMES);
                }
            };
            for &(name, tag) in FILE_ENTRIES.iter() {
                table
                    .expect_count(SECTION,
                                  "OLDFILENAMES",
                                  TAG_OLDFILENAMES,
                                  file_count,
                                  name,
                                  tag)?;
            }
        } else {
            let dir_count = match table.get(TAG_DIRNAMES) {
                Some(value) => value.count(),
                None => {
                    invalid_data!("Missing DIRNAMES entry (tag {}) in {} \
                                   section (since using {})",
                                  TAG_DIRNAMES,
                                  SECTION,
                                  REQUIRE_COMPRESSED_FILE_NAMES);
                }
            };
            let file_count = match table.get(TAG_BASENAMES) {
                Some(value) => value.count(),
                None => {
                    invalid_data!("Missing BASENAMES entry (tag {}) in {} \
                                   section (since using {})",
                                  TAG_BASENAMES,
                                  SECTION,
                                  REQUIRE_COMPRESSED_FILE_NAMES);
                }
            };
            match table.get(TAG_DIRINDEXES) {
                Some(&IndexValue::Int32(ref values)) => {
                    for &value in values.iter() {
                        if value < 0 || (value as usize) >= dir_count {
                            invalid_data!("Invalid value ({}) in DIRINDEXES \
                                           entry (tag {}) in {} section \
                                           (DIRNAMES entry (tag {}) count is \
                                           {})",
                                          value,
                                          TAG_DIRINDEXES,
                                          SECTION,
                                          TAG_DIRNAMES,
                                          dir_count);
                        }
                    }
                }
                _ => {
                    invalid_data!("Missing {} entry (tag {}) in {} section \
                                   (since using {})",
                                  "DIRINDEXES",
                                  TAG_DIRINDEXES,
                                  SECTION,
                                  REQUIRE_COMPRESSED_FILE_NAMES);
                }
            }
            table
                .expect_count(SECTION,
                              "BASENAMES",
                              TAG_BASENAMES,
                              file_count,
                              "DIRINDEXES",
                              TAG_DIRINDEXES)?;
            for &(name, tag) in FILE_ENTRIES.iter() {
                table
                    .expect_count(SECTION,
                                  "BASENAMES",
                                  TAG_BASENAMES,
                                  file_count,
                                  name,
                                  tag)?;
            }
        }

        Ok(HeaderSection {
               table,
               use_old_filenames,
           })
    }

    pub(crate) fn write<W: Write + Seek>(&self, writer: W) -> io::Result<()> {
        self.table.write(writer, false)
    }

    /// Returns the raw underlying index table.
    pub fn table(&self) -> &IndexTable { &self.table }

    /// Returns the name of the package.
    pub fn package_name(&self) -> &str {
        self.table.get_string(TAG_NAME).unwrap()
    }

    pub(crate) fn set_package_name(&mut self, name: String) {
        self.table.set(TAG_NAME, IndexValue::String(name));
    }

    /// Returns the version number of the package.
    pub fn version_string(&self) -> &str {
        self.table.get_string(TAG_VERSION).unwrap()
    }

    pub(crate) fn set_version_string(&mut self, version: String) {
        self.table.set(TAG_VERSION, IndexValue::String(version));
    }

    /// Returns the release number of the package.
    pub fn release_string(&self) -> &str {
        self.table.get_string(TAG_RELEASE).unwrap()
    }

    pub(crate) fn set_release_string(&mut self, release: String) {
        self.table.set(TAG_RELEASE, IndexValue::String(release));
    }

    /// Returns the name of the author of the package.
    pub fn vendor_name(&self) -> Option<&str> {
        self.table.get_string(TAG_VENDOR)
    }

    /// Returns the name of the license which applies to this package.
    pub fn license_name(&self) -> &str {
        self.table.get_string(TAG_LICENSE).unwrap()
    }

    /// Returns the name of the compression type used for the Archive section
    /// (e.g. "gzip" or "bzip2").
    pub fn payload_compressor(&self) -> &str {
        self.table.get_string(TAG_PAYLOADCOMPRESSOR).unwrap()
    }

    pub(crate) fn set_payload_compressor(&mut self, compressor: String) {
        self.table.set(TAG_PAYLOADCOMPRESSOR, IndexValue::String(compressor));
    }

    /// Returns the compression level used for the Archive section (e.g. "9").
    pub fn payload_compression_level(&self) -> &str {
        self.table.get_string(TAG_PAYLOADFLAGS).unwrap()
    }

    pub(crate) fn set_payload_compression_level(&mut self, level: String) {
        self.table.set(TAG_PAYLOADFLAGS, IndexValue::String(level));
    }

    /// Returns an iterator over the files in the package.
    pub fn files(&self) -> FileInfoIter {
        let length = self.table.get(TAG_FILESIZES).unwrap().count();
        FileInfoIter {
            table: &self.table,
            use_old_filenames: self.use_old_filenames,
            next_index: 0,
            length,
        }
    }

    pub(crate) fn add_file(&mut self, file_info: FileInfo) {
        if self.use_old_filenames {
            self.table.push_string(TAG_OLDFILENAMES, file_info.name.clone());
        } else {
            let slash = file_info.name.rfind('/').map(|i| i + 1).unwrap_or(0);
            let (dirname, basename) = file_info.name.split_at(slash);
            let mut found = false;
            let mut dirindex = 0;
            for dir in self.table.get_string_array(TAG_DIRNAMES).unwrap() {
                if dir == dirname {
                    found = true;
                    break;
                }
                dirindex += 1;
            }
            if !found {
                self.table.push_string(TAG_DIRNAMES, dirname.to_string());
            }
            self.table.push_string(TAG_BASENAMES, basename.to_string());
            self.table.push_int32(TAG_DIRINDEXES, dirindex);
        }
        self.table.push_int32(TAG_FILESIZES, file_info.size);
        self.table.push_int16(TAG_FILEMODES, file_info.mode);
        self.table.push_int16(TAG_FILERDEVS, file_info.rdev);
        self.table.push_int32(TAG_FILEMTIMES, file_info.mtime);
        self.table.push_string(TAG_FILEMD5S, file_info.md5.clone());
        self.table.push_string(TAG_FILELINKTOS, file_info.linkto.clone());
        self.table.push_int32(TAG_FILEFLAGS, file_info.flags);
        self.table.push_string(TAG_FILEUSERNAME, file_info.user.clone());
        self.table.push_string(TAG_FILEGROUPNAME, file_info.group.clone());
        self.table.push_int32(TAG_FILEDEVICES, file_info.device);
        self.table.push_int32(TAG_FILEINODES, file_info.inode);
        self.table.push_string(TAG_FILELANGS, file_info.lang.clone());
    }

    /// Returns the timestamp when the package was built, if present.
    pub fn build_time(&self) -> Option<SystemTime> {
        self.table
            .get_nth_int32(TAG_BUILDTIME, 0)
            .map(convert::i32_to_system_time)
    }

    /// Returns an iterator over the entries in the package changelog.
    pub fn changelog(&self) -> ChangeLogIter {
        let length = self.table.get(TAG_CHANGELOGTIME).unwrap().count();
        ChangeLogIter {
            table: &self.table,
            next_index: 0,
            length,
        }
    }
}

// ========================================================================= //

/// Metadata about a file in the package.
#[allow(dead_code)]
pub struct FileInfo {
    name: String,
    size: i32,
    mode: i16,
    rdev: i16,
    mtime: i32,
    md5: String,
    linkto: String,
    flags: i32,
    user: String,
    group: String,
    device: i32,
    inode: i32,
    lang: String,
}

impl FileInfo {
    /// Constructs a new `FileInfo` with all other fields set to defaults.
    pub fn new<S: Into<String>>(install_path: S, file_size: u32) -> FileInfo {
        FileInfo {
            name: install_path.into(),
            size: file_size as i32,
            mode: 0o644,
            rdev: 0,
            mtime: 0,
            md5: String::new(),
            linkto: String::new(),
            flags: 0,
            user: "root".to_string(),
            group: "root".to_string(),
            device: 0,
            inode: 0,
            lang: String::new(),
        }
    }

    /// Constructs a new `FileInfo` from file metadata.
    pub fn from_metadata<S: Into<String>>(install_path: S,
                                          metadata: &Metadata)
                                          -> io::Result<FileInfo> {
        FileInfo::from_metadata_internal(install_path.into(), metadata)
    }

    #[cfg(unix)]
    fn from_metadata_internal(install_path: String, metadata: &Metadata)
                              -> io::Result<FileInfo> {
        let file_info = FileInfo {
            name: install_path,
            size: metadata.len() as i32,
            mode: metadata.mode() as i16,
            rdev: metadata.rdev() as i16,
            mtime: metadata.mtime() as i32,
            md5: String::new(),
            linkto: String::new(),
            flags: 0,
            user: "root".to_string(),
            group: "root".to_string(),
            device: 0,
            inode: metadata.ino() as i32,
            lang: String::new(),
        };
        Ok(file_info)
    }

    #[cfg(not(unix))]
    fn from_metadata_internal(install_path: String, metadata: &Metadata)
                              -> io::Result<FileInfo> {
        let modified_time = metadata.modified()?;
        let file_info = FileInfo {
            name: install_path,
            size: metadata.len() as i32,
            mode: if metadata.readonly() { 0o444 } else { 0o664 },
            rdev: 0,
            mtime: convert::system_time_to_u32(modified_time),
            md5: String::new(),
            linkto: String::new(),
            flags: 0,
            user: "root".to_string(),
            group: "root".to_string(),
            device: 0,
            inode: 0,
            lang: String::new(),
        };
        Ok(file_info)
    }

    /// Returns the install path of the file.
    pub fn name(&self) -> &str { &self.name }

    /// Returns the size of the file, in bytes.
    pub fn size(&self) -> u32 { ((self.size as i64) & 0xffffffff) as u32 }

    /// Returns the Unix mode bits for this file.
    pub fn mode(&self) -> u16 { ((self.mode as i32) & 0xffff) as u16 }

    /// Returns the file's last-modified timestamp.
    pub fn modified_time(&self) -> SystemTime {
        convert::i32_to_system_time(self.mtime)
    }

    /// Returns the file's expected MD5 checksum.
    pub fn md5_checksum(&self) -> &str { &self.md5 }

    /// Returns the target path if this file is a symbolic link.
    pub fn symlink_target(&self) -> Option<&str> {
        if self.linkto.is_empty() {
            None
        } else {
            Some(&self.linkto)
        }
    }

    /// Returns the name of the owner user for this file.
    pub fn user_name(&self) -> &str { &self.user }

    /// Returns the name of the group for this file.
    pub fn group_name(&self) -> &str { &self.group }

    /// Returns the original inode number of the file.
    pub fn inode(&self) -> u32 { ((self.inode as i64) & 0xffffffff) as u32 }
}

// ========================================================================= //

/// An iterator over metadata for files in the package.
pub struct FileInfoIter<'a> {
    table: &'a IndexTable,
    use_old_filenames: bool,
    next_index: usize,
    length: usize,
}

impl<'a> Iterator for FileInfoIter<'a> {
    type Item = FileInfo;

    fn next(&mut self) -> Option<FileInfo> {
        let idx = self.next_index;
        if idx == self.length {
            return None;
        }
        self.next_index += 1;
        let name = if self.use_old_filenames {
            self.table
                .get_nth_string(TAG_OLDFILENAMES, idx)
                .unwrap()
                .to_string()
        } else {
            let dir_index = self.table
                .get_nth_int32(TAG_DIRINDEXES, idx)
                .unwrap() as usize;
            let base_name =
                self.table.get_nth_string(TAG_BASENAMES, idx).unwrap();
            let dir_name =
                self.table.get_nth_string(TAG_DIRNAMES, dir_index).unwrap();
            let mut name = dir_name.to_string();
            name.push_str(base_name);
            name
        };
        let md5 = self.table.get_nth_string(TAG_FILEMD5S, idx).unwrap();
        let linkto = self.table.get_nth_string(TAG_FILELINKTOS, idx).unwrap();
        let user = self.table.get_nth_string(TAG_FILEUSERNAME, idx).unwrap();
        let group = self.table.get_nth_string(TAG_FILEGROUPNAME, idx).unwrap();
        let lang = self.table.get_nth_string(TAG_FILELANGS, idx).unwrap();
        let file_info = FileInfo {
            name,
            size: self.table.get_nth_int32(TAG_FILESIZES, idx).unwrap(),
            mode: self.table.get_nth_int16(TAG_FILEMODES, idx).unwrap(),
            rdev: self.table.get_nth_int16(TAG_FILERDEVS, idx).unwrap(),
            mtime: self.table.get_nth_int32(TAG_FILEMTIMES, idx).unwrap(),
            md5: md5.to_string(),
            linkto: linkto.to_string(),
            flags: self.table.get_nth_int32(TAG_FILEFLAGS, idx).unwrap(),
            user: user.to_string(),
            group: group.to_string(),
            device: self.table.get_nth_int32(TAG_FILEDEVICES, idx).unwrap(),
            inode: self.table.get_nth_int32(TAG_FILEINODES, idx).unwrap(),
            lang: lang.to_string(),
        };
        Some(file_info)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.length - self.next_index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for FileInfoIter<'a> {}

// ========================================================================= //

/// An entry in the package changelog.
pub struct ChangeLogEntry {
    timestamp: SystemTime,
    author: String,
    description: String,
}

impl ChangeLogEntry {
    /// Returns the timestamp when this change was made.
    pub fn timestamp(&self) -> SystemTime { self.timestamp }

    /// Returns the name of the author of this change.
    pub fn author(&self) -> &str { &self.author }

    /// Returns a description of this change.
    pub fn description(&self) -> &str { &self.description }
}

// ========================================================================= //

/// An iterator over entries in the package changelog.
pub struct ChangeLogIter<'a> {
    table: &'a IndexTable,
    next_index: usize,
    length: usize,
}

impl<'a> Iterator for ChangeLogIter<'a> {
    type Item = ChangeLogEntry;

    fn next(&mut self) -> Option<ChangeLogEntry> {
        let idx = self.next_index;
        if idx == self.length {
            return None;
        }
        self.next_index += 1;
        let time = self.table.get_nth_int32(TAG_CHANGELOGTIME, idx).unwrap();
        let author =
            self.table.get_nth_string(TAG_CHANGELOGNAME, idx).unwrap();
        let description =
            self.table.get_nth_string(TAG_CHANGELOGTEXT, idx).unwrap();
        let entry = ChangeLogEntry {
            timestamp: convert::i32_to_system_time(time),
            author: author.to_string(),
            description: description.to_string(),
        };
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.length - self.next_index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for ChangeLogIter<'a> {}

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
