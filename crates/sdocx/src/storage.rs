use crate::ParseLimits;
use crate::binary::Reader;
use crate::error::{Error, Result};
use crate::note::StoredNote;
use crate::report::ParseReport;
use crate::types::{Document, ObjectType};

const LAYER_HEADER_MIN_SIZE: usize = 16;
const LAYER_HEADER_MAX_SIZE: usize = 16 * 1024;
const INTEGRITY_TRAILER_SIZE: usize = 32;

/// The physical representation of one `.page` archive entry.
///
/// Samsung Notes stores document-level flowing text separately in `note.note`,
/// so a stored page is not necessarily the same thing as a visible page.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredPage {
    /// Fixed page header.
    pub header: StoredPageHeader,
    /// Layer collection beginning at `header.raw_layer_offset`.
    pub layers: StoredPageLayers,
}

/// A parsed document together with its physical archive structure.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParsedDocument {
    /// Stable high-level document model used by existing callers.
    pub document: Document,
    /// Physical `.page` records in the same order as `document.pages`.
    pub stored_pages: Vec<StoredArchivePage>,
    /// Authoritative page ordering metadata, when present in the archive.
    pub page_manifest: Option<PageManifest>,
    /// Structured `note.note` title/body and header, when present.
    pub note: Option<StoredNote>,
    /// Non-fatal compatibility findings.
    pub report: ParseReport,
}

impl ParsedDocument {
    /// Discard the physical representation and return the high-level model.
    pub fn into_document(self) -> Document {
        self.document
    }
}

/// One physical `.page` record and its ZIP entry name.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredArchivePage {
    /// ZIP entry name containing this page.
    pub archive_entry: String,
    /// Parsed physical page structure.
    pub page: StoredPage,
}

/// Parsed contents of `pageIdInfo.dat`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageManifest {
    /// Integrity bytes at the beginning of `pageIdInfo.dat`.
    pub integrity_header: [u8; INTEGRITY_TRAILER_SIZE],
    /// Pages in authoritative Samsung Notes order.
    pub entries: Vec<PageManifestEntry>,
    /// Uninterpreted extension bytes from a newer writer, if any.
    pub trailing_data: Vec<u8>,
}

/// One ordered page entry from `pageIdInfo.dat`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageManifestEntry {
    /// Page UUID.
    pub page_id: String,
    /// Integrity bytes recorded for the corresponding `.page` entry.
    pub integrity_hash: [u8; INTEGRITY_TRAILER_SIZE],
}

/// Fixed header fields at the beginning of a `.page` entry.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredPageHeader {
    /// Absolute offset of the serialized layer collection.
    pub raw_layer_offset: u32,
    /// Absolute offset of the page-property block.
    pub property_offset: u32,
    /// Undocumented byte at offset `0x08`.
    pub unknown_08: u8,
    /// Raw text-only page flag.
    pub text_only_flag: u32,
    /// Undocumented byte following `text_only_flag`.
    pub unknown_0d: u8,
    /// Bit mask describing fields in the page-property block.
    pub property_mask: u32,
    /// Raw Samsung Notes orientation value.
    pub orientation: u32,
    /// Stored page width.
    pub width: u32,
    /// Stored page height.
    pub height: u32,
    /// Stored horizontal page offset.
    pub offset_x: u32,
    /// Stored vertical page offset.
    pub offset_y: u32,
    /// Page identifier embedded in the page record.
    pub uuid: String,
    /// Raw Samsung timestamp, when present in the header.
    pub modified_time_raw: Option<u64>,
    /// Page format version, when present in the header.
    pub format_version: Option<u32>,
    /// Minimum reader format version, when present in the header.
    pub minimum_format_version: Option<u32>,
}

/// The layer collection stored in one physical page.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredPageLayers {
    /// Index of the active layer.
    pub current_layer_index: u16,
    /// Layers in serialized order.
    pub layers: Vec<StoredLayer>,
}

/// One Samsung Notes page layer.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredLayer {
    /// Absolute metadata offset recorded by Samsung Notes.
    pub metadata_offset: u32,
    /// First raw layer flag byte.
    pub flags_1: u8,
    /// Second raw layer flag byte.
    pub flags_2: u8,
    /// Layer number used by Samsung Notes.
    pub number: u32,
    /// Undocumented bytes following the known 16-byte layer header.
    pub header_extra: Vec<u8>,
    /// Top-level objects in serialized order.
    pub objects: Vec<StoredObject>,
    /// Integrity trailer following the layer's object tree.
    pub integrity_trailer: [u8; INTEGRITY_TRAILER_SIZE],
}

/// One object record in a stored layer, including its nested child records.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredObject {
    /// S Pen SDK object type.
    pub object_type: ObjectType,
    /// Declared object size, including the 32-byte integrity trailer.
    pub declared_size: u32,
    /// Byte offset of the payload within the uncompressed `.page` entry.
    pub payload_offset: usize,
    /// Number of payload bytes before the integrity trailer.
    pub payload_size: usize,
    /// Integrity trailer belonging to this object.
    pub integrity_trailer: [u8; INTEGRITY_TRAILER_SIZE],
    /// Child objects serialized immediately after this record.
    pub children: Vec<StoredObject>,
}

impl StoredObject {
    /// Borrow this object's payload from its original uncompressed `.page` bytes.
    pub fn payload<'a>(&self, page_bytes: &'a [u8]) -> Option<&'a [u8]> {
        let end = self.payload_offset.checked_add(self.payload_size)?;
        page_bytes.get(self.payload_offset..end)
    }
}

/// Parse the physical structure of one uncompressed `.page` archive entry.
pub fn parse_stored_page_bytes(data: &[u8]) -> Result<StoredPage> {
    parse_stored_page_bytes_with_limits(data, &ParseLimits::default())
}

/// Parse one uncompressed `.page` entry with explicit resource limits.
pub fn parse_stored_page_bytes_with_limits(
    data: &[u8],
    limits: &ParseLimits,
) -> Result<StoredPage> {
    let mut reader = Reader::new(data, "page header");
    let raw_layer_offset = reader.read_u32("raw layer offset")?;
    let property_offset = reader.read_u32("property offset")?;
    let unknown_08 = reader.read_u8("unknown header byte 0x08")?;
    let text_only_flag = reader.read_u32("text-only flag")?;
    let unknown_0d = reader.read_u8("unknown header byte 0x0d")?;
    let property_mask = reader.read_u32("page property mask")?;
    let orientation = reader.read_u32("page orientation")?;
    let width = reader.read_u32("page width")?;
    let height = reader.read_u32("page height")?;
    let offset_x = reader.read_u32("page x offset")?;
    let offset_y = reader.read_u32("page y offset")?;
    let uuid = reader.read_utf16_u16("page UUID")?;

    let header_end = [raw_layer_offset, property_offset]
        .into_iter()
        .filter(|offset| *offset != 0)
        .map(|offset| usize::try_from(offset).unwrap_or(usize::MAX))
        .min()
        .unwrap_or(data.len())
        .min(data.len());
    let modified_time_raw = read_optional_u64(&mut reader, header_end, "modified timestamp")?;
    let format_version = read_optional_u32(&mut reader, header_end, "format version")?;
    let minimum_format_version =
        read_optional_u32(&mut reader, header_end, "minimum format version")?;

    let layer_offset = usize::try_from(raw_layer_offset)
        .map_err(|_| Error::Format("page layer offset does not fit in memory".into()))?;
    let layers = parse_layers(data, layer_offset, limits)?;

    Ok(StoredPage {
        header: StoredPageHeader {
            raw_layer_offset,
            property_offset,
            unknown_08,
            text_only_flag,
            unknown_0d,
            property_mask,
            orientation,
            width,
            height,
            offset_x,
            offset_y,
            uuid,
            modified_time_raw,
            format_version,
            minimum_format_version,
        },
        layers,
    })
}

/// Parse `pageIdInfo.dat` using default resource limits.
pub fn parse_page_manifest_bytes(data: &[u8]) -> Result<PageManifest> {
    parse_page_manifest_bytes_with_limits(data, &ParseLimits::default())
}

/// Parse `pageIdInfo.dat` with explicit resource limits.
pub fn parse_page_manifest_bytes_with_limits(
    data: &[u8],
    limits: &ParseLimits,
) -> Result<PageManifest> {
    let mut reader = Reader::new(data, "page manifest");
    let integrity_header = reader
        .read_bytes(INTEGRITY_TRAILER_SIZE, "manifest integrity header")?
        .try_into()
        .map_err(|_| Error::Format("invalid page manifest integrity header".into()))?;
    let entry_count = usize::from(reader.read_u16("manifest page count")?);
    check_limit("page count", limits.max_pages, entry_count)?;

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let page_id = reader.read_utf16_u16("manifest page UUID")?;
        let integrity_hash = reader
            .read_bytes(INTEGRITY_TRAILER_SIZE, "page integrity hash")?
            .try_into()
            .map_err(|_| Error::Format("invalid page integrity hash".into()))?;
        entries.push(PageManifestEntry {
            page_id,
            integrity_hash,
        });
    }
    let trailing_data = reader
        .read_bytes(reader.remaining(), "manifest extension data")?
        .to_vec();

    Ok(PageManifest {
        integrity_header,
        entries,
        trailing_data,
    })
}

fn read_optional_u32(
    reader: &mut Reader<'_>,
    end: usize,
    field: &'static str,
) -> Result<Option<u32>> {
    if reader.position().saturating_add(4) <= end {
        reader.read_u32(field).map(Some)
    } else {
        Ok(None)
    }
}

fn read_optional_u64(
    reader: &mut Reader<'_>,
    end: usize,
    field: &'static str,
) -> Result<Option<u64>> {
    if reader.position().saturating_add(8) <= end {
        reader.read_u64(field).map(Some)
    } else {
        Ok(None)
    }
}

fn parse_layers(data: &[u8], offset: usize, limits: &ParseLimits) -> Result<StoredPageLayers> {
    let mut reader = Reader::at(data, offset, "page layers")?;
    let layer_count = usize::from(reader.read_u16("layer count")?);
    if layer_count == 0 {
        return Err(Error::Format("page layers: layer count is zero".into()));
    }
    check_limit("layers per page", limits.max_layers_per_page, layer_count)?;

    let current_layer_index = reader.read_u16("current layer index")?;
    if usize::from(current_layer_index) >= layer_count {
        return Err(Error::Format(format!(
            "page layers: current layer index {current_layer_index} is outside {layer_count} layers"
        )));
    }

    let mut total_objects = 0_usize;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        layers.push(parse_layer(&mut reader, limits, &mut total_objects)?);
    }

    Ok(StoredPageLayers {
        current_layer_index,
        layers,
    })
}

fn parse_layer(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    total_objects: &mut usize,
) -> Result<StoredLayer> {
    let header_size = usize::try_from(reader.read_u32("layer header size")?)
        .map_err(|_| Error::Format("layer header size does not fit in memory".into()))?;
    if !(LAYER_HEADER_MIN_SIZE..=LAYER_HEADER_MAX_SIZE).contains(&header_size) {
        return Err(Error::Format(format!(
            "page layers: invalid layer header size {header_size}"
        )));
    }

    let metadata_offset = reader.read_u32("layer metadata offset")?;
    reader.skip(1, "unknown layer byte 0x08")?;
    let flags_1 = reader.read_u8("layer flags 1")?;
    reader.skip(1, "unknown layer byte 0x0a")?;
    let flags_2 = reader.read_u8("layer flags 2")?;
    let number = reader.read_u32("layer number")?;
    let header_extra = reader
        .read_bytes(
            header_size - LAYER_HEADER_MIN_SIZE,
            "layer header extension",
        )?
        .to_vec();

    let object_count = usize::try_from(reader.read_u32("layer object count")?)
        .map_err(|_| Error::Format("layer object count does not fit in memory".into()))?;
    check_accumulated_object_limit(total_objects, object_count, limits)?;

    let mut objects = Vec::with_capacity(object_count);
    for _ in 0..object_count {
        objects.push(parse_object(reader, limits, total_objects, 0)?);
    }
    let integrity_trailer = reader
        .read_bytes(INTEGRITY_TRAILER_SIZE, "layer integrity trailer")?
        .try_into()
        .map_err(|_| Error::Format("invalid layer integrity trailer".into()))?;

    Ok(StoredLayer {
        metadata_offset,
        flags_1,
        flags_2,
        number,
        header_extra,
        objects,
        integrity_trailer,
    })
}

fn parse_object(
    reader: &mut Reader<'_>,
    limits: &ParseLimits,
    total_objects: &mut usize,
    depth: usize,
) -> Result<StoredObject> {
    if depth >= limits.max_object_nesting_depth {
        return Err(Error::LimitExceeded {
            resource: "object nesting depth",
            limit: limits.max_object_nesting_depth as u64,
            actual: depth.saturating_add(1) as u64,
        });
    }

    let object_type = ObjectType::from(u32::from(reader.read_u8("object type")?));
    let child_count = usize::from(reader.read_u16("object child count")?);
    let declared_size = reader.read_u32("object size")?;
    let declared_size_usize = usize::try_from(declared_size)
        .map_err(|_| Error::Format("object size does not fit in memory".into()))?;
    if declared_size_usize < INTEGRITY_TRAILER_SIZE {
        return Err(Error::Format(format!(
            "page layers: object size {declared_size} is smaller than its integrity trailer"
        )));
    }

    let payload_offset = reader.position();
    let payload_size = declared_size_usize - INTEGRITY_TRAILER_SIZE;
    reader.skip(payload_size, "object payload")?;
    let integrity_trailer = reader
        .read_bytes(INTEGRITY_TRAILER_SIZE, "object integrity trailer")?
        .try_into()
        .map_err(|_| Error::Format("invalid object integrity trailer".into()))?;

    check_accumulated_object_limit(total_objects, child_count, limits)?;
    let mut children = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        children.push(parse_object(
            reader,
            limits,
            total_objects,
            depth.saturating_add(1),
        )?);
    }

    Ok(StoredObject {
        object_type,
        declared_size,
        payload_offset,
        payload_size,
        integrity_trailer,
        children,
    })
}

fn check_accumulated_object_limit(
    total_objects: &mut usize,
    additional: usize,
    limits: &ParseLimits,
) -> Result<()> {
    *total_objects = total_objects
        .checked_add(additional)
        .ok_or(Error::LimitExceeded {
            resource: "objects per page",
            limit: limits.max_objects_per_page as u64,
            actual: u64::MAX,
        })?;
    check_limit(
        "objects per page",
        limits.max_objects_per_page,
        *total_objects,
    )
}

fn check_limit(resource: &'static str, limit: usize, actual: usize) -> Result<()> {
    if actual > limit {
        Err(Error::LimitExceeded {
            resource,
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_page_manifest_bytes, parse_stored_page_bytes_with_limits};
    use crate::{Error, ObjectType, ParseLimits};

    fn stored_page_with_object_tree() -> Vec<u8> {
        let layer_offset = 0x80_u32;
        let mut data = Vec::new();
        data.extend_from_slice(&layer_offset.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.push(4);
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&1080_u32.to_le_bytes());
        data.extend_from_slice(&1527_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&4_u16.to_le_bytes());
        for unit in "page".encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        data.extend_from_slice(&123_u64.to_le_bytes());
        data.extend_from_slice(&5500_u32.to_le_bytes());
        data.extend_from_slice(&4000_u32.to_le_bytes());
        data.resize(layer_offset as usize, 0);

        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&16_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&[0, 1, 0, 2]);
        data.extend_from_slice(&7_u32.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());

        data.push(ObjectType::Container.raw() as u8);
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&32_u32.to_le_bytes());
        data.extend_from_slice(&[0xaa; 32]);

        data.push(ObjectType::TextBox.raw() as u8);
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&36_u32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend_from_slice(&[0xbb; 32]);
        data.extend_from_slice(&[0xcc; 32]);
        data
    }

    #[test]
    fn parses_page_layers_and_nested_objects() {
        let page = parse_stored_page_bytes_with_limits(
            &stored_page_with_object_tree(),
            &ParseLimits::default(),
        )
        .unwrap();

        assert_eq!(page.header.uuid, "page");
        assert_eq!(page.header.format_version, Some(5500));
        assert_eq!(page.layers.layers.len(), 1);
        assert_eq!(page.layers.layers[0].number, 7);
        let object = &page.layers.layers[0].objects[0];
        assert_eq!(object.object_type, ObjectType::Container);
        assert_eq!(object.children[0].object_type, ObjectType::TextBox);
        assert_eq!(
            object.children[0]
                .payload(&stored_page_with_object_tree())
                .unwrap(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn enforces_the_total_object_limit_across_children() {
        let limits = ParseLimits {
            max_objects_per_page: 1,
            ..ParseLimits::default()
        };

        let error = parse_stored_page_bytes_with_limits(&stored_page_with_object_tree(), &limits)
            .unwrap_err();

        assert!(matches!(
            error,
            Error::LimitExceeded {
                resource: "objects per page",
                limit: 1,
                actual: 2,
            }
        ));
    }

    #[test]
    fn parses_complete_page_manifest_entries() {
        let mut data = vec![0xaa; 32];
        data.extend_from_slice(&2_u16.to_le_bytes());
        for (id, hash) in [("first", 0x11), ("second", 0x22)] {
            data.extend_from_slice(&(id.encode_utf16().count() as u16).to_le_bytes());
            for unit in id.encode_utf16() {
                data.extend_from_slice(&unit.to_le_bytes());
            }
            data.extend_from_slice(&[hash; 32]);
        }

        let manifest = parse_page_manifest_bytes(&data).unwrap();

        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].page_id, "first");
        assert_eq!(manifest.entries[1].integrity_hash, [0x22; 32]);
        assert!(manifest.trailing_data.is_empty());
    }
}
