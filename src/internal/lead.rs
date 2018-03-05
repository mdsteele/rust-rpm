use byteorder::{BigEndian, ReadBytesExt};
use std::io::{self, Read};

// ========================================================================= //

pub const MAGIC_NUMBER: u32 = 0xedabeedb;
pub const VERSION_MAJOR: u8 = 3;
pub const VERSION_MINOR: u8 = 0;
pub const SIGNATURE_TYPE: u16 = 5;

// ========================================================================= //

/// The "Lead" section of an RPM package file.
pub struct Lead {
    package_type: PackageType,
    name: Vec<u8>,
    osnum: u16,
}

impl Lead {
    /// Reads in an RPM package file lead section.
    pub(crate) fn read<R: Read>(mut reader: R) -> io::Result<Lead> {
        let magic_number = reader.read_u32::<BigEndian>()?;
        if magic_number != MAGIC_NUMBER {
            invalid_data!("Not an RPM package (invalid magic number)");
        }
        let version_major = reader.read_u8()?;
        let version_minor = reader.read_u8()?;
        if version_major != VERSION_MAJOR || version_minor != VERSION_MINOR {
            invalid_data!("Can't read RPM format version {}.{} \
                           (only version {}.{} is supported)",
                          version_major,
                          version_minor,
                          VERSION_MAJOR,
                          VERSION_MINOR);
        }
        let package_type_num = reader.read_u16::<BigEndian>()?;
        let package_type = match PackageType::from_number(package_type_num) {
            Some(ptype) => ptype,
            None => {
                invalid_data!("Invalid package type ({})", package_type_num);
            }
        };
        // In theory, the arch field indicates the architecture that this
        // package is for.  But apparently in practice this field is unused.
        // See http://stackoverflow.com/questions/39416934 for details.
        let _arch = reader.read_u16::<BigEndian>()?;
        let mut name = vec![0u8; 66];
        reader.read_exact(&mut name)?;
        while name.last() == Some(&0) {
            name.pop();
        }
        let osnum = reader.read_u16::<BigEndian>()?;
        let signature_type = reader.read_u16::<BigEndian>()?;
        if signature_type != SIGNATURE_TYPE {
            invalid_data!("Invalid RPM signature type ({})", signature_type);
        }
        let mut reserved = [0u8; 16];
        reader.read_exact(&mut reserved)?;
        Ok(Lead {
               package_type,
               name,
               osnum,
           })
    }

    /// Returns what type of package this is (binary or source).
    pub fn package_type(&self) -> PackageType { self.package_type }

    /// Returns the name of the package.
    pub fn name(&self) -> &[u8] { &self.name }

    /// Returns the OS number that the package is for (e.g. 1 for Linux).
    pub fn osnum(&self) -> u16 { self.osnum }
}

// ========================================================================= //

/// A type of RPM package.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PackageType {
    /// A binary package.
    Binary,
    /// A source package.
    Source,
}

impl PackageType {
    pub(crate) fn from_number(number: u16) -> Option<PackageType> {
        match number {
            0 => Some(PackageType::Binary),
            1 => Some(PackageType::Source),
            _ => None,
        }
    }
}

// ========================================================================= //
