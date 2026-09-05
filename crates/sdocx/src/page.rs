use crate::ParseLimits;
use crate::binary::Reader;
use crate::decode::decode_stroke;
use crate::error::{Error, Result};
use crate::image::decode_image;
use crate::media::MediaResolver;
use crate::note::parse_page_text_box;
use crate::object::read_bbox;
use crate::report::{DiagnosticCode, ParseReport};
use crate::shape::{decode_line, decode_shape};
use crate::storage::{StoredObject, StoredPage};
use crate::types::{
    BoundingBox, Color, ObjectType, Page, PageElement, PageTemplate, PageTemplateSource,
};

pub(crate) fn parse_page(
    data: &[u8],
    stored: &StoredPage,
    limits: &ParseLimits,
    archive_entry: &str,
    report: &mut ParseReport,
    media: &MediaResolver,
) -> Result<Page> {
    let header = &stored.header;
    let stroke_count = stored
        .layers
        .layers
        .iter()
        .map(|layer| count_strokes(&layer.objects))
        .sum();
    check_limit(
        "strokes per page",
        limits.max_strokes_per_page,
        stroke_count,
    )?;
    let mut page = Page {
        uuid: header.uuid.clone(),
        width: header.width,
        height: header.height,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        strokes: Vec::with_capacity(stroke_count),
        elements: Vec::new(),
    };
    parse_page_properties(data, stored, &mut page)?;
    for layer in &stored.layers.layers {
        decode_objects(
            data,
            &layer.objects,
            &mut page,
            media,
            limits,
            archive_entry,
            report,
        )?;
    }
    Ok(page)
}

fn count_strokes(objects: &[StoredObject]) -> usize {
    objects
        .iter()
        .map(|object| {
            usize::from(object.object_type == ObjectType::Stroke) + count_strokes(&object.children)
        })
        .sum()
}

fn decode_objects(
    data: &[u8],
    objects: &[StoredObject],
    page: &mut Page,
    media: &MediaResolver,
    limits: &ParseLimits,
    archive_entry: &str,
    report: &mut ParseReport,
) -> Result<()> {
    for object in objects {
        let payload = object
            .payload(data)
            .ok_or_else(|| Error::Format("object payload is outside its page".into()))?;
        if !matches!(object.object_type, ObjectType::Other(_))
            && object.base_metadata(data).is_ok_and(|base| !base.visible)
        {
            continue;
        }
        if object.object_type == ObjectType::Stroke {
            let stroke = decode_stroke(payload, limits).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: stroke at 0x{:x}: {message}",
                    page.uuid, object.payload_offset
                )),
                error => error,
            })?;
            page.strokes.push(stroke);
        } else if object.object_type == ObjectType::TextBox {
            let decoded = parse_page_text_box(payload, limits).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: text box at 0x{:x}: {message}",
                    page.uuid, object.payload_offset
                )),
                error => error,
            })?;
            if !decoded.unsupported.is_empty() {
                report.warning(
                    DiagnosticCode::UnsupportedTextBoxFeature,
                    Some(archive_entry.to_owned()),
                    format!(
                        "page {}: text box at 0x{:x}: incomplete semantic support for {}",
                        page.uuid,
                        object.payload_offset,
                        decoded.unsupported.join(", ")
                    ),
                );
            }
            check_limit(
                "objects per page",
                limits.max_objects_per_page,
                page.elements.len() + 1,
            )?;
            page.elements.push(PageElement::TextBox(decoded.text_box));
        } else if object.object_type == ObjectType::Image {
            let mut decoded = decode_image(payload).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: image at 0x{:x}: {message}",
                    page.uuid, object.payload_offset
                )),
                error => error,
            })?;
            let location = format!("page {}: image at 0x{:x}", page.uuid, object.payload_offset);
            if !decoded.unsupported.is_empty() {
                report.warning(
                    DiagnosticCode::UnsupportedImageFeature,
                    Some(archive_entry.to_owned()),
                    format!(
                        "{location}: incomplete support for {}",
                        decoded.unsupported.join(", ")
                    ),
                );
            }
            match media.resolve(decoded.image.media_id) {
                Ok((index, inferred)) => {
                    decoded.image.media_index = Some(index);
                    if inferred {
                        report.warning(DiagnosticCode::InferredImageMediaReference, Some(archive_entry.to_owned()), format!("{location}: media/mediaInfo.dat is absent; resolved media ID {} using a unique numeric filename prefix", decoded.image.media_id.unwrap()));
                    }
                }
                Err(message) => report.warning(
                    DiagnosticCode::UnresolvedImageMedia,
                    Some(archive_entry.to_owned()),
                    format!("{location}: {message}"),
                ),
            }
            page.elements.push(PageElement::PlacedImage(decoded.image));
        } else if matches!(object.object_type, ObjectType::Shape | ObjectType::Line) {
            let decoded = if object.object_type == ObjectType::Shape {
                decode_shape(payload, limits)
                    .map(|decoded| (PageElement::Shape(decoded.value), decoded.unsupported))
            } else {
                decode_line(payload)
                    .map(|decoded| (PageElement::Line(decoded.value), decoded.unsupported))
            };
            let (element, unsupported) = decoded.map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: {:?} at 0x{:x}: {message}",
                    page.uuid, object.object_type, object.payload_offset
                )),
                error => error,
            })?;
            if !unsupported.is_empty() {
                report.warning(
                    DiagnosticCode::UnsupportedShapeFeature,
                    Some(archive_entry.to_owned()),
                    format!(
                        "page {}: {:?} at 0x{:x}: incomplete support for {}",
                        page.uuid,
                        object.object_type,
                        object.payload_offset,
                        unsupported.join(", ")
                    ),
                );
            }
            page.elements.push(element);
        } else if !matches!(object.object_type, ObjectType::Other(_)) {
            report.warning(
                DiagnosticCode::UnsupportedObjectType,
                Some(archive_entry.to_owned()),
                format!(
                    "page {}: {:?} (type {}) at 0x{:x}: payload retained without semantic decoding; child records are traversed separately",
                    page.uuid,
                    object.object_type,
                    object.object_type.raw(),
                    object.payload_offset
                ),
            );
        }
        decode_objects(
            data,
            &object.children,
            page,
            media,
            limits,
            archive_entry,
            report,
        )?;
    }
    Ok(())
}

fn parse_page_properties(data: &[u8], stored: &StoredPage, page: &mut Page) -> Result<()> {
    let header = &stored.header;
    if header.property_offset == 0 {
        return Ok(());
    }
    let bytes = data
        .get(header.property_offset as usize..header.raw_layer_offset as usize)
        .ok_or_else(|| Error::Format("page flexible fields are outside the header".into()))?;
    let mut fields = Reader::new(bytes, "page flexible fields");
    let mut pdf_page_index = None;
    for bit in 0..=9 {
        if header.property_mask & (1 << bit) == 0 {
            continue;
        }
        match bit {
            0 => page.content_bbox = read_bbox(&mut fields)?,
            1 => {
                let count = fields.read_u16("tag count")?;
                for _ in 0..count {
                    fields.read_utf16_u16("tag")?;
                }
            }
            2 => {
                fields.read_utf16_u16("template URI")?;
            }
            3 | 4 | 6 | 7 => {
                fields.read_u32("background property")?;
            }
            5 => {
                let argb = fields.read_u32("background color")?;
                page.background_color = Some(Color {
                    r: (argb >> 16) as u8,
                    g: (argb >> 8) as u8,
                    b: argb as u8,
                });
            }
            8 => {
                let count = fields.read_u16("PDF record count")?;
                for index in 0..count {
                    fields.read_u32("PDF media ID")?;
                    let page_index = fields.read_u32("PDF page index")?;
                    if index == 0 {
                        pdf_page_index = Some(page_index);
                    }
                    fields.skip(16, "PDF rectangle")?;
                }
            }
            9 => {
                let id = fields.read_u32("template type")?;
                if is_builtin_template_id(id) {
                    page.template = Some(PageTemplate {
                        id,
                        source: PageTemplateSource::BuiltIn,
                    });
                }
            }
            _ => unreachable!(),
        }
    }
    if let Some(page_index) = pdf_page_index {
        page.template = Some(PageTemplate {
            id: page_index,
            source: PageTemplateSource::CustomPdf { page_index },
        });
    }
    Ok(())
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

fn is_builtin_template_id(id: u32) -> bool {
    id != 0 && id <= 0xFFFF
}
