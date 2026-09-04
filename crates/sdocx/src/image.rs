use crate::binary::Reader;
use crate::frame::Frame;
use crate::object::read_bbox;
use crate::{Error, ObjectMetadata, PlacedImage, Result};

pub(crate) struct DecodedImage {
    pub(crate) image: PlacedImage,
    pub(crate) unsupported: Vec<&'static str>,
}

/// Native ObjectImage: 0 + 6 + 7 + 3. The displayed media ID is in frame 7's
/// FillImageEffect, not frame 3's optional border/original-image references.
pub(crate) fn decode_image(data: &[u8]) -> Result<DecodedImage> {
    let mut reader = Reader::new(data, "image object");
    let base = ObjectMetadata::read(&mut reader)?;
    let shape_base = Frame::read(&mut reader)?;
    shape_base.expect_kind(6)?;
    let shape = Frame::read(&mut reader)?;
    shape.expect_kind(7)?;
    let tail = Frame::read(&mut reader)?;
    tail.expect_kind(3)?;
    let mut unsupported = Vec::new();
    if shape_base.fields.has_other_bits(0)
        || shape_base.properties.has_other_bits(0)
        || !shape_base.fixed.is_empty()
        || !shape_base.flexible.is_empty()
    {
        unsupported.push("inherited shape settings");
    }
    // ObjectShapeBinaryHandler writes these fixed fields before its flexible
    // data: shape type, local bounds, rotation, sized path, control points.
    let mut fixed = Reader::new(shape.fixed, "image shape fixed data");
    fixed.read_u32("shape type")?;
    read_bbox(&mut fixed)?;
    let shape_rotation = finite_f32(&mut fixed, "shape rotation")?;
    let path_size = fixed.read_u32("shape path size")? as usize;
    fixed.skip(path_size, "shape path")?;
    let point_count = usize::from(fixed.read_u8("control point count")?);
    fixed.skip(point_count * 16, "control points")?;
    if (shape_rotation != 0.0 && f64::from(shape_rotation) != base.rotation_degrees.unwrap_or(0.0))
        || path_size != 0
        || point_count != 0
        || fixed.remaining() != 0
        || shape.properties.has_other_bits(0)
    {
        unsupported.push("image shape geometry or properties");
    }
    let mut fields = Reader::new(shape.flexible, "image shape fields");
    if shape.fields.contains(0) {
        skip_sized(&mut fields, "image text common")?;
        unsupported.push("image text content");
    }
    if shape.fields.contains(1) {
        fields.read_u8("text control")?;
    }
    if shape.fields.contains(2) {
        fields.read_i32("shape pen name ID")?;
    }
    let media_id = if shape.fields.contains(3) {
        // This gap is not emitted by the observed writer. Its unknown width
        // prevents locating later fields safely; never search for a fill marker.
        unsupported.push("unknown field before the image fill");
        None
    } else {
        if shape.fields.contains(4) {
            fields.read_i32("shape pen settings ID")?;
        }
        if shape.fields.contains(5) {
            let size = fields.read_u32("fill size")? as usize;
            let kind = fields.read_u8("fill type")?;
            let fill = fields.read_bytes(size, "fill payload")?;
            if kind == 2 {
                parse_image_fill(fill, &mut unsupported)?
            } else {
                unsupported.push("non-image fill effect");
                None
            }
        } else {
            None
        }
    };
    if shape.fields.has_other_bits(0x35) || fields.remaining() != 0 {
        unsupported.push("additional shape fields");
    }
    let mut image = PlacedImage {
        bbox: base.bbox,
        rotation_degrees: base.rotation_degrees,
        media_id,
        media_index: None,
        crop_rect: None,
        border_media_id: None,
        original_media_id: None,
    };
    let mut fields = Reader::new(tail.flexible, "image settings");
    for bit in 0..=19 {
        if !tail.fields.contains(bit) {
            continue;
        }
        match bit {
            1 => image.crop_rect = Some(read_rect(&mut fields)?),
            3 => {
                fields.read_u32("border color")?;
            }
            4 => {
                finite_f32(&mut fields, "border width")?;
            }
            5 => {
                fields.read_u16("border type")?;
            }
            9 => image.border_media_id = read_media_id(&mut fields)?,
            10 => {
                read_rect(&mut fields)?;
            }
            11 => {
                for _ in 0..4 {
                    finite_f32(&mut fields, "border widths")?;
                }
            }
            12 => {
                fields.read_u32("border nine-patch width")?;
            }
            17 => {
                read_bbox(&mut fields)?;
            }
            18 => image.original_media_id = read_media_id(&mut fields)?,
            19 => {
                skip_sized(&mut fields, "image path")?;
                fields.skip(32, "image path rectangles")?;
            }
            _ => break, // Unknown preceding field: leave the remainder bounded.
        }
    }
    if tail.fields.has_other_bits(0)
        || tail.properties.has_other_bits(0)
        || !tail.fixed.is_empty()
        || fields.remaining() != 0
    {
        unsupported.push("crop, border, original-image or extension settings");
    }
    if reader.remaining() != 0 {
        while reader.remaining() != 0 {
            Frame::read(&mut reader)?;
        }
        unsupported.push("additional image frames");
    }
    Ok(DecodedImage { image, unsupported })
}

fn parse_image_fill(data: &[u8], unsupported: &mut Vec<&'static str>) -> Result<Option<u32>> {
    // Normal WDoc is 62 bytes. Coedit substitutes a 64-byte hash for the
    // four-byte bind ID (122 bytes); never interpret that hash as a numeric ID.
    if data.len() > 62 {
        unsupported.push("alternate or extended image-fill encoding");
        return Ok(None);
    }
    let mut reader = Reader::new(data, "image fill");
    let fill_type = reader.read_u8("image fill mode")?;
    let media_id = read_media_id(&mut reader)?;
    let mut settings = [0.0; 9];
    for value in &mut settings {
        *value = finite_f32(&mut reader, "image fill settings")?;
    }
    let rotatable = reader.read_u8("fill rotatable flag")?;
    let nine_patch = read_rect(&mut reader)?;
    let nine_patch_width = reader.read_i32("nine-patch width")?;
    if fill_type != 0
        || settings[..6].iter().any(|value| *value != 0.0)
        || settings[6..8].iter().any(|value| *value != 100.0)
        || settings[8] != 0.0
        || rotatable != 0
        || nine_patch != [0; 4]
        || nine_patch_width != 0
    {
        unsupported.push("image fill transforms, tiling, transparency or nine-patch");
    }
    Ok(media_id)
}

fn read_media_id(reader: &mut Reader<'_>) -> Result<Option<u32>> {
    let id = reader.read_i32("media bind ID")?;
    Ok(u32::try_from(id).ok())
}

fn read_rect(reader: &mut Reader<'_>) -> Result<[i32; 4]> {
    Ok([
        reader.read_i32("rectangle left")?,
        reader.read_i32("rectangle top")?,
        reader.read_i32("rectangle right")?,
        reader.read_i32("rectangle bottom")?,
    ])
}

fn finite_f32(reader: &mut Reader<'_>, field: &'static str) -> Result<f32> {
    let value = reader.read_f32(field)?;
    if !value.is_finite() {
        return Err(Error::Format(format!("image: non-finite {field}")));
    }
    Ok(value)
}

fn skip_sized(reader: &mut Reader<'_>, field: &'static str) -> Result<()> {
    let size = reader.read_u32(field)? as usize;
    reader.skip(size, field)
}
