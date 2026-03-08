use std::io::{Read, Seek};

use crate::error::{Error, Result};
use crate::page::parse_page;
use crate::types::{Color, Document, DocumentMetadata, Page};

/// Parse a `.sdocx` ZIP archive from a reader.
pub fn parse_from_reader<R: Read + Seek>(reader: R) -> Result<Document> {
    let mut archive = zip::ZipArchive::new(reader)?;

    let mut metadata = DocumentMetadata::default();

    // Parse end_tag.bin (optional — graceful degradation)
    if let Ok(mut entry) = archive.by_name("end_tag.bin") {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        parse_end_tag(&buf, &mut metadata);
    }

    // Parse note.note (optional)
    if let Ok(mut entry) = archive.by_name("note.note") {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        parse_note_note(&buf, &mut metadata);
    }

    // Parse pageIdInfo.dat (optional)
    if let Ok(mut entry) = archive.by_name("pageIdInfo.dat") {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        parse_page_id_info(&buf, &mut metadata);
    }

    // Find and parse all .page files
    let page_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            if name.ends_with(".page") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    if page_names.is_empty() {
        return Err(Error::Format("no .page files found in archive".into()));
    }

    let mut pages: Vec<Page> = Vec::with_capacity(page_names.len());
    for name in &page_names {
        let mut entry = archive.by_name(name)?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        let page = parse_page(&buf)?;
        pages.push(page);
    }

    Ok(Document { pages, metadata })
}

/// Extract timestamps from `end_tag.bin`.
fn parse_end_tag(data: &[u8], metadata: &mut DocumentMetadata) {
    if data.len() < 0x58 {
        return;
    }
    let created = i64::from_le_bytes(data[0x48..0x50].try_into().unwrap());
    let modified = i64::from_le_bytes(data[0x50..0x58].try_into().unwrap());
    metadata.created_ms = Some(created);
    metadata.modified_ms = Some(modified);
}

/// Extract background color and page dimensions from `note.note`.
fn parse_note_note(data: &[u8], metadata: &mut DocumentMetadata) {
    // Page dimensions at 0x28, 0x2C
    if data.len() >= 0x30 {
        let w = u32::from_le_bytes(data[0x28..0x2C].try_into().unwrap());
        let h = u32::from_le_bytes(data[0x2C..0x30].try_into().unwrap());
        if w > 0 && h > 0 {
            metadata.page_dimensions = Some((w, h));
        }
    }

    // Background color: pattern [18 00] [00 00 01 00 00 00] [R] [G] [B] [FF]
    if data.len() >= 12 {
        for i in 0..data.len() - 12 {
            if data[i] == 0x18
                && data[i + 1] == 0x00
                && data[i + 2..i + 8] == [0x00, 0x00, 0x01, 0x00, 0x00, 0x00]
                && data[i + 11] == 0xFF
            {
                metadata.background_color = Some(Color {
                    r: data[i + 8],
                    g: data[i + 9],
                    b: data[i + 10],
                });
                break;
            }
        }
    }
}

/// Extract page UUIDs from `pageIdInfo.dat`.
fn parse_page_id_info(data: &[u8], metadata: &mut DocumentMetadata) {
    if data.len() < 0x24 {
        return;
    }
    let count = u16::from_le_bytes(data[0x20..0x22].try_into().unwrap()) as usize;
    let mut offset = 0x22;

    for _ in 0..count {
        if offset + 2 > data.len() {
            break;
        }
        let char_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if offset + char_len * 2 > data.len() {
            break;
        }
        let uuid: String = data[offset..offset + char_len * 2]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .map(|c| char::from_u32(c as u32).unwrap_or('\u{FFFD}'))
            .collect();
        metadata.page_ids.push(uuid);
        offset += char_len * 2;
    }
}
