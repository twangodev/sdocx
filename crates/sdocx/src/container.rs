use std::io::{Read, Seek};

use crate::error::{Error, Result};
use crate::page::parse_page;
use crate::types::{
    BoundingBox, Color, Document, DocumentMetadata, MediaAsset, Page, RichTextBox, RichTextRun,
};

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

    let mut note_text = None;

    // Parse note.note (optional)
    if let Ok(mut entry) = archive.by_name("note.note") {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        parse_note_note(&buf, &mut metadata);
        note_text = parse_note_text(&buf);
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

    metadata.media_assets = parse_media_assets(&mut archive)?;

    let mut pages: Vec<Page> = Vec::with_capacity(page_names.len());
    for name in &page_names {
        let mut entry = archive.by_name(name)?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        let page = parse_page(&buf)?;
        pages.push(page);
    }

    if let (Some(page), Some(text)) = (pages.first_mut(), note_text.clone()) {
        page.elements.push(crate::types::PageElement::TextBox(text));
    }
    metadata.note_text = note_text;

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
    if data.len() >= 0x08 {
        let flags = u32::from_le_bytes(data[0x04..0x08].try_into().unwrap());
        metadata.dark_mode_compatibility = Some(flags & 0x0800 != 0);
    }

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

fn parse_media_assets<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<Vec<MediaAsset>> {
    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let entry = archive.by_index(i).ok()?;
            let name = entry.name().to_string();
            let lower = name.to_ascii_lowercase();
            if name.starts_with("media/")
                && (lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".png")
                    || lower.ends_with(".webp"))
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort_by_key(|name| {
        name.rsplit('/')
            .next()
            .and_then(|file| file.split('@').next())
            .and_then(|prefix| prefix.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    });

    let mut assets = Vec::with_capacity(names.len());
    for name in names {
        let mut entry = archive.by_name(&name)?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data)?;
        let lower = name.to_ascii_lowercase();
        let mime_type = if lower.ends_with(".png") {
            "image/png"
        } else if lower.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        assets.push(MediaAsset {
            name,
            mime_type: mime_type.to_string(),
            data,
        });
    }
    Ok(assets)
}

fn parse_note_text(data: &[u8]) -> Option<RichTextBox> {
    let (text, text_end) = first_utf16_text(data)?;
    let styles = &data[text_end..];
    let color = tlv_color(styles, 0x01);
    let font_size = tlv_f32(styles, 0x03);
    let runs = parse_rich_text_runs(styles, text.chars().count());
    Some(RichTextBox {
        // `note.note` stores the typed note body as the default page text layer. The body itself
        // carries leading blank lines, so the renderer can place it from the page origin.
        bbox: BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.0,
            y_max: 0.0,
        },
        rotation_degrees: None,
        text,
        color,
        highlight_color: None,
        underline: false,
        font_size,
        runs,
    })
}

fn first_utf16_text(data: &[u8]) -> Option<(String, usize)> {
    let mut offset = 0;
    while offset + 6 <= data.len() {
        let mut end = offset;
        let mut units = Vec::new();
        while end + 2 <= data.len() {
            let unit = u16::from_le_bytes(data[end..end + 2].try_into().ok()?);
            let printable = unit == 0x0A || (0x20..=0xD7FF).contains(&unit);
            if !printable {
                break;
            }
            units.push(unit);
            end += 2;
        }
        let text = String::from_utf16(&units).ok()?;
        let trimmed = text.trim();
        if trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
            && looks_like_note_text(trimmed)
        {
            return Some((text, end));
        }
        offset += 2;
    }
    None
}

fn looks_like_note_text(text: &str) -> bool {
    let mut total = 0;
    let mut common = 0;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if ch.is_ascii_alphanumeric() || ch.is_ascii_punctuation() {
            common += 1;
        }
    }
    total >= 3 && common * 4 >= total * 3
}

fn tlv_color(data: &[u8], tag: u16) -> Option<Color> {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(22) {
        if data[offset..offset + 4] == marker && data[offset + 21] == 0xFF {
            return Some(Color {
                r: data[offset + 20],
                g: data[offset + 19],
                b: data[offset + 18],
            });
        }
    }
    None
}

fn tlv_f32(data: &[u8], tag: u16) -> Option<f32> {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(24) {
        if data[offset..offset + 4] == marker {
            for value_offset in [18, 20, 24] {
                let value = f32::from_le_bytes(
                    data[offset + value_offset..offset + value_offset + 4]
                        .try_into()
                        .ok()?,
                );
                if value.is_finite() && (4.0..=96.0).contains(&value) {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn parse_rich_text_runs(data: &[u8], text_len: usize) -> Vec<RichTextRun> {
    let mut runs = Vec::new();
    collect_style_runs(data, text_len, 0x05, true, false, &mut runs);
    collect_style_runs(data, text_len, 0x06, false, true, &mut runs);
    runs
}

fn collect_style_runs(
    data: &[u8],
    text_len: usize,
    tag: u16,
    bold: bool,
    italic: bool,
    runs: &mut Vec<RichTextRun>,
) {
    let marker = [0x18, 0x00, tag as u8, (tag >> 8) as u8];
    for offset in 0..data.len().saturating_sub(22) {
        if data[offset..offset + 4] != marker {
            continue;
        }
        let start = u32::from_le_bytes(data[offset + 6..offset + 10].try_into().unwrap()) as usize;
        let end = u32::from_le_bytes(data[offset + 10..offset + 14].try_into().unwrap()) as usize;
        let enabled = u32::from_le_bytes(data[offset + 18..offset + 22].try_into().unwrap()) != 0;
        if enabled && start < end && end <= text_len {
            runs.push(RichTextRun {
                start,
                end,
                bold,
                italic,
            });
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

#[cfg(test)]
mod tests {
    use super::parse_note_note;
    use crate::types::DocumentMetadata;

    #[test]
    fn parses_dark_mode_compatibility_flag() {
        let mut metadata = DocumentMetadata::default();
        let mut data = vec![0; 0x30];
        data[0x04..0x08].copy_from_slice(&0x0804_u32.to_le_bytes());

        parse_note_note(&data, &mut metadata);

        assert_eq!(metadata.dark_mode_compatibility, Some(true));

        data[0x04..0x08].copy_from_slice(&0x0004_u32.to_le_bytes());
        parse_note_note(&data, &mut metadata);

        assert_eq!(metadata.dark_mode_compatibility, Some(false));
    }
}
