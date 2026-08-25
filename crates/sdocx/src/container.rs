use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};
use crate::note::parse_note_bytes_with_limits;
use crate::page::parse_page;
use crate::report::{DiagnosticCode, ParseReport};
use crate::storage::{
    ParsedDocument, StoredArchivePage, StoredObject, parse_page_manifest_bytes_with_limits,
    parse_stored_page_bytes_with_limits,
};
use crate::types::{
    Color, Document, DocumentMetadata, FormatVersion, MediaAsset, ObjectType, Page,
};
use crate::{ParseLimits, ParseOptions};

const PROTECTED_DOCUMENT_MARKER: &[u8] = b"Document for S-Pen SDK";

/// Parse a `.sdocx` ZIP archive from a reader.
pub fn parse_from_reader<R: Read + Seek>(reader: R, options: &ParseOptions) -> Result<Document> {
    parse_detailed_from_reader(reader, options).map(ParsedDocument::into_document)
}

/// Parse a `.sdocx` archive while retaining its physical page structure.
pub fn parse_detailed_from_reader<R: Read + Seek>(
    mut reader: R,
    options: &ParseOptions,
) -> Result<ParsedDocument> {
    if is_protected_document(&mut reader)? {
        return Err(Error::ProtectedDocument);
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut archive = zip::ZipArchive::new(reader)?;
    validate_archive(&mut archive, &options.limits)?;

    let mut metadata = DocumentMetadata::default();
    let mut report = ParseReport::default();

    // Parse end_tag.bin (optional — graceful degradation)
    if let Some(buf) = read_optional_entry(&mut archive, "end_tag.bin", &options.limits)? {
        parse_end_tag(&buf, &mut metadata);
    }

    let mut note = None;
    let mut note_text = None;

    // Parse note.note (optional)
    if let Some(buf) = read_optional_entry(&mut archive, "note.note", &options.limits)? {
        parse_note_note(&buf, &mut metadata);
        let parsed_note = parse_note_bytes_with_limits(&buf, &options.limits)?;
        metadata.note_title = Some(parsed_note.title.clone());
        note_text = Some(parsed_note.body.clone());
        note = Some(parsed_note);
    }

    // Parse pageIdInfo.dat (optional)
    let page_manifest =
        if let Some(buf) = read_optional_entry(&mut archive, "pageIdInfo.dat", &options.limits)? {
            let manifest = parse_page_manifest_bytes_with_limits(&buf, &options.limits)?;
            metadata.page_ids = manifest
                .entries
                .iter()
                .map(|entry| entry.page_id.clone())
                .collect();
            Some(manifest)
        } else {
            report.warning(
                DiagnosticCode::MissingPageManifest,
                None,
                "pageIdInfo.dat is absent; using sorted archive entry names",
            );
            None
        };

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
    page_names.sort();

    metadata.media_assets = parse_media_assets(&mut archive, &options.limits)?;

    let mut page_records = Vec::with_capacity(page_names.len());
    for name in &page_names {
        let buf = read_required_entry(&mut archive, name, &options.limits)?;
        let page = parse_page(&buf, &options.limits)?;
        let stored_page = parse_stored_page_bytes_with_limits(&buf, &options.limits)?;

        let file_id = Path::new(name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name);
        if file_id != stored_page.header.uuid {
            report.warning(
                DiagnosticCode::PageIdentifierMismatch,
                Some(name.clone()),
                format!(
                    "archive filename identifies page {file_id}, but its header identifies {}",
                    stored_page.header.uuid
                ),
            );
        }
        if page.uuid != stored_page.header.uuid {
            return Err(Error::Format(format!(
                "{name}: semantic and physical page parsers disagree on the embedded UUID"
            )));
        }
        report_unknown_object_types(&stored_page.layers.layers, name, &mut report);

        page_records.push(ParsedPageRecord {
            semantic: page,
            stored: StoredArchivePage {
                archive_entry: name.clone(),
                page: stored_page,
            },
        });
    }
    page_records = order_page_records(page_records, &metadata.page_ids, &mut report);
    let (mut pages, stored_pages): (Vec<_>, Vec<_>) = page_records
        .into_iter()
        .map(|record| (record.semantic, record.stored))
        .unzip();

    if let (Some(page), Some(text)) = (pages.first_mut(), note_text.clone()) {
        page.elements.push(crate::types::PageElement::TextBox(text));
    }
    metadata.note_text = note_text;

    Ok(ParsedDocument {
        document: Document { pages, metadata },
        stored_pages,
        page_manifest,
        note,
        report,
    })
}

struct ParsedPageRecord {
    semantic: Page,
    stored: StoredArchivePage,
}

fn order_page_records(
    records: Vec<ParsedPageRecord>,
    page_ids: &[String],
    report: &mut ParseReport,
) -> Vec<ParsedPageRecord> {
    let mut indexes_by_id: HashMap<String, VecDeque<usize>> = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        indexes_by_id
            .entry(record.stored.page.header.uuid.clone())
            .or_default()
            .push_back(index);
    }

    let mut selected = vec![false; records.len()];
    let mut ordered_indexes = Vec::with_capacity(records.len());
    for page_id in page_ids {
        if let Some(index) = indexes_by_id.get_mut(page_id).and_then(VecDeque::pop_front) {
            selected[index] = true;
            ordered_indexes.push(index);
        } else {
            report.warning(
                DiagnosticCode::MissingPageEntry,
                Some("pageIdInfo.dat".into()),
                format!("page manifest references missing page {page_id}"),
            );
        }
    }
    for (index, was_selected) in selected.iter().enumerate() {
        if !was_selected {
            report.warning(
                DiagnosticCode::UnlistedPageEntry,
                Some(records[index].stored.archive_entry.clone()),
                format!(
                    "page {} is not listed in pageIdInfo.dat",
                    records[index].stored.page.header.uuid
                ),
            );
            ordered_indexes.push(index);
        }
    }

    let mut records = records.into_iter().map(Some).collect::<Vec<_>>();
    ordered_indexes
        .into_iter()
        .filter_map(|index| records[index].take())
        .collect()
}

fn report_unknown_object_types(
    layers: &[crate::storage::StoredLayer],
    archive_entry: &str,
    report: &mut ParseReport,
) {
    let mut unknown_types = BTreeSet::new();
    for layer in layers {
        collect_unknown_object_types(&layer.objects, &mut unknown_types);
    }
    for raw in unknown_types {
        report.warning(
            DiagnosticCode::UnknownObjectType,
            Some(archive_entry.to_string()),
            format!("retained unknown S Pen object type {raw}"),
        );
    }
}

fn collect_unknown_object_types(objects: &[StoredObject], unknown_types: &mut BTreeSet<u32>) {
    for object in objects {
        if let ObjectType::Other(raw) = object.object_type {
            unknown_types.insert(raw);
        }
        collect_unknown_object_types(&object.children, unknown_types);
    }
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

#[cfg(test)]
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
