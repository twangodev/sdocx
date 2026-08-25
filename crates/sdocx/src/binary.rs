use crate::error::{Error, Result};

/// Bounds-checked reader for Samsung's little-endian binary records.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    position: usize,
    context: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], context: &'static str) -> Self {
        Self {
            data,
            position: 0,
            context,
        }
    }

    pub(crate) fn at(data: &'a [u8], position: usize, context: &'static str) -> Result<Self> {
        if position > data.len() {
            return Err(Error::Format(format!(
                "{context}: offset 0x{position:x} is past the end of the record"
            )));
        }
        Ok(Self {
            data,
            position,
            context,
        })
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    pub(crate) fn read_u8(&mut self, field: &'static str) -> Result<u8> {
        Ok(self.read_array::<1>(field)?[0])
    }

    pub(crate) fn read_u16(&mut self, field: &'static str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_u32(&mut self, field: &'static str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_u64(&mut self, field: &'static str) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array(field)?))
    }

    pub(crate) fn read_bytes(&mut self, length: usize, field: &'static str) -> Result<&'a [u8]> {
        let start = self.position;
        let end = start.checked_add(length).ok_or_else(|| {
            Error::Format(format!(
                "{}: {field} length overflows at offset 0x{start:x}",
                self.context
            ))
        })?;
        let bytes = self.data.get(start..end).ok_or_else(|| {
            Error::Format(format!(
                "{}: truncated {field} at offset 0x{start:x} (need {length} bytes, have {})",
                self.context,
                self.remaining()
            ))
        })?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn skip(&mut self, length: usize, field: &'static str) -> Result<()> {
        self.read_bytes(length, field).map(|_| ())
    }

    pub(crate) fn read_utf16_u16(&mut self, field: &'static str) -> Result<String> {
        let unit_count = usize::from(self.read_u16(field)?);
        if unit_count == usize::from(u16::MAX) {
            return Err(Error::Format(format!(
                "{}: {field} uses the null string sentinel",
                self.context
            )));
        }
        let byte_count = unit_count
            .checked_mul(2)
            .ok_or_else(|| Error::Format(format!("{}: {field} length overflows", self.context)))?;
        let bytes = self.read_bytes(byte_count, field)?;
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>();
        String::from_utf16(&units)
            .map_err(|_| Error::Format(format!("{}: invalid UTF-16 in {field}", self.context)))
    }

    fn read_array<const N: usize>(&mut self, field: &'static str) -> Result<[u8; N]> {
        self.read_bytes(N, field)?
            .try_into()
            .map_err(|_| Error::Format(format!("{}: invalid {field}", self.context)))
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn reports_the_field_and_offset_for_truncated_data() {
        let mut reader = Reader::new(&[0x01, 0x02], "test record");

        let error = reader.read_u32("declared size").unwrap_err();

        assert_eq!(
            error.to_string(),
            "format error: test record: truncated declared size at offset 0x0 (need 4 bytes, have 2)"
        );
    }

    #[test]
    fn decodes_length_prefixed_utf16() {
        let mut reader = Reader::new(&[2, 0, b'A', 0, 0x3d, 0xd8], "test record");

        let error = reader.read_utf16_u16("name").unwrap_err();

        assert_eq!(
            error.to_string(),
            "format error: test record: invalid UTF-16 in name"
        );
    }
}
