use internal::index::{IndexTable, IndexType, IndexValue};
use std::io::{self, Read};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ========================================================================= //

/// The name of this section.
const SECTION: &str = "Header";

// ========================================================================= //

/// Required tag for the name of the package.
const TAG_NAME: i32 = 1000;
/// Required tag for the version number of the package.
const TAG_VERSION: i32 = 1001;
/// Required tag for the release number of the package.
const TAG_RELEASE: i32 = 1002;
/// Required tag for a one-line description of the package.
const TAG_SUMMARY: i32 = 1004;
/// Required tag for a longer description of the package.
const TAG_DESCRIPTION: i32 = 1005;
/// Required tag for the sum of the sizes of the regular files in the archive.
const TAG_SIZE: i32 = 1009;
/// Required tag for the license which applies to this package.
const TAG_LICENSE: i32 = 1014;
/// Required tag for the OS of the package.  The value must be "linux".
const TAG_OS: i32 = 1021;
/// Required tag for the archetecture that the package is for.
const TAG_ARCH: i32 = 1022;
/// Optional tag for the uncompressed size of the Payload archive, including
/// the cpio headers.
const TAG_ARCHIVESIZE: i32 = 1046;

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

const TAG_PROVIDENAME: i32 = 1047;
const TAG_REQUIREFLAGS: i32 = 1048;
const TAG_REQUIRENAME: i32 = 1049;
const TAG_REQUIREVERSION: i32 = 1050;

// Known index entires for Header section.  The bool indicates whether the
// entry is required (true) or optional (false).
#[cfg_attr(rustfmt, rustfmt_skip)]
const ENTRIES: &[(bool, &str, i32, IndexType, Option<usize>)] = &[
    // Package information:
    (true,  "NAME",         TAG_NAME,         IndexType::String,      None),
    (true,  "VERSION",      TAG_VERSION,      IndexType::String,      None),
    (true,  "RELEASE",      TAG_RELEASE,      IndexType::String,      None),
    (true,  "SUMMARY",      TAG_SUMMARY,      IndexType::I18nString,  None),
    (true,  "DESCRIPTION",  TAG_DESCRIPTION,  IndexType::I18nString,  None),
    (true,  "SIZE",         TAG_SIZE,         IndexType::Int32,       Some(1)),
    (true,  "LICENSE",      TAG_LICENSE,      IndexType::String,      None),
    (true,  "OS",           TAG_OS,           IndexType::String,      None),
    (true,  "ARCH",         TAG_ARCH,         IndexType::String,      None),
    (false, "ARCHIVESIZE",  TAG_ARCHIVESIZE,  IndexType::Int32,       Some(1)),
    // TODO: Add others.
    // Installation information:
    // TODO: Add these.
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
    (true,  "REQUIREFLAGS",  TAG_REQUIREFLAGS,  IndexType::Int32,       None),
    (true,  "REQUIRENAME",   TAG_REQUIRENAME,   IndexType::StringArray, None),
    (true,  "REQUIREVERSION",TAG_REQUIREVERSION,IndexType::StringArray, None),
    // TODO: Add others.
    // Other information:
    // TODO: Add these.
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

/// Can be listed under `TAG_REQUIRENAME` to indicate that we're not using
/// `TAG_OLDFILENAMES`.
const REQUIRE_COMPRESSED_FILE_NAMES: &str = "rpmlib(CompressedFileNames)";

/// The required value under `TAG_OS`.
const OS_STRING: &str = "linux";

// ========================================================================= //

/// The "Header" section of an RPM package file.
pub struct HeaderSection {
    table: IndexTable,
    use_old_filenames: bool,
}

impl HeaderSection {
    pub(crate) fn read<R: Read>(reader: R) -> io::Result<HeaderSection> {
        let table = IndexTable::read(reader)?;
        for &(required, name, tag, itype, count) in ENTRIES.iter() {
            table.validate(SECTION, required, name, tag, itype, count)?;
        }

        // Validate package information:
        {
            let os_string = table.get_string(TAG_OS).unwrap();
            if os_string != OS_STRING {
                invalid_data!("Incorrect value for OS entry (tag {}) in \
                               {} section (was {:?}, but must be {:?})",
                              TAG_OS,
                              SECTION,
                              os_string,
                              OS_STRING);
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
        let use_old_filenames = match table.get(TAG_REQUIRENAME) {
            Some(&IndexValue::StringArray(ref values)) => {
                !values.contains(&REQUIRE_COMPRESSED_FILE_NAMES.to_string())
            }
            _ => panic!(),
        };
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

    /// Returns the raw underlying index table.
    pub fn table(&self) -> &IndexTable { &self.table }

    /// Returns the name of the package.
    pub fn package_name(&self) -> &str {
        self.table.get_string(TAG_NAME).unwrap()
    }

    /// Returns the version number of the package.
    pub fn version_string(&self) -> &str {
        self.table.get_string(TAG_VERSION).unwrap()
    }

    /// Returns the release number of the package.
    pub fn release_string(&self) -> &str {
        self.table.get_string(TAG_RELEASE).unwrap()
    }

    /// Returns an iterator over the files in the package.
    pub fn files(&self) -> FileInfoIter {
        let length = match self.table.get(TAG_FILESIZES) {
            Some(&IndexValue::Int32(ref values)) => values.len(),
            _ => panic!(),
        };
        FileInfoIter {
            table: &self.table,
            use_old_filenames: self.use_old_filenames,
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
    /// Returns the install path of the file.
    pub fn name(&self) -> &str { &self.name }

    /// Returns the size of the file, in bytes.
    pub fn size(&self) -> u64 { ((self.size as i64) & 0xffffffff) as u64 }

    /// Returns the file's last-modified timestamp.
    pub fn modified_time(&self) -> SystemTime {
        let seconds = ((self.mtime as i64) & 0xffffffff) as u64;
        UNIX_EPOCH + Duration::new(seconds, 0)
    }

    /// Returns the file's expected MD5 checksum.
    pub fn md5_checksum(&self) -> &str { &self.md5 }
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
