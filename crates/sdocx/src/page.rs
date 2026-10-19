use crate::binary::Reader;
use crate::decode::decode_stroke;
use crate::error::{Error, Result};
use crate::image::{decode_image, decode_text_images};
use crate::media::MediaResolver;
use crate::note::parse_page_text_box;
use crate::object::read_bbox;
use crate::progress::WorkProgress;
use crate::report::{DiagnosticCode, ParseReport};
use crate::shape::{decode_line, decode_shape};
use crate::storage::{StoredObject, StoredPage};
use crate::types::{
    BoundingBox, Color, ObjectType, Page, PageElement, PageTemplate, PageTemplateSource,
    PdfPaperRecord, PdfPaperRectangle,
};
use crate::{ParseLimits, Progress, ProgressStage};

pub(crate) fn parse_page(
    data: &[u8],
    stored: &StoredPage,
    limits: &ParseLimits,
    archive_entry: &str,
    report: &mut ParseReport,
    media: &MediaResolver,
    document_metadata: &crate::DocumentMetadata,
    stroke_resources: &crate::StrokeResources,
    observer: &mut dyn FnMut(Progress),
) -> Result<Page> {
    let header = &stored.header;
    let stroke_count = stored
        .layers
        .layers
        .iter()
        .try_fold(0_usize, |count, layer| {
            count_strokes(&layer.objects, data, limits)
                .map(|layer_count| count.saturating_add(layer_count))
        })?;
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
    let total = count_objects(&current_layer.objects);
    let mut decoder = PageDecoder {
        page_uuid: &page.uuid,
        media,
        limits,
        archive_entry,
        report,
        document_metadata,
        stroke_resources,
        progress: WorkProgress::new(observer, ProgressStage::Objects, total),
    };
    page.objects = decoder.decode_objects(data, &current_layer.objects)?;
    Ok(page)
}

fn count_strokes(objects: &[StoredObject], data: &[u8], limits: &ParseLimits) -> Result<usize> {
    objects.iter().try_fold(0_usize, |count, object| {
        let own_count = match object.object_type {
            ObjectType::Stroke => 1,
            ObjectType::Formula => {
                let formula = object.formula_metadata_with_limits(data, limits)?;
                formula
                    .strokes
                    .len()
                    .saturating_add(formula.answer_strokes.len())
            }
            _ => 0,
        };
        let child_count = count_strokes(&object.children, data, limits)?;
        Ok(count.saturating_add(own_count).saturating_add(child_count))
    })
}

fn count_objects(objects: &[StoredObject]) -> usize {
    objects
        .iter()
        .map(|object| 1 + count_objects(&object.children))
        .sum()
}

struct PageDecoder<'a, 'p> {
    page_uuid: &'a str,
    media: &'a MediaResolver,
    limits: &'a ParseLimits,
    archive_entry: &'a str,
    report: &'a mut ParseReport,
    document_metadata: &'a crate::DocumentMetadata,
    stroke_resources: &'a crate::StrokeResources,
    progress: WorkProgress<'p>,
}

impl PageDecoder<'_, '_> {
    fn decode_objects(
        &mut self,
        data: &[u8],
        objects: &[StoredObject],
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
                self.progress.advance(1 + count_objects(&object.children));
                continue;
            }
            let mut content = None;
            if object.object_type == ObjectType::Stroke {
                let stroke = decode_stroke(payload, self.limits).map_err(|error| match error {
                    Error::Format(message) => Error::Format(format!(
                        "page {}: stroke at 0x{:x}: {message}",
                        self.page_uuid, object.payload_offset
                    )),
                    error => error,
                })?;
                content = Some(PageObjectContent::Stroke(stroke));
            } else if object.object_type == ObjectType::TextBox {
                let mut decoded =
                    parse_page_text_box(payload, self.limits).map_err(|error| match error {
                        Error::Format(message) => Error::Format(format!(
                            "page {}: text box at 0x{:x}: {message}",
                            self.page_uuid, object.payload_offset
                        )),
                        error => error,
                    })?;
                decode_text_images(
                    &mut decoded.text_box,
                    self.media,
                    self.archive_entry,
                    self.report,
                )?;
                if !decoded.unsupported.is_empty() {
                    self.report.warning(
                        DiagnosticCode::UnsupportedTextBoxFeature,
                        Some(self.archive_entry.to_owned()),
                        format!(
                            "page {}: text box at 0x{:x}: incomplete semantic support for {}",
                            self.page_uuid,
                            object.payload_offset,
                            decoded.unsupported.join(", ")
                        ),
                    );
                }
                check_limit(
                    "objects per page",
                    self.limits.max_objects_per_page,
                    decoded_objects.len() + 1,
                )?;
                content = Some(PageObjectContent::Element(PageElement::TextBox(
                    decoded.text_box,
                )));
            } else if object.object_type == ObjectType::Image {
                let decoded = decode_image(payload).map_err(|error| match error {
                    Error::Format(message) => Error::Format(format!(
                        "page {}: image at 0x{:x}: {message}",
                        self.page_uuid, object.payload_offset
                    )),
                    error => error,
                })?;
                let location = format!(
                    "page {}: image at 0x{:x}",
                    self.page_uuid, object.payload_offset
                );
                if let Some(image) =
                    decoded.resolve(self.media, self.archive_entry, &location, self.report)
                {
                    content = Some(PageObjectContent::Element(PageElement::PlacedImage(image)));
                }
            } else if matches!(object.object_type, ObjectType::Shape | ObjectType::Line) {
                let decoded = if object.object_type == ObjectType::Shape {
                    decode_shape(payload, self.limits)
                        .map(|decoded| (PageElement::Shape(decoded.value), decoded.unsupported))
                } else {
                    decode_line(payload)
                        .map(|decoded| (PageElement::Line(decoded.value), decoded.unsupported))
                };
                let (mut element, unsupported) = decoded.map_err(|error| match error {
                    Error::Format(message) => Error::Format(format!(
                        "page {}: {:?} at 0x{:x}: {message}",
                        self.page_uuid, object.object_type, object.payload_offset
                    )),
                    error => error,
                })?;
                if !unsupported.is_empty() {
                    self.report.warning(
                        DiagnosticCode::UnsupportedShapeFeature,
                        Some(self.archive_entry.to_owned()),
                        format!(
                            "page {}: {:?} at 0x{:x}: incomplete support for {}",
                            self.page_uuid,
                            object.object_type,
                            object.payload_offset,
                            unsupported.join(", ")
                        ),
                    );
                }
                if let PageElement::Shape(shape) = &mut element
                    && let Some(text) = &mut shape.text
                {
                    decode_text_images(text, self.media, self.archive_entry, self.report)?;
                }
                content = Some(PageObjectContent::Element(element));
            } else if object.object_type == ObjectType::Container {
                if base.is_none() {
                    self.report.warning(
                    DiagnosticCode::UnsupportedContainerFeature,
                    Some(self.archive_entry.to_owned()),
                    format!("page {}: container at 0x{:x}: unreadable common metadata; retaining child order without verified parent visibility or render-layer selection", self.page_uuid, object.payload_offset),
                );
                }
                content = Some(PageObjectContent::Container(Vec::new()));
            } else if object.object_type == ObjectType::Formula {
                let mut formula = object
                    .formula_metadata_with_limits(data, self.limits)
                    .map_err(|error| match error {
                        Error::Format(message) => Error::Format(format!(
                            "page {}: formula at 0x{:x}: {message}",
                            self.page_uuid, object.payload_offset
                        )),
                        error => error,
                    })?;
                self.stroke_resources.resolve_formula(&mut formula);
                match formula.select_render_source(self.document_metadata, false, None) {
                    crate::formula::FormulaRenderSelection::StrokeFallback { strokes } => {
                        content = Some(PageObjectContent::Container(
                            strokes
                                .into_iter()
                                .map(|formula_stroke| PageObject {
                                    render_layer: ObjectRenderLayer::Base,
                                    source_offset: Some(object.payload_offset),
                                    content: PageObjectContent::Stroke(
                                        formula_stroke.stroke.clone(),
                                    ),
                                })
                                .collect(),
                        ));
                    }
                    crate::formula::FormulaRenderSelection::Hidden => {}
                    crate::formula::FormulaRenderSelection::ImageSelected {
                        result_rect: Some(result_rect),
                        ..
                    } => {
                        let bounds = crate::formula::bounds::formula_drawn_bounds(
                            &formula,
                            self.stroke_resources.is_complete(),
                        );
                        let media_id = formula
                            .latex_image_media_id
                            .and_then(|id| u32::try_from(id).ok());
                        let resolved_media =
                            media_id.and_then(|id| self.media.resolve(Some(id)).ok());
                        let placement = bounds.zip(resolved_media).and_then(
                            |([left, top, _, _], (media_index, _))| {
                                let rect = [
                                    left + result_rect.x_min as f32,
                                    top + result_rect.y_min as f32,
                                    left + result_rect.x_max as f32,
                                    top + result_rect.y_max as f32,
                                ];
                                rect.iter().all(|coord| coord.is_finite()).then_some((
                                    media_index,
                                    BoundingBox {
                                        x_min: f64::from(rect[0]),
                                        y_min: f64::from(rect[1]),
                                        x_max: f64::from(rect[2]),
                                        y_max: f64::from(rect[3]),
                                    },
                                ))
                            },
                        );
                        if let Some((media_index, bbox)) = placement {
                            content = Some(PageObjectContent::Element(PageElement::FormulaImage(
                                crate::PlacedImage {
                                    bbox,
                                    rotation_degrees: None,
                                    media_id,
                                    media_index: Some(media_index),
                                    crop_rect: None,
                                    original_bbox: None,
                                    border_media_id: None,
                                    original_media_id: None,
                                },
                            )));
                        } else {
                            self.report.warning(
                                DiagnosticCode::UnsupportedObjectType,
                                Some(self.archive_entry.to_owned()),
                                format!("page {}: Formula at 0x{:x}: native bitmap selected but its page origin cannot be derived from resolved visible child bounds and result rectangle", self.page_uuid, object.payload_offset),
                            );
                        }
                    }
                    crate::formula::FormulaRenderSelection::ImageSelected { .. }
                    | crate::formula::FormulaRenderSelection::CachedImageSelected { .. }
                    | crate::formula::FormulaRenderSelection::Unsupported(_) => {
                        self.report.warning(
                            DiagnosticCode::UnsupportedObjectType,
                            Some(self.archive_entry.to_owned()),
                            format!(
                                "page {}: Formula at 0x{:x}: image route or media decode is not available to the page renderer; source retained in stored page data when requested",
                                self.page_uuid, object.payload_offset
                            ),
                        );
                    }
                }
            } else if !matches!(object.object_type, ObjectType::Other(_)) {
                self.report.warning(
                DiagnosticCode::UnsupportedObjectType,
                Some(self.archive_entry.to_owned()),
                format!(
                    "page {}: {:?} (type {}) at 0x{:x}: payload retained without semantic decoding; child records are traversed separately",
                    self.page_uuid,
                    object.object_type,
                    object.object_type.raw(),
                    object.payload_offset
                ),
            );
            }
            let mut children = self.decode_objects(data, &object.children)?;
            if let Some(mut content) = content {
                if let PageObjectContent::Container(container) = &mut content {
                    container.append(&mut children);
                }
                let render_layer = if let Some(base) = base.as_ref()
                    && base.declares_render_layer()
                {
                    let metadata = base.flexible_metadata_with_limits(self.limits)?;
                    metadata.render_layer().unwrap_or_else(|| {
                    self.report.warning(
                        DiagnosticCode::UnresolvedObjectRenderLayer,
                        Some(self.archive_entry.to_owned()),
                        format!("page {}: object at 0x{:x}: declared render-layer field could not be decoded after unsupported common field {:?}", self.page_uuid, object.payload_offset, metadata.first_unparsed_field),
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
            if object.object_type != ObjectType::Container {
                decoded_objects.extend(children.into_iter().map(|mut child| {
                    child.content = PageObjectContent::Descendant(Box::new(child.content));
                    child
                }));
            } else {
                decoded_objects.extend(children);
            }
            self.progress.advance(1);
        }
        Ok(decoded_objects)
    }
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

#[cfg(test)]
mod provenance_tests {
    use super::*;
    use crate::{ObjectRenderLayer, PageObjectContent, ProgressStage, StoredObject};
    use std::collections::HashSet;

    fn frame(kind: i16, properties: u8, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
        let offset = 17 + fixed.len();
        let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        bytes.extend_from_slice(&[1, properties, 4]);
        bytes.extend_from_slice(&fields.to_le_bytes());
        bytes.extend_from_slice(fixed);
        bytes.extend_from_slice(flexible);
        bytes
    }

    fn native_image_payload() -> Vec<u8> {
        let mut base_fixed = 5500_u32.to_le_bytes().to_vec();
        base_fixed.extend_from_slice(&0_u16.to_le_bytes());
        base_fixed.extend_from_slice(&1234_i64.to_le_bytes());
        for value in [0.0_f64, 0.0, 20.0, 20.0] {
            base_fixed.extend_from_slice(&value.to_le_bytes());
        }
        base_fixed.extend_from_slice(&0_i32.to_le_bytes());
        base_fixed.push(0);
        let mut bytes = frame(0, 9, 1, &base_fixed, &0.0_f32.to_le_bytes());
        bytes.extend(frame(6, 0, 0, &[], &[]));
        let mut shape_fixed = 4_u32.to_le_bytes().to_vec();
        for value in [0.0_f64, 0.0, 20.0, 20.0] {
            shape_fixed.extend_from_slice(&value.to_le_bytes());
        }
        shape_fixed.extend_from_slice(&0.0_f32.to_le_bytes());
        shape_fixed.extend_from_slice(&0_u32.to_le_bytes());
        shape_fixed.push(0);
        bytes.extend(frame(7, 0, 0, &shape_fixed, &[]));
        bytes.extend(frame(3, 0, 0, &[], &[]));
        bytes
    }

    fn object(
        object_type: ObjectType,
        payload_size: usize,
        children: Vec<StoredObject>,
    ) -> StoredObject {
        StoredObject {
            object_type,
            declared_size: u32::try_from(payload_size + 32).unwrap(),
            payload_offset: 0,
            payload_size,
            integrity_trailer: [0; 32],
            children,
        }
    }

    fn decode(objects: &[StoredObject], data: &[u8]) -> Vec<crate::PageObject> {
        let media = MediaResolver::new(None, &[], &HashSet::new());
        let limits = ParseLimits::default();
        let document_metadata = crate::DocumentMetadata::default();
        let stroke_resources = crate::StrokeResources::default();
        let mut report = ParseReport::default();
        let mut observer = |_| {};
        let mut decoder = PageDecoder {
            page_uuid: "provenance-test",
            media: &media,
            limits: &limits,
            archive_entry: "page.page",
            report: &mut report,
            document_metadata: &document_metadata,
            stroke_resources: &stroke_resources,
            progress: WorkProgress::new(&mut observer, ProgressStage::Objects, 2),
        };
        decoder.decode_objects(data, objects).unwrap()
    }

    #[test]
    fn decoder_marks_promoted_image_as_descendant_while_root_keeps_boundary() {
        let payload = native_image_payload();
        let image = object(ObjectType::Image, payload.len(), Vec::new());
        let direct = decode(std::slice::from_ref(&image), &payload);
        assert!(matches!(
            direct[0].content,
            PageObjectContent::Element(PageElement::PlacedImage(_))
        ));
        assert_eq!(direct[0].render_layer, ObjectRenderLayer::Base);
        #[cfg(feature = "render")]
        assert_eq!(
            direct[0].standard_batch_boundary(),
            Some(crate::composition::StandardBatchBoundary::Image)
        );

        let parent = object(ObjectType::Other(250), 0, vec![image]);
        let promoted = decode(&[parent], &payload);
        assert!(matches!(
            promoted[0].content.leaf(),
            PageObjectContent::Element(PageElement::PlacedImage(_))
        ));
        assert!(matches!(
            promoted[0].content,
            PageObjectContent::Descendant(_)
        ));
        assert_eq!(promoted[0].render_layer, ObjectRenderLayer::Base);
        #[cfg(feature = "render")]
        assert_eq!(promoted[0].standard_batch_boundary(), None);
    }
}
