use crate::binary::Reader;
use crate::{BoundingBox, Error, ObjectMetadata, ParseLimits, Point, Result};

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectFlexibleMetadata {
    pub partial_rectangles: Option<Vec<[u8; 16]>>,
    pub sor_info: Option<String>,
    pub sor_data: Option<ObjectBundle>,
    pub sor_package_link: Option<String>,
    pub extra_data: Option<ObjectBundle>,
    pub attached_file_id: Option<i32>,
    pub min_size: Option<ObjectSize>,
    pub max_size: Option<ObjectSize>,
    pub append_time_raw: Option<i64>,
    pub owner_page_size: Option<ObjectPageSize>,
    pub layout_type: Option<ObjectLayoutType>,
    pub saved_span_data: Option<[u8; 20]>,
    pub captured_thumbnail_media_id: Option<i32>,
    pub pivot: Option<Point>,
    pub group_id: Option<String>,
    pub page_index: Option<i32>,
    pub render_layer_id: Option<i32>,
    pub first_unparsed_field: Option<usize>,
    pub trailing_data: Vec<u8>,
}

impl ObjectFlexibleMetadata {
    pub fn render_layer(&self) -> Option<ObjectRenderLayer> {
        self.render_layer_id.map(ObjectRenderLayer::from)
    }

    pub fn saved_span_snapshot(&self) -> Option<ObjectSpanSnapshot> {
        self.saved_span_data.map(|data| {
            let components = data.as_chunks::<4>().0;
            let [left, top, right, bottom, rotation_degrees] =
                std::array::from_fn(|index| f32::from_le_bytes(components[index]));
            ObjectSpanSnapshot {
                bbox: BoundingBox {
                    x_min: f64::from(left),
                    y_min: f64::from(top),
                    x_max: f64::from(right),
                    y_max: f64::from(bottom),
                },
                rotation_degrees,
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectSpanSnapshot {
    pub bbox: BoundingBox,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectPageSize {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectLayoutType {
    Normal,
    Flow,
    Block,
    Undefined,
    Other(u8),
}

impl From<u8> for ObjectLayoutType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Flow,
            2 => Self::Block,
            3 => Self::Undefined,
            value => Self::Other(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectRenderLayer {
    Base,
    Top,
    Masking,
    Other(i32),
}

impl From<i32> for ObjectRenderLayer {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Base,
            1 => Self::Top,
            2 => Self::Masking,
            value => Self::Other(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectBundle {
    pub category_mask: u8,
    pub entries: Vec<ObjectBundleEntry>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ObjectBundleEntry {
    pub key: String,
    pub value: ObjectBundleValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ObjectBundleValue {
    String(Option<String>),
    Integer(i32),
    StringArray(Vec<String>),
    Bytes(Vec<u8>),
}

impl ObjectMetadata {
    pub fn flexible_metadata(&self) -> Result<ObjectFlexibleMetadata> {
        self.flexible_metadata_with_limits(&ParseLimits::default())
    }

    pub fn flexible_metadata_with_limits(
        &self,
        limits: &ParseLimits,
    ) -> Result<ObjectFlexibleMetadata> {
        if self.flexible_trailing_data.len() as u64 > limits.max_entry_size {
            return Err(Error::LimitExceeded {
                resource: "object metadata size",
                limit: limits.max_entry_size,
                actual: self.flexible_trailing_data.len() as u64,
            });
        }
        ObjectFlexibleDecoder {
            limits,
            entries: 0,
            text_units: 0,
        }
        .parse(self)
    }
}

struct ObjectFlexibleDecoder<'a> {
    limits: &'a ParseLimits,
    entries: usize,
    text_units: usize,
}

impl ObjectFlexibleDecoder<'_> {
    fn parse(&mut self, base: &ObjectMetadata) -> Result<ObjectFlexibleMetadata> {
        let mut reader = Reader::new(&base.flexible_trailing_data, "object flexible data");
        let mut metadata = ObjectFlexibleMetadata::default();
        for bit in 1..base.field_mask.len() * 8 {
            if base.field_mask[bit / 8] & (1 << (bit % 8)) == 0 {
                continue;
            }
            match bit {
                1 => {
                    let count = usize::from(reader.read_u16("partial rectangle count")?);
                    self.reserve_entries(&reader, count, 16)?;
                    let bytes = reader.read_bytes(count * 16, "partial rectangles")?;
                    metadata.partial_rectangles = Some(bytes.as_chunks::<16>().0.to_vec());
                }
                2 => metadata.sor_info = Some(self.string(&mut reader, "SOR information")?),
                3 | 5 => {
                    let Some(bundle) = self.bundle(&mut reader)? else {
                        metadata.first_unparsed_field = Some(bit);
                        break;
                    };
                    if bit == 3 {
                        metadata.sor_data = Some(bundle);
                    } else {
                        metadata.extra_data = Some(bundle);
                    }
                }
                4 => {
                    metadata.sor_package_link = Some(self.string(&mut reader, "SOR package link")?)
                }
                6 => {
                    if base.format_version >= 6 {
                        metadata.attached_file_id = Some(reader.read_i32("attached file ID")?);
                    }
                }
                7 => {
                    if base.format_version >= 9 {
                        metadata.min_size = Some(ObjectSize {
                            width: reader.read_f32("minimum width")?,
                            height: reader.read_f32("minimum height")?,
                        });
                    }
                }
                8 => {
                    if base.format_version >= 13 {
                        metadata.max_size = Some(ObjectSize {
                            width: reader.read_f32("maximum width")?,
                            height: reader.read_f32("maximum height")?,
                        });
                    }
                }
                13 => metadata.append_time_raw = Some(reader.read_i64("append time")?),
                14 => {
                    metadata.owner_page_size = Some(ObjectPageSize {
                        width: reader.read_i32("owner page width")?,
                        height: reader.read_i32("owner page height")?,
                    });
                }
                15 => metadata.layout_type = Some(reader.read_u8("layout type")?.into()),
                16 => {
                    metadata.saved_span_data = Some(
                        reader
                            .read_bytes(20, "saved span data")?
                            .try_into()
                            .map_err(|_| Error::Format("invalid saved span data".into()))?,
                    );
                }
                17 => {
                    metadata.captured_thumbnail_media_id =
                        Some(reader.read_i32("captured thumbnail media ID")?)
                }
                18 => {
                    metadata.pivot = Some(Point {
                        x: reader.read_f64("pivot x")?,
                        y: reader.read_f64("pivot y")?,
                    });
                }
                19 => metadata.group_id = Some(self.string(&mut reader, "group ID")?),
                20 => metadata.page_index = Some(reader.read_i32("page index")?),
                21 => metadata.render_layer_id = Some(reader.read_i32("render layer ID")?),
                _ => {
                    metadata.first_unparsed_field = Some(bit);
                    break;
                }
            }
        }
        metadata.trailing_data = reader.remaining_bytes().to_vec();
        Ok(metadata)
    }

    fn bundle(&mut self, reader: &mut Reader<'_>) -> Result<Option<ObjectBundle>> {
        let data = reader.remaining_bytes();
        let mut bundle = Reader::new(data, "object metadata bundle");
        let category_mask = bundle.read_u8("category mask")?;
        if category_mask & !0x0f != 0 {
            return Ok(None);
        }
        let mut entries = Vec::new();
        for category in 0..4 {
            if category_mask & (1 << category) == 0 {
                continue;
            }
            let count = usize::from(bundle.read_u16("category entry count")?);
            let minimum_size = if category == 1 || category == 3 { 6 } else { 4 };
            self.reserve_entries(&bundle, count, minimum_size)?;
            for _ in 0..count {
                let key = self.key(&mut bundle)?;
                let value = match category {
                    0 => ObjectBundleValue::String(self.nullable_string(&mut bundle)?),
                    1 => ObjectBundleValue::Integer(bundle.read_i32("integer value")?),
                    2 => {
                        let count = usize::from(bundle.read_u16("string array count")?);
                        self.reserve_entries(&bundle, count, 2)?;
                        let mut values = Vec::with_capacity(count);
                        for _ in 0..count {
                            values.push(self.string(&mut bundle, "string array value")?);
                        }
                        ObjectBundleValue::StringArray(values)
                    }
                    _ => {
                        let length = bundle.read_u32("byte array length")? as usize;
                        ObjectBundleValue::Bytes(bundle.read_bytes(length, "byte array")?.to_vec())
                    }
                };
                entries.push(ObjectBundleEntry { key, value });
            }
        }
        let length = bundle.position();
        reader.skip(length, "bundle")?;
        Ok(Some(ObjectBundle {
            category_mask,
            entries,
            data: data[..length].to_vec(),
        }))
    }

    fn string(&mut self, reader: &mut Reader<'_>, field: &'static str) -> Result<String> {
        let units = usize::from(reader.read_u16(field)?);
        self.reserve_text_units(units)?;
        reader.read_utf16_units(units, field, self.limits.max_text_characters)
    }

    fn nullable_string(&mut self, reader: &mut Reader<'_>) -> Result<Option<String>> {
        let units = reader.read_i16("string value length")?;
        if units < 0 {
            return Ok(None);
        }
        self.reserve_text_units(units as usize)?;
        reader
            .read_utf16_units(
                units as usize,
                "string value",
                self.limits.max_text_characters,
            )
            .map(Some)
    }

    fn key(&mut self, reader: &mut Reader<'_>) -> Result<String> {
        let length = usize::from(reader.read_u16("key byte count")?);
        let value = std::str::from_utf8(reader.read_bytes(length, "key")?)
            .map_err(|_| Error::Format("object metadata bundle: invalid UTF-8 key".into()))?;
        self.reserve_text_units(value.encode_utf16().count())?;
        Ok(value.to_owned())
    }

    fn reserve_text_units(&mut self, count: usize) -> Result<()> {
        let total = self
            .text_units
            .checked_add(count)
            .ok_or_else(|| Error::Format("object metadata: text count overflows".into()))?;
        if total > self.limits.max_text_characters {
            return Err(Error::LimitExceeded {
                resource: "text characters",
                limit: self.limits.max_text_characters as u64,
                actual: total as u64,
            });
        }
        self.text_units = total;
        Ok(())
    }

    fn reserve_entries(
        &mut self,
        reader: &Reader<'_>,
        count: usize,
        minimum_size: usize,
    ) -> Result<()> {
        let total = self
            .entries
            .checked_add(count)
            .ok_or_else(|| Error::Format("object metadata: entry count overflows".into()))?;
        if total > self.limits.max_object_metadata_entries {
            return Err(Error::LimitExceeded {
                resource: "object metadata entries",
                limit: self.limits.max_object_metadata_entries as u64,
                actual: total as u64,
            });
        }
        if count > reader.remaining() / minimum_size {
            return Err(Error::Format(
                "object metadata: entry count exceeds its bounded payload".into(),
            ));
        }
        self.entries = total;
        Ok(())
    }
}
