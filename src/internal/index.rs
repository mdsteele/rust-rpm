use byteorder::{BigEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

// ========================================================================= //

const MAGIC_NUMBER: u32 = 0x8eade801;

// ========================================================================= //

/// A key-value table.
pub struct IndexTable {
    values: BTreeMap<i32, IndexValue>,
}

impl IndexTable {
    pub(crate) fn read<R: Read>(mut reader: R) -> io::Result<IndexTable> {
        let magic_number = reader.read_u32::<BigEndian>()?;
        if magic_number != MAGIC_NUMBER {
            invalid_data!("Invalid magic number for index table ({:08x})",
                          magic_number);
        }
        let reserved = reader.read_u32::<BigEndian>()?;
        if reserved != 0 {
            invalid_data!("Invalid reserved field for index table ({:08x})",
                          reserved);
        }
        let num_values = reader.read_u32::<BigEndian>()? as usize;
        let data_size = reader.read_u32::<BigEndian>()? as usize;
        let data_size = ((data_size + 7) / 8) * 8;
        let mut index_map = BTreeMap::new();
        for _ in 0..num_values {
            let tag = reader.read_i32::<BigEndian>()?;
            if index_map.contains_key(&tag) {
                invalid_data!("Repeated tag in index table ({})", tag);
            }
            let typenum = reader.read_i32::<BigEndian>()?;
            let offset = reader.read_u32::<BigEndian>()?;
            let count = reader.read_u32::<BigEndian>()?;
            index_map.insert(tag, (typenum, offset, count));
        }
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;
        let mut cursor = Cursor::new(&data);
        // TODO: Get correct locale count for I18nStrings.
        let mut value_map = BTreeMap::new();
        for (tag, (typenum, offset, count)) in index_map.into_iter() {
            cursor.seek(SeekFrom::Start(offset as u64))?;
            let value = IndexValue::read(&mut cursor, typenum, count)?;
            value_map.insert(tag, value);
        }
        Ok(IndexTable { values: value_map })
    }

    /// Returns the map of all values.
    pub fn map(&self) -> &BTreeMap<i32, IndexValue> { &self.values }

    /// Returns true if the given tag is present.
    pub fn has(&self, tag: i32) -> bool { self.values.contains_key(&tag) }

    /// Returns the value for the given tag, if if is present.
    pub fn get(&self, tag: i32) -> Option<&IndexValue> {
        self.values.get(&tag)
    }

    /// Returns the value for the given tag, if it is present and is a string.
    pub fn get_string(&self, tag: i32) -> Option<&str> {
        match self.get(tag) {
            Some(&IndexValue::String(ref string)) => Some(string.as_str()),
            _ => None,
        }
    }

    /// Returns the value for the given tag, if it is present and is binary.
    pub fn get_binary(&self, tag: i32) -> Option<&[u8]> {
        match self.get(tag) {
            Some(&IndexValue::Binary(ref binary)) => Some(binary.as_slice()),
            _ => None,
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is a
    /// string array, and has that many values.
    pub fn get_nth_string(&self, tag: i32, n: usize) -> Option<&str> {
        match self.get(tag) {
            Some(&IndexValue::StringArray(ref values)) => {
                if n < values.len() {
                    Some(&values[n])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is an
    /// int16 array, and has that many values.
    pub fn get_nth_int16(&self, tag: i32, n: usize) -> Option<i16> {
        match self.get(tag) {
            Some(&IndexValue::Int16(ref values)) => {
                if n < values.len() {
                    Some(values[n])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is an
    /// int32 array, and has that many values.
    pub fn get_nth_int32(&self, tag: i32, n: usize) -> Option<i32> {
        match self.get(tag) {
            Some(&IndexValue::Int32(ref values)) => {
                if n < values.len() {
                    Some(values[n])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(crate) fn validate(&self, section: &str, required: bool, name: &str,
                           tag: i32, itype: IndexType, count: Option<usize>)
                           -> io::Result<()> {
        if let Some(value) = self.get(tag) {
            let actual_itype = value.index_type();
            if actual_itype != itype {
                invalid_data!("Incorrect type for {} entry (tag {}) in {} \
                               section (was {:?}, but must be {:?})",
                              name,
                              tag,
                              section,
                              actual_itype,
                              itype);
            }
            if let Some(expected_count) = count {
                let actual_count = value.count();
                if actual_count != expected_count {
                    invalid_data!("Incorrect number of values for {} entry \
                                   (tag {}) in {} section \
                                   (was {}, but must be {})",
                                  name,
                                  tag,
                                  section,
                                  actual_count,
                                  expected_count);
                }
            }
        } else if required {
            invalid_data!("Missing {} entry (tag {}) in {} section",
                          name,
                          tag,
                          section);
        }
        Ok(())
    }

    pub(crate) fn expect_count(&self, section: &str, name1: &str, tag1: i32,
                               count1: usize, name2: &str, tag2: i32)
                               -> io::Result<()> {
        let count2 = self.get(tag2).map(IndexValue::count).unwrap_or(0);
        if count1 != count2 {
            invalid_data!("Counts for {} entry (tag {}) and {} entry (tag {}) \
                           in {} section don't match ({} vs. {})",
                          name1,
                          tag1,
                          name2,
                          tag2,
                          section,
                          count1,
                          count2);
        }
        Ok(())
    }

    pub(crate) fn expect_string_value(&self, section: &str, name: &str,
                                      tag: i32, value: &str)
                                      -> io::Result<()> {
        let actual_value = self.get_string(tag).unwrap();
        if actual_value != value {
            invalid_data!("Incorrect value for {} entry (tag {}) in \
                           {} section (was {:?}, but must be {:?})",
                          name,
                          tag,
                          section,
                          actual_value,
                          value);
        }
        Ok(())
    }
}

// ========================================================================= //

/// A value stored in an index table.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum IndexValue {
    /// A null value.
    Null,
    /// An array of chars.
    Char(Vec<u8>),
    /// An array of 8-bit integers.
    Int8(Vec<i8>),
    /// An array of 16-bit integers.
    Int16(Vec<i16>),
    /// An array of 32-bit integers.
    Int32(Vec<i32>),
    /// An array of 64-bit integers.
    Int64(Vec<i64>),
    /// A single string.
    String(String),
    /// A single binary blob.
    Binary(Vec<u8>),
    /// An array of strings.
    StringArray(Vec<String>),
    /// An array of localized strings.
    I18nString(Vec<String>),
}

impl IndexValue {
    fn read<R: Read>(reader: &mut R, typenum: i32, count: u32)
                     -> io::Result<IndexValue> {
        match typenum {
            0 => Ok(IndexValue::Null),
            1 => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(IndexValue::Char(buffer))
            }
            2 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i8()?);
                }
                Ok(IndexValue::Int8(array))
            }
            3 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i16::<BigEndian>()?);
                }
                Ok(IndexValue::Int16(array))
            }
            4 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i32::<BigEndian>()?);
                }
                Ok(IndexValue::Int32(array))
            }
            5 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i64::<BigEndian>()?);
                }
                Ok(IndexValue::Int64(array))
            }
            6 => {
                if count != 1 {
                    invalid_data!("Invalid count in index entry for type \
                                   String (was {}, but must be 1)",
                                  count);
                }
                let string = read_nul_terminated_string(reader)?;
                Ok(IndexValue::String(string))
            }
            7 => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(IndexValue::Binary(buffer))
            }
            8 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(IndexValue::StringArray(array))
            }
            9 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(IndexValue::I18nString(array))
            }
            _ => {
                invalid_data!("Invalid type number in index entry ({})",
                              typenum)
            }
        }
    }

    pub(crate) fn index_type(&self) -> IndexType {
        match *self {
            IndexValue::Null => IndexType::Null,
            IndexValue::Char(_) => IndexType::Char,
            IndexValue::Int8(_) => IndexType::Int8,
            IndexValue::Int16(_) => IndexType::Int16,
            IndexValue::Int32(_) => IndexType::Int32,
            IndexValue::Int64(_) => IndexType::Int64,
            IndexValue::String(_) => IndexType::String,
            IndexValue::Binary(_) => IndexType::Binary,
            IndexValue::StringArray(_) => IndexType::StringArray,
            IndexValue::I18nString(_) => IndexType::I18nString,
        }
    }

    pub(crate) fn count(&self) -> usize {
        match *self {
            IndexValue::Null => 1,
            IndexValue::Char(ref values) => values.len(),
            IndexValue::Int8(ref values) => values.len(),
            IndexValue::Int16(ref values) => values.len(),
            IndexValue::Int32(ref values) => values.len(),
            IndexValue::Int64(ref values) => values.len(),
            IndexValue::String(_) => 1,
            IndexValue::Binary(ref data) => data.len(),
            IndexValue::StringArray(ref values) => values.len(),
            IndexValue::I18nString(ref values) => values.len(),
        }
    }
}

fn read_nul_terminated_string<R: Read>(reader: &mut R) -> io::Result<String> {
    let mut buffer = Vec::<u8>::new();
    loop {
        let byte = reader.read_u8()?;
        if byte == 0 {
            break;
        }
        buffer.push(byte);
    }
    match String::from_utf8(buffer) {
        Ok(string) => Ok(string),
        Err(_) => invalid_data!("Invalid UTF-8 in header string entry"),
    }
}

// ========================================================================= //

/// A type of value stored in a header table.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IndexType {
    /// A null value.
    Null,
    /// An array of chars.
    Char,
    /// An array of 8-bit integers.
    Int8,
    /// An array of 16-bit integers.
    Int16,
    /// An array of 32-bit integers.
    Int32,
    /// An array of 64-bit integers.
    Int64,
    /// A single string.
    String,
    /// A single binary blob.
    Binary,
    /// An array of strings.
    StringArray,
    /// An array of localized strings.
    I18nString,
}

// ========================================================================= //
