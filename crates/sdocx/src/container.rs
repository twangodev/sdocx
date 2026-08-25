use std::collections::{HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom};

use crate::error::{Error, Result};
use crate::page::parse_page;
use crate::types::{
    BoundingBox, Color, Document, DocumentMetadata, FormatVersion, MediaAsset, Page, RichTextBox,
    RichTextRun,
};
use crate::{ParseLimits, ParseOptions};

const PROTECTED_DOCUMENT_MARKER: &[u8] = b"Document for S-Pen SDK";

/// Parse a `.sdocx` ZIP archive from a reader.
pub fn parse_from_reader<R: Read + Seek>(
    mut reader: R,
    options: &ParseOptions,
) -> Result<Document> {
    if is_protected_document(&mut reader)? {
        return Err(Error::ProtectedDocument);
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut archive = zip::ZipArchive::new(reader)?;
    validate_archive(&mut archive, &options.limits)?;

    let mut metadata = DocumentMetadata::default();

    // Parse end_tag.bin (optional — graceful degradation)
    if let Some(buf) = read_optional_entry(&mut archive, "end_tag.bin", &options.limits)? {
        parse_end_tag(&buf, &mut metadata);
    }

    let mut note_text = None;

    // Parse note.note (optional)
    if let Some(buf) = read_optional_entry(&mut archive, "note.note", &options.limits)? {
        parse_note_note(&buf, &mut metadata);
        note_text = parse_note_text(&buf);
    }

    // Parse pageIdInfo.dat (optional)
    if let Some(buf) = read_optional_entry(&mut archive, "pageIdInfo.dat", &options.limits)? {
        parse_page_id_info(&buf, &mut metadata, &options.limits)?;
    }

    // Find and parse all .page files
    let mut page_names = Vec::new();
    for index in 0..archive.len() {
        let name = {
            let entry = archive.by_index(index)?;
            let name = entry.name().to_string();
            name.ends_with(".page").then_some(name)
        };
        if let Some(name) = name {
            page_names.push(name);
        }
    }

    if page_names.is_empty() {
        return Err(Error::Format("no .page files found in archive".into()));
    }
    check_limit("page count", options.limits.max_pages, page_names.len())?;

    metadata.media_assets = parse_media_assets(&mut archive, &options.limits)?;

    let mut pages: Vec<Page> = Vec::with_capacity(page_names.len());
    for name in &page_names {
        let buf = read_required_entry(&mut archive, name, &options.limits)?;
        let page = parse_page(&buf, &options.limits)?;
        pages.push(page);
    }
    pages = order_pages(pages, &metadata.page_ids);

    if let (Some(page), Some(text)) = (pages.first_mut(), note_text.clone()) {
        page.elements.push(crate::types::PageElement::TextBox(text));
    }
    metadata.note_text = note_text;

    Ok(Document { pages, metadata })
}

fn validate_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    limits: &ParseLimits,
) -> Result<()> {
    check_limit(
        "archive entry count",
        limits.max_archive_entries,
        archive.len(),
    )?;

    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        check_u64_limit("archive entry size", limits.max_entry_size, entry.size())?;
        total_size = total_size
            .checked_add(entry.size())
            .ok_or(Error::LimitExceeded {
                resource: "total uncompressed size",
                limit: limits.max_total_uncompressed_size,
                actual: u64::MAX,
            })?;
        check_u64_limit(
            "total uncompressed size",
            limits.max_total_uncompressed_size,
            total_size,
        )?;
    }
    Ok(())
}

fn read_optional_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limits: &ParseLimits,
) -> Result<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(entry) => read_zip_entry(entry, limits).map(Some),
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn read_required_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limits: &ParseLimits,
) -> Result<Vec<u8>> {
    let entry = archive.by_name(name)?;
    read_zip_entry(entry, limits)
}

fn read_zip_entry<R: Read>(
    entry: zip::read::ZipFile<'_, R>,
    limits: &ParseLimits,
) -> Result<Vec<u8>> {
    check_u64_limit("archive entry size", limits.max_entry_size, entry.size())?;
    let mut data = Vec::new();
    entry
        .take(limits.max_entry_size.saturating_add(1))
        .read_to_end(&mut data)?;
    check_u64_limit(
        "archive entry size",
        limits.max_entry_size,
        u64::try_from(data.len()).unwrap_or(u64::MAX),
    )?;
    Ok(data)
}

fn is_protected_document<R: Read + Seek>(reader: &mut R) -> std::io::Result<bool> {
    let len = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    let mut magic = [0_u8; 4];
    let magic_len = reader.read(&mut magic)?;
    if magic_len >= 2 && &magic[..2] == b"PK" {
        reader.seek(SeekFrom::Start(0))?;
        return Ok(false);
    }

    let tail_len = len.min(4096);
    reader.seek(SeekFrom::Start(len.saturating_sub(tail_len)))?;
    let mut tail = Vec::new();
    reader.take(tail_len).read_to_end(&mut tail)?;
    reader.seek(SeekFrom::Start(0))?;

    Ok(tail
        .windows(PROTECTED_DOCUMENT_MARKER.len())
        .any(|window| window == PROTECTED_DOCUMENT_MARKER))
}

fn check_limit(resource: &'static str, limit: usize, actual: usize) -> Result<()> {
    check_u64_limit(
        resource,
        u64::try_from(limit).unwrap_or(u64::MAX),
        u64::try_from(actual).unwrap_or(u64::MAX),
    )
}

fn check_u64_limit(resource: &'static str, limit: u64, actual: u64) -> Result<()> {
    if actual > limit {
        Err(Error::LimitExceeded {
            resource,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

fn order_pages(pages: Vec<Page>, page_ids: &[String]) -> Vec<Page> {
    let mut indexes_by_id: HashMap<String, VecDeque<usize>> = HashMap::new();
    for (index, page) in pages.iter().enumerate() {
        indexes_by_id
            .entry(page.uuid.clone())
            .or_default()
            .push_back(index);
    }

    let mut selected = vec![false; pages.len()];
    let mut ordered_indexes = Vec::with_capacity(pages.len());
    for page_id in page_ids {
        if let Some(index) = indexes_by_id.get_mut(page_id).and_then(VecDeque::pop_front) {
            selected[index] = true;
            ordered_indexes.push(index);
        }
    }
    ordered_indexes.extend(
        selected
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| (!selected).then_some(index)),
    );

    let mut pages: Vec<Option<Page>> = pages.into_iter().map(Some).collect();
    ordered_indexes
        .into_iter()
        .filter_map(|index| pages[index].take())
        .collect()
}

/// Extract timestamps from `end_tag.bin`.
fn parse_end_tag(data: &[u8], metadata: &mut DocumentMetadata) {
    if let Some(version) = data
        .get(0x02..0x04)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .filter(|version| *version != 0)
    {
        metadata.format_version = Some(FormatVersion(version));
    }
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
    if metadata.format_version.is_none() {
        metadata.format_version = data
            .get(0x0E..0x10)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u16::from_le_bytes)
            .filter(|version| *version != 0)
            .map(FormatVersion);
    }
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

fn parse_media_assets<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    limits: &ParseLimits,
) -> Result<Vec<MediaAsset>> {
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let name = {
            let entry = archive.by_index(index)?;
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
        };
        if let Some(name) = name {
            names.push(name);
        }
    }
    names.sort_by_key(|name| media_archive_id(name).map(u64::from).unwrap_or(u64::MAX));

    let mut assets = Vec::with_capacity(names.len());
    for name in names {
        let data = read_required_entry(archive, &name, limits)?;
        let lower = name.to_ascii_lowercase();
        let mime_type = if lower.ends_with(".png") {
            "image/png"
        } else if lower.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        assets.push(MediaAsset {
            archive_id: media_archive_id(&name),
            name,
            mime_type: mime_type.to_string(),
            data,
        });
    }
    Ok(assets)
}

fn media_archive_id(name: &str) -> Option<u32> {
    name.rsplit('/').next()?.split('@').next()?.parse().ok()
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
fn parse_page_id_info(
    data: &[u8],
    metadata: &mut DocumentMetadata,
    limits: &ParseLimits,
) -> Result<()> {
    if data.len() < 0x22 {
        return Ok(());
    }
    let count = u16::from_le_bytes(data[0x20..0x22].try_into().unwrap()) as usize;
    check_limit("page count", limits.max_pages, count)?;
    let mut offset: usize = 0x22;

    for _ in 0..count {
        let Some(length_end) = offset.checked_add(2) else {
            break;
        };
        let Some(length_bytes) = data.get(offset..length_end) else {
            break;
        };
        let char_len = u16::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
        offset = length_end;
        let Some(byte_len) = char_len.checked_mul(2) else {
            break;
        };
        let Some(uuid_end) = offset.checked_add(byte_len) else {
            break;
        };
        let Some(uuid_bytes) = data.get(offset..uuid_end) else {
            break;
        };
        let uuid: String = uuid_bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .map(|c| char::from_u32(c as u32).unwrap_or('\u{FFFD}'))
            .collect();
        metadata.page_ids.push(uuid);
        offset = uuid_end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{order_pages, parse_end_tag, parse_from_reader, parse_note_note};
    use crate::types::{BoundingBox, DocumentMetadata, FormatVersion, Page};
    use crate::{Error, ParseOptions};

    fn page(uuid: &str) -> Page {
        Page {
            uuid: uuid.to_string(),
            width: 1,
            height: 1,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            strokes: Vec::new(),
            elements: Vec::new(),
        }
    }

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

    #[test]
    fn parses_format_version_with_note_fallback() {
        let mut metadata = DocumentMetadata::default();
        let mut note = vec![0; 0x10];
        note[0x0E..0x10].copy_from_slice(&4000_u16.to_le_bytes());
        parse_note_note(&note, &mut metadata);
        assert_eq!(metadata.format_version, Some(FormatVersion(4000)));

        let mut end_tag = vec![0; 4];
        end_tag[0x02..0x04].copy_from_slice(&5500_u16.to_le_bytes());
        parse_end_tag(&end_tag, &mut metadata);
        assert_eq!(metadata.format_version, Some(FormatVersion::CURRENT));
    }

    #[test]
    fn orders_pages_by_page_id_info_then_preserves_unlisted_order() {
        let pages = vec![page("second"), page("unlisted"), page("first")];
        let ids = vec!["first".to_string(), "second".to_string()];

        let ordered = order_pages(pages, &ids);

        assert_eq!(
            ordered
                .iter()
                .map(|page| page.uuid.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second", "unlisted"]
        );
    }

    #[test]
    fn reports_protected_document_marker() {
        let bytes = b"encrypted payload Document for S-Pen SDK";
        let error = parse_from_reader(Cursor::new(bytes), &ParseOptions::default()).unwrap_err();

        assert!(matches!(error, Error::ProtectedDocument));
    }
}
