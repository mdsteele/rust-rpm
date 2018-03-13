use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

// ========================================================================= //

pub const MAGIC_NUMBER: u32 = 0xedabeedb;
pub const VERSION_MAJOR: u8 = 3;
pub const VERSION_MINOR: u8 = 0;
pub const OS_NUM: u16 = 1;
pub const SIGNATURE_TYPE: u16 = 5;

// ========================================================================= //

/// The "Lead" section of an RPM package file.
pub struct LeadSection {
    package_type: PackageType,
    name: Vec<u8>,
}

impl LeadSection {
    pub(crate) fn new(package_type: PackageType, name: Vec<u8>)
                      -> LeadSection {
        LeadSection { package_type, name }
    }

    /// Reads in an RPM package file lead section.
    pub(crate) fn read<R: Read>(mut reader: R) -> io::Result<LeadSection> {
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
        let os_num = reader.read_u16::<BigEndian>()?;
        if os_num != OS_NUM {
            invalid_data!("Invalid RPM OS number ({})", os_num);
        }
        let signature_type = reader.read_u16::<BigEndian>()?;
        if signature_type != SIGNATURE_TYPE {
            invalid_data!("Invalid RPM signature type ({})", signature_type);
        }
        let mut reserved = [0u8; 16];
        reader.read_exact(&mut reserved)?;
        Ok(LeadSection { package_type, name })
    }

    pub(crate) fn write<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u32::<BigEndian>(MAGIC_NUMBER)?;
        writer.write_u8(VERSION_MAJOR)?;
        writer.write_u8(VERSION_MINOR)?;
        writer.write_u16::<BigEndian>(self.package_type.number())?;
        writer.write_u16::<BigEndian>(1)?; // arch
        // The name field is always 66 bytes long.  The name itself must be at
        // most 65 bytes and NUL-terminated.
        let mut name = self.name.clone();
        name.resize(65, 0);
        name.push(0);
        writer.write_all(&name)?;
        writer.write_u16::<BigEndian>(OS_NUM)?;
        writer.write_u16::<BigEndian>(SIGNATURE_TYPE)?;
        let reserved = [0u8; 16];
        writer.write_all(&reserved)?;
        Ok(())
    }

    /// Returns what type of package this is (binary or source).
    pub fn package_type(&self) -> PackageType { self.package_type }

    /// Returns the name of the package.
    pub fn name(&self) -> &[u8] { &self.name }
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

    pub(crate) fn number(&self) -> u16 {
        match *self {
            PackageType::Binary => 0,
            PackageType::Source => 1,
        }
    }
}

// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::{LeadSection, PackageType};

    #[test]
    fn package_type_number_round_trip() {
        let package_types = &[PackageType::Binary, PackageType::Source];
        for &package_type in package_types {
            assert_eq!(PackageType::from_number(package_type.number()),
                       Some(package_type));
        }
    }

    #[test]
    fn lead_section_round_trip() {
        let name: &[u8] = b"foobar-1.4.0-123";
        let lead = LeadSection::new(PackageType::Source, name.to_vec());
        let mut output = Vec::new();
        lead.write(&mut output).unwrap();
        let lead = LeadSection::read(output.as_slice()).unwrap();
        assert_eq!(lead.package_type(), PackageType::Source);
        assert_eq!(lead.name(), name);
    }
}

// ========================================================================= //
