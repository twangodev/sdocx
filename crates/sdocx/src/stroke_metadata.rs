use crate::binary::Reader;
use crate::decode::StrokeChannels;
use crate::frame::{Frame, Mask};
use crate::{Error, ObjectMetadata, ObjectType, ParseLimits, Result, StoredObject};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StrokeMetadata {
    pub base: ObjectMetadata,
    pub properties: StrokeProperties,
    pub point_count: u16,
    pub tool_type_raw: u16,
    pub style: StrokeStyle,
    pub property_mask: Vec<u8>,
    pub field_mask: Vec<u8>,
    pub trailing_data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StrokeProperties {
    pub compressed: bool,
    pub replay_only: bool,
    pub stylus_channels: bool,
    pub eraser: bool,
    pub fixed_width: bool,
    pub millisecond_timestamps: bool,
    pub top_layer_pen: bool,
    pub alpha_lock: bool,
    pub binary_added: bool,
    pub generated: bool,
    pub fixed_opacity: bool,
    pub rainbow_effect: bool,
    pub straighten: bool,
    pub reveal_mode: bool,
}

impl StrokeProperties {
    fn read(mask: Mask<'_>) -> Self {
        Self {
            compressed: mask.contains(0),
            replay_only: mask.contains(1),
            stylus_channels: mask.contains(2),
            eraser: mask.contains(3),
            fixed_width: mask.contains(4),
            millisecond_timestamps: mask.contains(5),
            top_layer_pen: mask.contains(6),
            alpha_lock: mask.contains(7),
            binary_added: !mask.contains(8),
            generated: !mask.contains(10),
            fixed_opacity: mask.contains(11),
            rainbow_effect: mask.contains(12),
            straighten: mask.contains(13),
            reveal_mode: mask.contains(14),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StrokeStyle {
    pub legacy_pen_name_id: Option<i32>,
    pub advanced_pen_setting_id: Option<i32>,
    pub color_argb: Option<u32>,
    pub pen_size: Option<f32>,
    pub field_4_raw: Option<u8>,
    pub legacy_partial_rectangle_data: Option<Vec<[u8; 4]>>,
    pub pen_name_id: Option<i32>,
    pub fixed_width: Option<f32>,
    pub size_level: Option<i32>,
    pub particle_density: Option<i32>,
    pub rendering_level: Option<i32>,
    pub original_width: Option<i32>,
    pub initial_tolerance: Option<f32>,
    pub line_type_raw: Option<u16>,
    pub dash_offset: Option<f32>,
    pub stroke_type_raw: Option<u16>,
    pub pen_repeat_distance: Option<f32>,
    pub particle_size: Option<f32>,
    pub pattern_index: Option<i32>,
    pub pattern_scale: Option<f32>,
    pub particle_level: Option<i32>,
    pub rainbow_distance: Option<i32>,
    pub rainbow_offset: Option<f32>,
    pub gradient_colors_argb: Option<Vec<u32>>,
    pub color_type_raw: Option<u16>,
    pub first_unparsed_field: Option<usize>,
    pub trailing_data: Vec<u8>,
}

impl StoredObject {
    pub fn stroke_metadata(&self, page_bytes: &[u8]) -> Result<StrokeMetadata> {
        self.stroke_metadata_with_limits(page_bytes, &ParseLimits::default())
    }

    pub fn stroke_metadata_with_limits(
        &self,
        page_bytes: &[u8],
        limits: &ParseLimits,
    ) -> Result<StrokeMetadata> {
        if self.object_type != ObjectType::Stroke {
            return Err(Error::Format(format!(
                "expected a Stroke object, found {:?}",
                self.object_type
            )));
        }
        let payload = self
            .payload(page_bytes)
            .ok_or_else(|| Error::Format("stroke payload is outside its page".into()))?;
        if payload.len() as u64 > limits.max_entry_size {
            return Err(Error::LimitExceeded {
                resource: "stroke payload size",
                limit: limits.max_entry_size,
                actual: payload.len() as u64,
            });
        }
        let mut reader = Reader::new(payload, "stroke object");
        let base = ObjectMetadata::read(&mut reader)?;
        let frame = Frame::read(&mut reader)?;
        frame.expect_kind(1)?;
        let channels = StrokeChannels::read(&frame, limits)?;
        let style = StrokeStyle::read(&frame, &base, limits)?;
        Ok(StrokeMetadata {
            base,
            properties: StrokeProperties::read(frame.properties),
            point_count: channels.point_count,
            tool_type_raw: channels.tool_type_raw,
            style,
            property_mask: frame.properties.bytes().to_vec(),
            field_mask: frame.fields.bytes().to_vec(),
            trailing_data: reader.remaining_bytes().to_vec(),
        })
    }
}

impl StrokeStyle {
    pub(crate) fn read_prefix<'a>(frame: &Frame<'a>) -> Result<(Self, Reader<'a>)> {
        let mut reader = Reader::new(frame.flexible, "stroke style");
        let mut style = Self::default();
        if frame.fields.contains(0) {
            style.legacy_pen_name_id = Some(reader.read_i32("legacy pen name ID")?);
        }
        if frame.fields.contains(1) {
            style.advanced_pen_setting_id = Some(reader.read_i32("pen settings ID")?);
        }
        if frame.fields.contains(2) {
            style.color_argb = Some(reader.read_u32("ARGB color")?);
        }
        if frame.fields.contains(3) {
            let size = reader.read_f32("pen size")?;
            if !size.is_finite() || size < 0.0 {
                return Err(Error::Format("invalid stroke pen size".into()));
            }
            style.pen_size = Some(size);
        }
        Ok((style, reader))
    }

    fn read(frame: &Frame<'_>, base: &ObjectMetadata, limits: &ParseLimits) -> Result<Self> {
        let (mut style, mut reader) = Self::read_prefix(frame)?;
        let mut entries = 0;
        for bit in 4..usize::from(frame.fields.byte_count()) * 8 {
            if !frame.fields.contains(bit) {
                continue;
            }
            match bit {
                4 => style.field_4_raw = Some(reader.read_u8("field 4")?),
                5 => {
                    let count = partial_rectangle_count(base)?;
                    reserve_entries(&reader, count, &mut entries, limits)?;
                    let bytes = reader.read_bytes(count * 4, "legacy partial rectangle data")?;
                    style.legacy_partial_rectangle_data = Some(bytes.as_chunks::<4>().0.to_vec());
                }
                7 => style.pen_name_id = Some(reader.read_i32("pen name ID")?),
                8 => style.fixed_width = Some(finite_float(&mut reader, "fixed width")?),
                9 => style.size_level = Some(reader.read_i32("size level")?),
                10 => style.particle_density = Some(reader.read_i32("particle density")?),
                11 => style.rendering_level = Some(reader.read_i32("rendering level")?),
                12 => style.original_width = Some(reader.read_i32("original width")?),
                13 => {
                    style.initial_tolerance = Some(finite_float(&mut reader, "initial tolerance")?)
                }
                14 => style.line_type_raw = Some(reader.read_u16("line type")?),
                15 => style.dash_offset = Some(finite_float(&mut reader, "dash offset")?),
                16 => style.stroke_type_raw = Some(reader.read_u16("stroke type")?),
                17 => {
                    style.pen_repeat_distance =
                        Some(finite_float(&mut reader, "pen repeat distance")?)
                }
                18 => style.particle_size = Some(finite_float(&mut reader, "particle size")?),
                19 => style.pattern_index = Some(reader.read_i32("pattern index")?),
                20 => style.pattern_scale = Some(finite_float(&mut reader, "pattern scale")?),
                21 => style.particle_level = Some(reader.read_i32("particle level")?),
                22 => style.rainbow_distance = Some(reader.read_i32("rainbow distance")?),
                23 => style.rainbow_offset = Some(finite_float(&mut reader, "rainbow offset")?),
                24 => {
                    let count = usize::from(reader.read_u16("gradient color count")?);
                    reserve_entries(&reader, count, &mut entries, limits)?;
                    let bytes = reader.read_bytes(count * 4, "gradient colors")?;
                    style.gradient_colors_argb = Some(
                        bytes
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .copied()
                            .map(u32::from_le_bytes)
                            .collect(),
                    );
                }
                25 => style.color_type_raw = Some(reader.read_u16("color type")?),
                _ => {
                    style.first_unparsed_field = Some(bit);
                    break;
                }
            }
        }
        style.trailing_data = reader.remaining_bytes().to_vec();
        Ok(style)
    }
}

fn partial_rectangle_count(base: &ObjectMetadata) -> Result<usize> {
    if !base.field_mask.first().is_some_and(|mask| mask & 2 != 0) {
        return Ok(0);
    }
    let mut reader = Reader::new(
        &base.flexible_trailing_data,
        "stroke base partial rectangles",
    );
    let count = usize::from(reader.read_u16("partial rectangle count")?);
    reader.skip(count * 16, "partial rectangles")?;
    Ok(count)
}

fn finite_float(reader: &mut Reader<'_>, field: &'static str) -> Result<f32> {
    let value = reader.read_f32(field)?;
    if !value.is_finite() {
        return Err(Error::Format(format!("non-finite stroke {field}")));
    }
    Ok(value)
}

fn reserve_entries(
    reader: &Reader<'_>,
    count: usize,
    entries: &mut usize,
    limits: &ParseLimits,
) -> Result<()> {
    *entries = entries.checked_add(count).ok_or(Error::LimitExceeded {
        resource: "stroke metadata entries",
        limit: limits.max_object_metadata_entries as u64,
        actual: u64::MAX,
    })?;
    if *entries > limits.max_object_metadata_entries {
        return Err(Error::LimitExceeded {
            resource: "stroke metadata entries",
            limit: limits.max_object_metadata_entries as u64,
            actual: *entries as u64,
        });
    }
    if count > reader.remaining() / 4 {
        return Err(Error::Format(
            "stroke metadata records exceed their frame".into(),
        ));
    }
    Ok(())
}
