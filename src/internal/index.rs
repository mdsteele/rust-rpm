use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

// ========================================================================= //

const MAGIC_NUMBER: u32 = 0x8eade801;

// ========================================================================= //

/// A key-value table.
pub struct IndexTable {
    values: BTreeMap<i32, IndexValue>,
}

impl IndexTable {
    pub(crate) fn new() -> IndexTable {
        IndexTable { values: BTreeMap::new() }
    }

    pub(crate) fn read<R: Read>(mut reader: R, pad: bool)
                                -> io::Result<IndexTable> {
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
        let mut data_size = reader.read_u32::<BigEndian>()? as usize;
        if pad {
            data_size = ((data_size + 7) / 8) * 8;
        }
        let mut index_map = BTreeMap::new();
        for _ in 0..num_values {
            let tag = reader.read_i32::<BigEndian>()?;
            if index_map.contains_key(&tag) {
                invalid_data!("Repeated tag in index table ({})", tag);
            }
            let typenum = reader.read_i32::<BigEndian>()?;
            let index_type = match IndexType::from_number(typenum) {
                Some(index_type) => index_type,
                None => {
                    invalid_data!("Invalid type number in index entry ({})",
                                  typenum);
                }
            };
            let offset = reader.read_u32::<BigEndian>()?;
            let count = reader.read_u32::<BigEndian>()?;
            index_map.insert(tag, (index_type, offset, count));
        }
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;
        let mut cursor = Cursor::new(&data);
        // TODO: Get correct locale count for I18nStrings.
        let mut value_map = BTreeMap::new();
        for (tag, (index_type, offset, count)) in index_map.into_iter() {
            cursor.seek(SeekFrom::Start(offset as u64))?;
            let value = IndexValue::read(&mut cursor, index_type, count)?;
            value_map.insert(tag, value);
        }
        Ok(IndexTable { values: value_map })
    }

    pub(crate) fn write<W: Write + Seek>(&self, mut writer: W, pad: bool)
                                         -> io::Result<()> {
        // Build the index store:
        let mut data = Vec::<u8>::new();
        let mut entry_map = BTreeMap::new();
        for (&tag, value) in self.values.iter() {
            let alignment = value.index_type().alignment();
            let remainder = data.len() % alignment;
            if remainder != 0 {
                let pad_to = data.len() + alignment - remainder;
                data.resize(pad_to, 0);
            }
            entry_map.insert(tag, (value, data.len() as u32));
            value.write(&mut data)?;
        }
        if pad {
            let alignment = 8;
            let remainder = data.len() % alignment;
            if remainder != 0 {
                let pad_to = data.len() + alignment - remainder;
                data.resize(pad_to, 0);
            }
        }

        // Write the index table to the file:
        writer.write_u32::<BigEndian>(MAGIC_NUMBER)?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u32::<BigEndian>(self.values.len() as u32)?;
        writer.write_u32::<BigEndian>(data.len() as u32)?;
        for (&tag, &(value, offset)) in entry_map.iter() {
            writer.write_i32::<BigEndian>(tag)?;
            writer.write_i32::<BigEndian>(value.index_type().number())?;
            writer.write_u32::<BigEndian>(offset)?;
            writer.write_u32::<BigEndian>(value.count() as u32)?;
        }
        writer.write_all(&data)?;
        Ok(())
    }

    /// Returns the map of all values.
    pub fn map(&self) -> &BTreeMap<i32, IndexValue> { &self.values }

    /// Returns true if the given tag is present.
    pub fn has(&self, tag: i32) -> bool { self.values.contains_key(&tag) }

    /// Returns the value for the given tag, if if is present.
    pub fn get(&self, tag: i32) -> Option<&IndexValue> {
        self.values.get(&tag)
    }

    /// Sets the value for the given tag.
    pub fn set(&mut self, tag: i32, value: IndexValue) {
        self.values.insert(tag, value);
    }

    /// Returns the value for the given tag, if it is present and is a string.
    pub(crate) fn get_string(&self, tag: i32) -> Option<&str> {
        match self.get(tag) {
            Some(&IndexValue::String(ref string)) => Some(string.as_str()),
            _ => None,
        }
    }

    /// Returns the value for the given tag, if it is present and is binary.
    pub(crate) fn get_binary(&self, tag: i32) -> Option<&[u8]> {
        match self.get(tag) {
            Some(&IndexValue::Binary(ref binary)) => Some(binary.as_slice()),
            _ => None,
        }
    }

    /// Returns the value for the given tag, if it is present and is a string
    /// array.
    pub(crate) fn get_string_array(&self, tag: i32) -> Option<&[String]> {
        match self.get(tag) {
            Some(&IndexValue::StringArray(ref array)) => {
                Some(array.as_slice())
            }
            _ => None,
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is a
    /// string array or i18n string array, and has that many values.
    pub(crate) fn get_nth_string(&self, tag: i32, n: usize) -> Option<&str> {
        match self.get(tag) {
            Some(&IndexValue::StringArray(ref values)) |
            Some(&IndexValue::I18nString(ref values)) => {
                if n < values.len() {
                    Some(&values[n])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Adds a string onto the end of an existing string array.  Panics if
    /// there is not already a string array entry for the given tag.
    pub(crate) fn push_string(&mut self, tag: i32, string: String) {
        match self.values.get_mut(&tag) {
            Some(&mut IndexValue::StringArray(ref mut array)) => {
                array.push(string);
            }
            Some(value) => {
                panic!("Internal error: Entry for tag {} is {:?}, not {:?}",
                       tag,
                       value.index_type(),
                       IndexType::StringArray);
            }
            None => panic!("Internal error: No entry for tag {}", tag),
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is an
    /// int16 array, and has that many values.
    pub(crate) fn get_nth_int16(&self, tag: i32, n: usize) -> Option<u16> {
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

    /// Adds an `i16` onto the end of an existing array.  Panics if there is
    /// not already an `Int16` entry for the given tag.
    pub(crate) fn push_int16(&mut self, tag: i32, value: u16) {
        match self.values.get_mut(&tag) {
            Some(&mut IndexValue::Int16(ref mut array)) => {
                array.push(value);
            }
            Some(value) => {
                panic!("Internal error: Entry for tag {} is {:?}, not {:?}",
                       tag,
                       value.index_type(),
                       IndexType::Int16);
            }
            None => panic!("Internal error: No entry for tag {}", tag),
        }
    }

    /// Returns the nth value for the given tag, if it is present, and is an
    /// int32 array, and has that many values.
    pub(crate) fn get_nth_int32(&self, tag: i32, n: usize) -> Option<u32> {
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

    /// Adds an `i32` onto the end of an existing array.  Panics if there is
    /// not already an `Int32` entry for the given tag.
    pub(crate) fn push_int32(&mut self, tag: i32, value: u32) {
        match self.values.get_mut(&tag) {
            Some(&mut IndexValue::Int32(ref mut array)) => {
                array.push(value);
            }
            Some(value) => {
                panic!("Internal error: Entry for tag {} is {:?}, not {:?}",
                       tag,
                       value.index_type(),
                       IndexType::Int32);
            }
            None => panic!("Internal error: No entry for tag {}", tag),
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
    Int8(Vec<u8>),
    /// An array of 16-bit integers.
    Int16(Vec<u16>),
    /// An array of 32-bit integers.
    Int32(Vec<u32>),
    /// An array of 64-bit integers.
    Int64(Vec<u64>),
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
    fn read<R: Read>(reader: &mut R, index_type: IndexType, count: u32)
                     -> io::Result<IndexValue> {
        match index_type {
            IndexType::Null => Ok(IndexValue::Null),
            IndexType::Char => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(IndexValue::Char(buffer))
            }
            IndexType::Int8 => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(IndexValue::Int8(buffer))
            }
            IndexType::Int16 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_u16::<BigEndian>()?);
                }
                Ok(IndexValue::Int16(array))
            }
            IndexType::Int32 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_u32::<BigEndian>()?);
                }
                Ok(IndexValue::Int32(array))
            }
            IndexType::Int64 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_u64::<BigEndian>()?);
                }
                Ok(IndexValue::Int64(array))
            }
            IndexType::String => {
                if count != 1 {
                    invalid_data!("Invalid count in index entry for type \
                                   String (was {}, but must be 1)",
                                  count);
                }
                let string = read_nul_terminated_string(reader)?;
                Ok(IndexValue::String(string))
            }
            IndexType::Binary => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(IndexValue::Binary(buffer))
            }
            IndexType::StringArray => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(IndexValue::StringArray(array))
            }
            IndexType::I18nString => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(IndexValue::I18nString(array))
            }
        }
    }

    fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match *self {
            IndexValue::Null => {}
            IndexValue::Char(ref array) |
            IndexValue::Int8(ref array) |
            IndexValue::Binary(ref array) => {
                writer.write_all(array)?;
            }
            IndexValue::Int16(ref array) => {
                for &value in array {
                    writer.write_u16::<BigEndian>(value)?;
                }
            }
            IndexValue::Int32(ref array) => {
                for &value in array {
                    writer.write_u32::<BigEndian>(value)?;
                }
            }
            IndexValue::Int64(ref array) => {
                for &value in array {
                    writer.write_u64::<BigEndian>(value)?;
                }
            }
            IndexValue::String(ref string) => {
                write_nul_terminated_string(writer, string.as_str())?;
            }
            IndexValue::StringArray(ref array) |
            IndexValue::I18nString(ref array) => {
                for string in array {
                    write_nul_terminated_string(writer, string.as_str())?;
                }
            }
        }
        Ok(())
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

fn write_nul_terminated_string<W: Write>(writer: &mut W, string: &str)
                                         -> io::Result<()> {
    writer.write_all(string.as_bytes())?;
    writer.write_u8(0)?;
    Ok(())
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

impl IndexType {
    fn from_number(number: i32) -> Option<IndexType> {
        match number {
            0 => Some(IndexType::Null),
            1 => Some(IndexType::Char),
            2 => Some(IndexType::Int8),
            3 => Some(IndexType::Int16),
            4 => Some(IndexType::Int32),
            5 => Some(IndexType::Int64),
            6 => Some(IndexType::String),
            7 => Some(IndexType::Binary),
            8 => Some(IndexType::StringArray),
            9 => Some(IndexType::I18nString),
            _ => None,
        }
    }

    fn number(&self) -> i32 {
        match *self {
            IndexType::Null => 0,
            IndexType::Char => 1,
            IndexType::Int8 => 2,
            IndexType::Int16 => 3,
            IndexType::Int32 => 4,
            IndexType::Int64 => 5,
            IndexType::String => 6,
            IndexType::Binary => 7,
            IndexType::StringArray => 8,
            IndexType::I18nString => 9,
        }
    }

    fn alignment(&self) -> usize {
        match *self {
            IndexType::Null => 1,
            IndexType::Char => 1,
            IndexType::Int8 => 1,
            IndexType::Int16 => 2,
            IndexType::Int32 => 4,
            IndexType::Int64 => 8,
            IndexType::String => 1,
            IndexType::Binary => 1,
            IndexType::StringArray => 1,
            IndexType::I18nString => 1,
        }
    }

    pub(crate) fn default_value(&self) -> IndexValue {
        match *self {
            IndexType::Null => IndexValue::Null,
            IndexType::Char => IndexValue::Char(Vec::new()),
            IndexType::Int8 => IndexValue::Int8(Vec::new()),
            IndexType::Int16 => IndexValue::Int16(Vec::new()),
            IndexType::Int32 => IndexValue::Int32(Vec::new()),
            IndexType::Int64 => IndexValue::Int64(Vec::new()),
            IndexType::String => IndexValue::String(String::new()),
            IndexType::Binary => IndexValue::Binary(Vec::new()),
            IndexType::StringArray => IndexValue::StringArray(Vec::new()),
            IndexType::I18nString => IndexValue::I18nString(Vec::new()),
        }
    }
}

// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::{IndexTable, IndexType, IndexValue};
    use std::io::Cursor;

    const ALL_INDEX_TYPES: &[IndexType] = &[
        IndexType::Null,
        IndexType::Char,
        IndexType::Int8,
        IndexType::Int16,
        IndexType::Int32,
        IndexType::Int64,
        IndexType::String,
        IndexType::Binary,
        IndexType::StringArray,
        IndexType::I18nString,
    ];

    #[test]
    fn index_type_number_round_trip() {
        for &index_type in ALL_INDEX_TYPES {
            assert_eq!(IndexType::from_number(index_type.number()),
                       Some(index_type));
        }
    }

    #[test]
    fn index_type_default_value_round_trip() {
        for &index_type in ALL_INDEX_TYPES {
            assert_eq!(index_type.default_value().index_type(), index_type);
        }
    }

    #[test]
    fn index_table_round_trip() {
        let mut table = IndexTable::new();
        table.set(1000, IndexValue::Null);
        table.set(1001, IndexValue::Char(vec![1, 2, 3, 4, 5]));
        table.set(1002, IndexValue::Int8(vec![6, 7]));
        table.set(1003, IndexValue::Int16(vec![890]));
        table.set(1004, IndexValue::Int32(vec![123, 456, 789]));
        table.set(1005, IndexValue::Int64(vec![9876543210]));
        table.set(1006, IndexValue::String("Hello, world!".to_string()));
        table.set(1007, IndexValue::Binary(b"\x12\x34\x56\x78\x9a".to_vec()));
        table.set(
            1008,
            IndexValue::StringArray(
                vec!["foo".to_string(), "bar".to_string()],
            ),
        );
        let mut output = Cursor::new(Vec::new());
        table.write(&mut output, false).unwrap();
        let output = output.into_inner();
        let table = IndexTable::read(output.as_slice(), false).unwrap();
        assert_eq!(table.map().len(), 9);
        assert_eq!(table.get(1000), Some(&IndexValue::Null));
        assert_eq!(table.get(1001),
                   Some(&IndexValue::Char(vec![1, 2, 3, 4, 5])));
        assert_eq!(table.get(1002), Some(&IndexValue::Int8(vec![6, 7])));
        assert_eq!(table.get(1003), Some(&IndexValue::Int16(vec![890])));
        assert_eq!(table.get(1004),
                   Some(&IndexValue::Int32(vec![123, 456, 789])));
        assert_eq!(table.get(1005),
                   Some(&IndexValue::Int64(vec![9876543210])));
        assert_eq!(table.get(1006),
                   Some(&IndexValue::String("Hello, world!".to_string())));
        assert_eq!(table.get(1007),
                   Some(&IndexValue::Binary(b"\x12\x34\x56\x78\x9a"
                                                .to_vec())));
        assert_eq!(
            table.get(1008),
            Some(&IndexValue::StringArray(
                vec!["foo".to_string(), "bar".to_string()],
            ))
        );
    }
}

// ========================================================================= //
