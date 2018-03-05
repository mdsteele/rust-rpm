use byteorder::{BigEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

// ========================================================================= //

const HEADER_MAGIC_NUMBER: u32 = 0x8eade801;

// ========================================================================= //

/// A key-value table.
pub struct HeaderTable {
    values: BTreeMap<u32, HeaderValue>,
}

impl HeaderTable {
    pub(crate) fn read<R: Read>(mut reader: R) -> io::Result<HeaderTable> {
        let magic_number = reader.read_u32::<BigEndian>()?;
        if magic_number != HEADER_MAGIC_NUMBER {
            invalid_data!("Invalid header magic number ({:08x})",
                          magic_number);
        }
        let reserved = reader.read_u32::<BigEndian>()?;
        if reserved != 0 {
            invalid_data!("Invalid header reserved field ({:08x})", reserved);
        }
        let num_values = reader.read_u32::<BigEndian>()? as usize;
        let data_size = reader.read_u32::<BigEndian>()? as usize;
        let data_size = ((data_size + 7) / 8) * 8;
        let mut index_map = BTreeMap::new();
        for _ in 0..num_values {
            let tag = reader.read_u32::<BigEndian>()?;
            if index_map.contains_key(&tag) {
                invalid_data!("Repeated tag in header ({})", tag);
            }
            let typenum = reader.read_u32::<BigEndian>()?;
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
            let value = HeaderValue::read(&mut cursor, typenum, count)?;
            value_map.insert(tag, value);
        }
        Ok(HeaderTable { values: value_map })
    }

    /// Returns the map of all values.
    pub fn map(&self) -> &BTreeMap<u32, HeaderValue> { &self.values }

    /// Returns the value for the given tag, if any.
    pub fn get(&self, tag: u32) -> Option<&HeaderValue> {
        self.values.get(&tag)
    }
}

// ========================================================================= //

/// A value stored in a header table.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HeaderValue {
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

impl HeaderValue {
    fn read<R: Read>(reader: &mut R, typenum: u32, count: u32)
                     -> io::Result<HeaderValue> {
        match typenum {
            0 => Ok(HeaderValue::Null),
            1 => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(HeaderValue::Char(buffer))
            }
            2 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i8()?);
                }
                Ok(HeaderValue::Int8(array))
            }
            3 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i16::<BigEndian>()?);
                }
                Ok(HeaderValue::Int16(array))
            }
            4 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i32::<BigEndian>()?);
                }
                Ok(HeaderValue::Int32(array))
            }
            5 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(reader.read_i64::<BigEndian>()?);
                }
                Ok(HeaderValue::Int64(array))
            }
            6 => {
                if count != 1 {
                    invalid_data!("Invalid count in header index for type \
                                   STRING (was {}, but must be 1)",
                                  count);
                }
                let string = read_nul_terminated_string(reader)?;
                Ok(HeaderValue::String(string))
            }
            7 => {
                let mut buffer = vec![0u8; count as usize];
                reader.read_exact(&mut buffer)?;
                Ok(HeaderValue::Binary(buffer))
            }
            8 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(HeaderValue::StringArray(array))
            }
            9 => {
                let mut array = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    array.push(read_nul_terminated_string(reader)?);
                }
                Ok(HeaderValue::I18nString(array))
            }
            _ => invalid_data!("Invalid type in header index ({})", typenum),
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
