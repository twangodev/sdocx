use crate::binary::Reader;
use crate::frame::Frame;
use crate::media::MediaResolver;
use crate::object::read_bbox;
use crate::shape::{read_style, visit_path};
use crate::{
    BoundingBox, DiagnosticCode, Error, ObjectMetadata, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, ParseReport, PlacedImage, Result, RichTextBox,
    RichTextObjectContent, ShapePaint,
};

pub(crate) struct DecodedImage {
    pub(crate) image: PlacedImage,
    pub(crate) unsupported: Vec<&'static str>,
    visible: bool,
}

impl DecodedImage {
    pub(crate) fn resolve(
        mut self,
        media: &MediaResolver,
        archive_entry: &str,
        location: &str,
        report: &mut ParseReport,
    ) -> Option<PlacedImage> {
        if !self.visible {
            return None;
        }
        if !self.unsupported.is_empty() {
            report.warning(
                DiagnosticCode::UnsupportedImageFeature,
                Some(archive_entry.to_owned()),
                format!(
                    "{location}: incomplete support for {}",
                    self.unsupported.join(", ")
                ),
            );
        }
        match media.resolve(self.image.media_id) {
            Ok((index, inferred)) => {
                self.image.media_index = Some(index);
                if inferred {
                    report.warning(DiagnosticCode::InferredImageMediaReference, Some(archive_entry.to_owned()), format!("{location}: media/mediaInfo.dat is absent; resolved media ID {} using a unique numeric filename prefix", self.image.media_id.unwrap()));
                }
            }
            Err(message) => report.warning(
                DiagnosticCode::UnresolvedImageMedia,
                Some(archive_entry.to_owned()),
                format!("{location}: {message}"),
            ),
        }
        Some(self.image)
    }
}

pub(crate) fn decode_text_images(
    text: &mut RichTextBox,
    media: &MediaResolver,
    archive_entry: &str,
    report: &mut ParseReport,
) -> Result<()> {
    decode_text_images_in_context(text, media, archive_entry, report, false)
}

fn decode_text_images_in_context(
    text: &mut RichTextBox,
    media: &MediaResolver,
    archive_entry: &str,
    report: &mut ParseReport,
    nested: bool,
) -> Result<()> {
    for span in &mut text.object_spans {
        if span.object_type == ObjectType::Image {
            let location = format!("embedded image at UTF-16 index {}", span.text_index_utf16);
            let mut decoded = decode_image(&span.object_data)?;
            if nested {
                decoded
                    .unsupported
                    .push("image layout inside another embedded text object");
            }
            if span.layout_option != ObjectSpanLayoutOption::Block
                || span.layout_constraint != ObjectSpanLayoutConstraint::Normal
            {
                decoded
                    .unsupported
                    .push("inline, alternate-margin or cross-page image layout");
            }
            span.content = decoded
                .resolve(media, archive_entry, &location, report)
                .map(Box::new)
                .map(RichTextObjectContent::Image);
        }
        match span.content.as_mut() {
            Some(RichTextObjectContent::Table(table)) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        decode_text_images_in_context(
                            &mut cell.content,
                            media,
                            archive_entry,
                            report,
                            true,
                        )?;
                    }
                }
            }
            Some(RichTextObjectContent::CodeBlock(code)) => {
                for text in code.title.iter_mut().chain(code.body.iter_mut()) {
                    decode_text_images_in_context(text, media, archive_entry, report, true)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

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
    read_image_outline(&shape_base, &mut unsupported)?;
    let mut fixed = Reader::new(shape.fixed, "image shape fixed data");
    let shape_type = fixed.read_u32("shape type")?;
    let geometry_bbox = read_bbox(&mut fixed)?;
    let shape_rotation = finite_f32(&mut fixed, "shape rotation")?;
    let path_size = fixed.read_u32("shape path size")? as usize;
    let path = fixed.read_bytes(path_size, "shape path")?;
    let rectangular_path = path.is_empty()
        || (is_placement_rectangle(path, base.bbox, base.rotation_degrees.unwrap_or(0.0))?
            && matches_placement(geometry_bbox, base.bbox));
    let point_count = usize::from(fixed.read_u8("control point count")?);
    fixed.skip(point_count * 16, "control points")?;
    if shape_type != 4
        || (shape_rotation != 0.0
            && f64::from(shape_rotation) != base.rotation_degrees.unwrap_or(0.0))
        || !rectangular_path
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
        fields.read_u8("text area type")?;
    }
    if shape.fields.contains(2) {
        fields.read_i32("shape pen name ID")?;
    }
    let media_id = if shape.fields.contains(3) {
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
        original_bbox: None,
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
                image.original_bbox = Some(read_bbox(&mut fields)?);
            }
            18 => image.original_media_id = read_media_id(&mut fields)?,
            19 => {
                skip_sized(&mut fields, "image path")?;
                fields.skip(32, "image path rectangles")?;
            }
            _ => break,
        }
    }
    if tail.fields.has_other_bits((1 << 1) | (1 << 17))
        || (image.crop_rect.is_some() && image.original_bbox.is_none())
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
    Ok(DecodedImage {
        image,
        unsupported,
        visible: base.visible,
    })
}

fn read_image_outline(frame: &Frame<'_>, unsupported: &mut Vec<&'static str>) -> Result<()> {
    if frame.fixed.is_empty() {
        if frame.fields.has_other_bits(0)
            || frame.properties.has_other_bits(0)
            || !frame.flexible.is_empty()
        {
            unsupported.push("inherited shape settings");
        }
        return Ok(());
    }
    let style = read_style(frame, unsupported)?;
    let invisible = match style.paint {
        ShapePaint::None => true,
        ShapePaint::Solid(argb) => argb >> 24 == 0,
        ShapePaint::Unsupported { .. } => false,
    };
    if style.width != 0.0 && !invisible {
        unsupported.push("image outlines");
    }
    Ok(())
}

fn matches_placement(a: BoundingBox, b: BoundingBox) -> bool {
    a.x_min == b.x_min && a.y_min == b.y_min && a.x_max == b.x_max && a.y_max == b.y_max
}

fn is_placement_rectangle(data: &[u8], bbox: BoundingBox, rotation: f64) -> Result<bool> {
    let mut points = [[0.0; 2]; 4];
    let mut commands = 0;
    let mut rectangular = true;
    let (consumed, supported) = visit_path(data, |verb, coordinates| {
        if commands < 4 && verb == if commands == 0 { 1 } else { 2 } {
            points[commands].copy_from_slice(coordinates);
        } else if commands != 4 || verb != 6 {
            rectangular = false;
        }
        commands += 1;
    })?;
    if !supported || !rectangular || commands != 5 || consumed != data.len() {
        return Ok(false);
    }
    let cx = (bbox.x_min + bbox.x_max) / 2.0;
    let cy = (bbox.y_min + bbox.y_max) / 2.0;
    let (sin, cos) = rotation.to_radians().sin_cos();
    let corners = [
        [bbox.x_min, bbox.y_min],
        [bbox.x_max, bbox.y_min],
        [bbox.x_max, bbox.y_max],
        [bbox.x_min, bbox.y_max],
    ];
    let coordinate_scale = corners
        .iter()
        .flatten()
        .fold(1.0_f64, |scale, value| scale.max(value.abs()));
    let tolerance = coordinate_scale * f64::from(f32::EPSILON) * 8.0;
    Ok(points.iter().zip(corners).all(|(point, [x, y])| {
        let expected = [
            cx + cos * (x - cx) - sin * (y - cy),
            cy + sin * (x - cx) + cos * (y - cy),
        ];
        point.iter().zip(expected).all(|(actual, expected)| {
            expected.is_finite() && (actual - expected).abs() <= tolerance
        })
    }))
}

fn parse_image_fill(data: &[u8], unsupported: &mut Vec<&'static str>) -> Result<Option<u32>> {
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
    reader.read_i32("nine-patch width")?;
    if fill_type != 0
        || settings[..6].iter().any(|value| *value != 0.0)
        || settings[6..8].iter().any(|value| *value != 100.0)
        || settings[8] != 0.0
        || rotatable != 0
        || nine_patch != [0; 4]
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
