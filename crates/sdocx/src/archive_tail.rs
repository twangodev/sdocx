use std::io::{self, Read, Seek, SeekFrom};

use crate::end_tag::END_TAG_SIGNATURE;

const ZIP_END_RECORD_SIZE: usize = 22;
const MAX_TAIL_SIZE: usize = ZIP_END_RECORD_SIZE + u16::MAX as usize + 2 + u16::MAX as usize;

pub(crate) struct ArchiveTail {
    pub archive_length: u64,
    pub end_tag: Option<Vec<u8>>,
}

impl ArchiveTail {
    pub(crate) fn read(reader: &mut (impl Read + Seek)) -> io::Result<Self> {
        let file_length = reader.seek(SeekFrom::End(0))?;
        let tail_length = file_length.min(MAX_TAIL_SIZE as u64) as usize;
        let tail_start = file_length - tail_length as u64;
        reader.seek(SeekFrom::Start(tail_start))?;
        let mut tail = vec![0; tail_length];
        reader.read_exact(&mut tail)?;
        reader.seek(SeekFrom::Start(0))?;

        for offset in 0..tail.len().saturating_sub(ZIP_END_RECORD_SIZE - 1) {
            let footer = &tail[offset..offset + ZIP_END_RECORD_SIZE];
            if &footer[..4] != b"PK\x05\x06"
                || footer[4..8] != [0; 4]
                || footer[8..10] != footer[10..12]
            {
                continue;
            }
            let directory_size = u32::from_le_bytes(footer[12..16].try_into().unwrap());
            let directory_offset = u32::from_le_bytes(footer[16..20].try_into().unwrap());
            if directory_size != u32::MAX
                && directory_offset != u32::MAX
                && u64::from(directory_size) + u64::from(directory_offset)
                    > tail_start + offset as u64
            {
                continue;
            }
            let comment_length = usize::from(u16::from_le_bytes([footer[20], footer[21]]));
            let tag_start = offset + ZIP_END_RECORD_SIZE + comment_length;
            let Some(tag) = tail.get(tag_start..) else {
                continue;
            };
            if tag.is_empty() {
                return Ok(Self {
                    archive_length: file_length,
                    end_tag: None,
                });
            }
            if tag.len() < 2 + END_TAG_SIGNATURE.len() {
                continue;
            }
            let declared_size = usize::from(u16::from_le_bytes([tag[0], tag[1]]));
            if declared_size + 2 != tag.len() && !tag.ends_with(END_TAG_SIGNATURE) {
                continue;
            }
            return Ok(Self {
                archive_length: tail_start + tag_start as u64,
                end_tag: Some(tag.to_vec()),
            });
        }
        Ok(Self {
            archive_length: file_length,
            end_tag: None,
        })
    }
}

pub(crate) struct ArchiveReader<R> {
    reader: R,
    length: u64,
    position: u64,
}

impl<R: Read + Seek> ArchiveReader<R> {
    pub(crate) fn new(mut reader: R, length: u64) -> io::Result<Self> {
        reader.seek(SeekFrom::Start(0))?;
        Ok(Self {
            reader,
            length,
            position: 0,
        })
    }
}

impl<R: Read> Read for ArchiveReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = (self.length.saturating_sub(self.position)).min(buffer.len() as u64) as usize;
        let read = self.reader.read(&mut buffer[..length])?;
        self.position += read as u64;
        Ok(read)
    }
}

impl<R: Seek> Seek for ArchiveReader<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::End(offset) => self.length.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.position.checked_add_signed(offset),
        }
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "archive seek is out of range")
        })?;
        self.position = self.reader.seek(SeekFrom::Start(position))?;
        Ok(self.position)
    }
}
