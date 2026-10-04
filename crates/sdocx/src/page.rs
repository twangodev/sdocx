use crate::ParseLimits;
use crate::binary::Reader;
use crate::decode::decode_stroke;
use crate::error::{Error, Result};
use crate::image::{decode_image, decode_text_images};
use crate::media::MediaResolver;
use crate::note::parse_page_text_box;
use crate::object::read_bbox;
use crate::report::{DiagnosticCode, ParseReport};
use crate::shape::{decode_line, decode_shape};
use crate::storage::{StoredObject, StoredPage};
use crate::types::{
    BoundingBox, Color, ObjectType, Page, PageElement, PageTemplate, PageTemplateSource,
    PdfPaperRecord, PdfPaperRectangle,
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
    let current_layer = &stored.layers.layers[usize::from(stored.layers.current_layer_index)];
    let mut page = Page {
        uuid: header.uuid.clone(),
        width: header.width,
        height: header.height,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: Vec::new(),
    };
    parse_page_properties(data, stored, &mut page)?;
    page.objects = decode_objects(
        data,
        &current_layer.objects,
        &page.uuid,
        media,
        limits,
        archive_entry,
        report,
    )?;
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
    page_uuid: &str,
    media: &MediaResolver,
    limits: &ParseLimits,
    archive_entry: &str,
    report: &mut ParseReport,
) -> Result<Vec<crate::PageObject>> {
    use crate::{ObjectRenderLayer, PageObject, PageObjectContent};
    let mut decoded_objects = Vec::with_capacity(objects.len());
    for object in objects {
        let payload = object
            .payload(data)
            .ok_or_else(|| Error::Format("object payload is outside its page".into()))?;
        let base = object.base_metadata(data).ok();
        if !matches!(object.object_type, ObjectType::Other(_))
            && base.as_ref().is_some_and(|base| !base.visible)
        {
            continue;
        }
        let mut content = None;
        if object.object_type == ObjectType::Stroke {
            let stroke = decode_stroke(payload, limits).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: stroke at 0x{:x}: {message}",
                    page_uuid, object.payload_offset
                )),
                error => error,
            })?;
            content = Some(PageObjectContent::Stroke(stroke));
        } else if object.object_type == ObjectType::TextBox {
            let mut decoded =
                parse_page_text_box(payload, limits).map_err(|error| match error {
                    Error::Format(message) => Error::Format(format!(
                        "page {}: text box at 0x{:x}: {message}",
                        page_uuid, object.payload_offset
                    )),
                    error => error,
                })?;
            decode_text_images(&mut decoded.text_box, media, archive_entry, report)?;
            if !decoded.unsupported.is_empty() {
                report.warning(
                    DiagnosticCode::UnsupportedTextBoxFeature,
                    Some(archive_entry.to_owned()),
                    format!(
                        "page {}: text box at 0x{:x}: incomplete semantic support for {}",
                        page_uuid,
                        object.payload_offset,
                        decoded.unsupported.join(", ")
                    ),
                );
            }
            check_limit(
                "objects per page",
                limits.max_objects_per_page,
                decoded_objects.len() + 1,
            )?;
            content = Some(PageObjectContent::Element(PageElement::TextBox(
                decoded.text_box,
            )));
        } else if object.object_type == ObjectType::Image {
            let decoded = decode_image(payload).map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: image at 0x{:x}: {message}",
                    page_uuid, object.payload_offset
                )),
                error => error,
            })?;
            let location = format!("page {page_uuid}: image at 0x{:x}", object.payload_offset);
            if let Some(image) = decoded.resolve(media, archive_entry, &location, report) {
                content = Some(PageObjectContent::Element(PageElement::PlacedImage(image)));
            }
        } else if matches!(object.object_type, ObjectType::Shape | ObjectType::Line) {
            let decoded = if object.object_type == ObjectType::Shape {
                decode_shape(payload, limits)
                    .map(|decoded| (PageElement::Shape(decoded.value), decoded.unsupported))
            } else {
                decode_line(payload)
                    .map(|decoded| (PageElement::Line(decoded.value), decoded.unsupported))
            };
            let (mut element, unsupported) = decoded.map_err(|error| match error {
                Error::Format(message) => Error::Format(format!(
                    "page {}: {:?} at 0x{:x}: {message}",
                    page_uuid, object.object_type, object.payload_offset
                )),
                error => error,
            })?;
            if !unsupported.is_empty() {
                report.warning(
                    DiagnosticCode::UnsupportedShapeFeature,
                    Some(archive_entry.to_owned()),
                    format!(
                        "page {}: {:?} at 0x{:x}: incomplete support for {}",
                        page_uuid,
                        object.object_type,
                        object.payload_offset,
                        unsupported.join(", ")
                    ),
                );
            }
            if let PageElement::Shape(shape) = &mut element
                && let Some(text) = &mut shape.text
            {
                decode_text_images(text, media, archive_entry, report)?;
            }
            content = Some(PageObjectContent::Element(element));
        } else if object.object_type == ObjectType::Container {
            if base.is_none() {
                report.warning(
                    DiagnosticCode::UnsupportedContainerFeature,
                    Some(archive_entry.to_owned()),
                    format!("page {page_uuid}: container at 0x{:x}: unreadable common metadata; retaining child order without verified parent visibility or render-layer selection", object.payload_offset),
                );
            }
            content = Some(PageObjectContent::Container(Vec::new()));
        } else if !matches!(object.object_type, ObjectType::Other(_)) {
            report.warning(
                DiagnosticCode::UnsupportedObjectType,
                Some(archive_entry.to_owned()),
                format!(
                    "page {}: {:?} (type {}) at 0x{:x}: payload retained without semantic decoding; child records are traversed separately",
                    page_uuid,
                    object.object_type,
                    object.object_type.raw(),
                    object.payload_offset
                ),
            );
        }
        let mut children = decode_objects(
            data,
            &object.children,
            page_uuid,
            media,
            limits,
            archive_entry,
            report,
        )?;
        if let Some(mut content) = content {
            if let PageObjectContent::Container(container) = &mut content {
                *container = std::mem::take(&mut children);
            }
            let render_layer = if let Some(base) = base.as_ref()
                && base.declares_render_layer()
            {
                let metadata = base.flexible_metadata_with_limits(limits)?;
                metadata.render_layer().unwrap_or_else(|| {
                    report.warning(
                        DiagnosticCode::UnresolvedObjectRenderLayer,
                        Some(archive_entry.to_owned()),
                        format!("page {page_uuid}: object at 0x{:x}: declared render-layer field could not be decoded after unsupported common field {:?}", object.payload_offset, metadata.first_unparsed_field),
                    );
                    ObjectRenderLayer::Unresolved
                })
            } else {
                ObjectRenderLayer::Base
            };
            decoded_objects.push(PageObject {
                render_layer,
                source_offset: Some(object.payload_offset),
                content,
            });
        }
        decoded_objects.extend(children);
    }
    Ok(decoded_objects)
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
                page.background.template_uri = Some(fields.read_utf16_u16("template URI")?);
            }
            3 => page.background.image_id = Some(fields.read_u32("background image ID")?),
            4 => page.background.image_mode = Some(fields.read_u32("background image mode")?),
            6 => page.background.width = Some(fields.read_u32("background width")?),
            7 => page.background.rotation = Some(fields.read_u32("background rotation")?),
            5 => {
                let argb = fields.read_u32("background color")?;
                page.background.color_argb = Some(argb);
                page.background_color = Some(Color {
                    r: (argb >> 16) as u8,
                    g: (argb >> 8) as u8,
                    b: argb as u8,
                });
            }
            8 => {
                let count = usize::from(fields.read_u16("PDF record count")?);
                let bytes = fields.read_bytes(count * 24, "PDF records")?;
                let mut records = Reader::new(bytes, "PDF records");
                let mut pdf_paper = Vec::with_capacity(count);
                for _ in 0..count {
                    let media_id = records.read_i32("PDF media ID")?;
                    let page_index = records.read_i32("PDF page index")?;
                    let raw: [u8; 16] = records
                        .read_bytes(16, "PDF rectangle")?
                        .try_into()
                        .expect("reader returned exactly 16 bytes");
                    let words = raw.as_chunks::<4>().0;
                    let rectangle = match header.format_version {
                        Some(2034..) => PdfPaperRectangle::Integer(std::array::from_fn(|index| {
                            i32::from_le_bytes(words[index])
                        })),
                        Some(_) => {
                            PdfPaperRectangle::LegacyFloatBits(std::array::from_fn(|index| {
                                u32::from_le_bytes(words[index])
                            }))
                        }
                        None => PdfPaperRectangle::Unspecified(raw),
                    };
                    pdf_paper.push(PdfPaperRecord {
                        media_id,
                        page_index,
                        rectangle,
                    });
                }
                pdf_page_index = pdf_paper.first().map(|record| record.page_index as u32);
                page.background.pdf_paper = Some(pdf_paper);
            }
            9 => {
                let id = fields.read_u32("template type")?;
                page.background.template_type = Some(id);
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
    id != 0
}
