#![cfg(feature = "pdf")]

#[path = "support/mod.rs"]
mod support;

use lopdf::{
    Dictionary, Document as PdfDocument, Object, ObjectId, Stream, content::Content, dictionary,
};
use sdocx::{
    ArchiveResource, ArchiveResourceContent, BoundingBox, Document, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, PageElement, PdfError, PdfOptions, PdfPaperRectangle, RichTextBox,
    RichTextObjectSpan, RichTextParagraph, RichTextParagraphType, RichTextSpan, RichTextSpanType,
    SpanIntervalType, fonts::FontBook, parse_media_manifest_bytes,
    render_layout_pages_pdf_detailed_with_fonts,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
};

const NOTE_WIDTH: u32 = 1812;
const NOTE_HEIGHT: u32 = 15_372;
const SEGMENT_HEIGHT: u32 = 1527;
const PDF_BINDING: u32 = 42;
const SOURCE_WIDTH: f32 = 726.204_3;
const SOURCE_HEIGHT: f32 = 595.440_06;

fn media_manifest_entries(entries: &[(u32, &str)]) -> sdocx::MediaManifest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&5500_u32.to_le_bytes());
    bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (binding, file_name) in entries {
        let mut record = Vec::new();
        record.extend_from_slice(&binding.to_le_bytes());
        record.extend_from_slice(&(file_name.len() as u16).to_le_bytes());
        for unit in file_name.encode_utf16() {
            record.extend_from_slice(&unit.to_le_bytes());
        }
        record.extend_from_slice(&[0, 0]);
        record.extend_from_slice(&1_u16.to_le_bytes());
        record.extend_from_slice(&0_i64.to_le_bytes());
        record.push(0);
        bytes.extend_from_slice(&(record.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&record);
    }
    bytes.extend_from_slice(b"EOFX");
    parse_media_manifest_bytes(&bytes).expect("test manifest is valid")
}

fn media_manifest() -> sdocx::MediaManifest {
    media_manifest_entries(&[(PDF_BINDING, "paper.pdf")])
}

fn bbox(x0: f64, y0: f64, x1: f64, y1: f64) -> BoundingBox {
    BoundingBox {
        x_min: x0,
        y_min: y0,
        x_max: x1,
        y_max: y1,
    }
}

fn frame(kind: i16, properties: u16, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 18 + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![2],
        properties.to_le_bytes().to_vec(),
        vec![4],
        fields.to_le_bytes().to_vec(),
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

fn encoded_stroke(rotation_degrees: Option<f32>, generated: bool) -> Vec<u8> {
    // Two uncompressed samples cross the first 1485-unit source-page edge.
    let stroke_box = bbox(100.0, 1460.0, 300.0, 1550.0);
    let mut base = 5500_u32.to_le_bytes().to_vec();
    let uuid = "00000000-0000-0000-0000-000000000042";
    base.extend_from_slice(&(uuid.len() as u16).to_le_bytes());
    base.extend_from_slice(uuid.as_bytes());
    base.extend_from_slice(&1_i64.to_le_bytes());
    for value in [
        stroke_box.x_min,
        stroke_box.y_min,
        stroke_box.x_max,
        stroke_box.y_max,
    ] {
        base.extend_from_slice(&value.to_le_bytes());
    }
    base.extend_from_slice(&0_i32.to_le_bytes());
    base.push(0);
    let mut base_flexible = Vec::new();
    if let Some(rotation) = rotation_degrees {
        base_flexible.extend_from_slice(&rotation.to_le_bytes());
    }
    let base_frame = frame(
        0,
        1 << 3,
        0x6000 | u32::from(rotation_degrees.is_some()),
        &base,
        &base_flexible,
    );

    let mut channels = Vec::new();
    for value in [100.0_f64, 1460.0, 300.0, 1550.0] {
        channels.extend_from_slice(&value.to_le_bytes());
    }
    for value in [0.8_f32, 0.8] {
        channels.extend_from_slice(&value.to_le_bytes());
    }
    for value in [100_i32, 110] {
        channels.extend_from_slice(&value.to_le_bytes());
    }
    let mut fixed = 2_u16.to_le_bytes().to_vec();
    fixed.extend(channels);
    fixed.extend_from_slice(&[1, 0]);
    let mut style = 0xff20_6090_u32.to_le_bytes().to_vec();
    style.extend_from_slice(&8.0_f32.to_le_bytes());
    let property_mask = if generated { 0 } else { 1 << 10 };
    let stroke_frame = frame(1, property_mask, (1 << 2) | (1 << 3), &fixed, &style);
    [base_frame, stroke_frame].concat()
}

fn source_pdf_pages(rotate: i64, user_unit: i64, sizes: &[[f32; 2]]) -> Vec<u8> {
    let mut pdf = PdfDocument::with_version("1.7");
    let pages_id = pdf.new_object_id();
    let image_id = pdf.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => 1,
            "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
        },
        vec![0x31, 0x62, 0x93],
    ));
    let content_id = pdf.add_object(Stream::new(
        Dictionary::new(),
        b"q 1 0 0 1 70 80 cm /Im0 Do Q\n0.1 0.2 0.8 RG 2 w 8 12 m 700 560 l S\n".to_vec(),
    ));
    let mut kids = Vec::new();
    for [width, height] in sizes {
        let page_id = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), f64::from(*width).into(), f64::from(*height).into()],
            "Rotate" => rotate, "UserUnit" => user_unit,
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image_id } },
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    pdf.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => kids, "Count" => sizes.len() as i64,
        }),
    );
    let root_id = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    pdf.trailer.set("Root", root_id);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

fn document_with_records(
    paper: bool,
    rotation: Option<f32>,
    source_rotation: i64,
    user_unit: i64,
    generated: bool,
    saved_records: &[[i32; 6]],
    source_sizes: &[[f32; 2]],
) -> Document {
    let mut records = Vec::new();
    if paper {
        // The native default page color is opaque near-white, not the parser's
        // implicit ARGB zero value.
        records.extend_from_slice(&0xfffc_fcfc_u32.to_le_bytes());
        records.extend_from_slice(&(saved_records.len() as u16).to_le_bytes());
        for record in saved_records {
            for value in record {
                records.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    let object = support::object(1, &encoded_stroke(rotation, generated), &[]);
    if paper {
        records.extend_from_slice(&0_u32.to_le_bytes()); // template type follows PDF records
    }
    let field_mask = if paper {
        (1 << 5) | (1 << 8) | (1 << 9)
    } else {
        0
    };
    let raw_page = support::page(&[vec![object]], field_mask, &records);
    let mut document = sdocx::parse_bytes(&support::archive(&raw_page)).unwrap();
    assert!(matches!(
        document.pages[0].objects[0].content,
        sdocx::PageObjectContent::Stroke(ref stroke)
            if stroke
                .rendering
                .as_ref()
                .is_some_and(|rendering| rendering.properties.generated == generated)
    ));
    document.pages[0].width = NOTE_WIDTH;
    document.pages[0].height = NOTE_HEIGHT;
    document.metadata.page_mode = Some(1);
    document.metadata.default_page_dimensions = Some((1080, SEGMENT_HEIGHT));
    document.metadata.page_dimensions = Some((NOTE_WIDTH, NOTE_HEIGHT));
    document.metadata.flow_dimensions = Some((1080, 9162));
    document.metadata.flow_page_padding = Some((0, 0));
    document.metadata.orientation = Some(0);
    if paper {
        document.metadata.archive_resources = vec![ArchiveResource {
            name: "media/paper.pdf".into(),
            archive_id: None,
            content: ArchiveResourceContent::Opaque {
                data: source_pdf_pages(source_rotation, user_unit, source_sizes),
            },
        }];
        document.metadata.media_manifest = Some(media_manifest());
    }
    document
}

fn document_with_generated(
    paper: bool,
    rotation: Option<f32>,
    source_rotation: i64,
    user_unit: i64,
    generated: bool,
) -> Document {
    document_with_records(
        paper,
        rotation,
        source_rotation,
        user_unit,
        generated,
        &[[PDF_BINDING as i32, 0, 0, 0, NOTE_WIDTH as i32, 1485]],
        &[[SOURCE_WIDTH, SOURCE_HEIGHT]],
    )
}

fn document(paper: bool, rotation: Option<f32>, source_rotation: i64, user_unit: i64) -> Document {
    document_with_generated(paper, rotation, source_rotation, user_unit, false)
}

fn export(document: &Document, indices: &[usize], dpi: f32) -> Result<sdocx::PdfOutput, PdfError> {
    export_with_fixed_axis(document, indices, dpi, None)
}

fn export_with_fixed_axis(
    document: &Document,
    indices: &[usize],
    dpi: f32,
    fixed_axis_size: Option<u32>,
) -> Result<sdocx::PdfOutput, PdfError> {
    let layout = sdocx::layout_document(document);
    let mut options = PdfOptions::default();
    options.dpi = dpi;
    options.pdf_paper_fixed_axis_size = fixed_axis_size;
    render_layout_pages_pdf_detailed_with_fonts(
        document,
        &layout,
        indices,
        &Default::default(),
        &options,
        &FontBook::default(),
    )
}

fn rich_text(text: &str, object_spans: Vec<RichTextObjectSpan>) -> RichTextBox {
    let length = text.encode_utf16().count() as u32;
    let paragraph_count = text.chars().filter(|character| *character == '\n').count() as u32 + 1;
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: text.to_owned(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: None,
        runs: Vec::new(),
        spans: vec![
            RichTextSpan {
                kind: RichTextSpanType::FontSize,
                start_utf16: 0,
                end_utf16: length,
                interval_type: SpanIntervalType::ClosedOpen,
                payload: 18.0_f32.to_le_bytes().to_vec(),
            },
            RichTextSpan {
                kind: RichTextSpanType::ForegroundColor,
                start_utf16: 0,
                end_utf16: length,
                interval_type: SpanIntervalType::ClosedOpen,
                payload: 0xff00_0000_u32.to_le_bytes().to_vec(),
            },
        ],
        paragraphs: vec![
            RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: paragraph_count,
                payload: 0_u32.to_le_bytes().to_vec(),
            },
            RichTextParagraph {
                kind: RichTextParagraphType::ParsingState,
                start_paragraph: 0,
                end_paragraph: paragraph_count,
                payload: 1_u32.to_le_bytes().to_vec(),
            },
        ],
        object_spans,
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn page_box(pdf: &PdfDocument, page_number: u32) -> [f64; 4] {
    let id = pdf.get_pages()[&page_number];
    let values = pdf
        .get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap();
    values
        .iter()
        .map(|value| {
            value
                .as_float()
                .map(f64::from)
                .or_else(|_| value.as_i64().map(|integer| integer as f64))
                .unwrap()
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PdfStreamBytes {
    encoded: Vec<u8>,
    decoded: Vec<u8>,
}

fn page_content_streams(
    pdf: &PdfDocument,
    page_id: ObjectId,
) -> Result<Vec<PdfStreamBytes>, String> {
    fn append(
        pdf: &PdfDocument,
        object: &Object,
        out: &mut Vec<PdfStreamBytes>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("PDF Contents nesting exceeds 64".into());
        }
        match object {
            Object::Reference(id) => append(
                pdf,
                pdf.get_object(*id).map_err(|error| error.to_string())?,
                out,
                depth + 1,
            ),
            Object::Array(items) => {
                for item in items {
                    append(pdf, item, out, depth + 1)?;
                }
                Ok(())
            }
            Object::Stream(stream) => {
                out.push(PdfStreamBytes {
                    encoded: stream.content.clone(),
                    decoded: stream
                        .decompressed_content()
                        .map_err(|error| error.to_string())?,
                });
                Ok(())
            }
            other => Err(format!("unsupported PDF Contents object: {other:?}")),
        }
    }

    let page = pdf
        .get_dictionary(page_id)
        .map_err(|error| error.to_string())?;
    let contents = page.get(b"Contents").map_err(|error| error.to_string())?;
    let mut out = Vec::new();
    append(pdf, contents, &mut out, 0)?;
    Ok(out)
}

type PdfResourceSignatures = BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, String>>;

fn pdf_object_signature(
    pdf: &PdfDocument,
    object: &Object,
    active: &mut HashSet<ObjectId>,
) -> Result<String, String> {
    match object {
        Object::Reference(id) => {
            if !active.insert(*id) {
                return Ok("cycle".into());
            }
            let result = pdf_object_signature(
                pdf,
                pdf.get_object(*id).map_err(|error| error.to_string())?,
                active,
            );
            active.remove(id);
            result
        }
        Object::Null => Ok("null".into()),
        Object::Boolean(value) => Ok(format!("bool:{value}")),
        Object::Integer(value) => Ok(format!("int:{value}")),
        Object::Real(value) => Ok(format!("real:{:08x}", value.to_bits())),
        Object::Name(value) => Ok(format!("name:{value:?}")),
        Object::String(value, format) => Ok(format!("string:{format:?}:{value:?}")),
        Object::Array(items) => Ok(format!(
            "array:[{}]",
            items
                .iter()
                .map(|item| pdf_object_signature(pdf, item, active))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        Object::Dictionary(dict) => {
            let mut entries = dict.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut values = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                values.push(format!(
                    "{key:?}={}",
                    pdf_object_signature(pdf, value, active)?
                ));
            }
            Ok(format!("dict:{{{}}}", values.join(",")))
        }
        Object::Stream(stream) => {
            let mut dictionary = stream.dict.clone();
            dictionary.remove(b"Length");
            let dictionary = pdf_object_signature(pdf, &Object::Dictionary(dictionary), active)?;
            let encoded_hash = Sha256::digest(&stream.content);
            // Preserve encoded bytes and the complete filter dictionary;
            // some real image filters are intentionally not decodable here.
            Ok(format!("stream:{dictionary}:encoded={encoded_hash:x}"))
        }
    }
}

fn pdf_page_group_signature(
    pdf: &PdfDocument,
    page_id: ObjectId,
) -> Result<Option<String>, String> {
    let page = pdf
        .get_dictionary(page_id)
        .map_err(|error| error.to_string())?;
    page.get(b"Group")
        .ok()
        .map(|group| pdf_object_signature(pdf, group, &mut HashSet::new()))
        .transpose()
}

fn pdf_resource_signatures(
    pdf: &PdfDocument,
    page_id: ObjectId,
) -> Result<PdfResourceSignatures, String> {
    fn dereference<'a>(pdf: &'a PdfDocument, mut object: &'a Object) -> Result<&'a Object, String> {
        let mut seen = HashSet::new();
        loop {
            match object {
                Object::Reference(id) => {
                    if !seen.insert(*id) {
                        return Err("cyclic PDF resource reference".into());
                    }
                    object = pdf.get_object(*id).map_err(|error| error.to_string())?;
                }
                _ => return Ok(object),
            }
        }
    }

    fn inherited_resource(pdf: &PdfDocument, page_id: ObjectId) -> Result<Option<&Object>, String> {
        let mut current_id = page_id;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(current_id) {
                return Err("cyclic PDF page Parent chain".into());
            }
            let page = pdf
                .get_dictionary(current_id)
                .map_err(|error| error.to_string())?;
            if let Ok(resources) = page.get(b"Resources") {
                return Ok(Some(resources));
            }
            let Ok(parent) = page.get(b"Parent") else {
                return Ok(None);
            };
            current_id = parent.as_reference().map_err(|error| error.to_string())?;
        }
    }

    let Some(resource_object) = inherited_resource(pdf, page_id)? else {
        return Ok(BTreeMap::new());
    };
    let resources = dereference(pdf, resource_object)?
        .as_dict()
        .map_err(|error| error.to_string())?;
    let mut out = BTreeMap::new();
    for (category, category_object) in resources.iter() {
        let mut entries = BTreeMap::new();
        match dereference(pdf, category_object)? {
            Object::Dictionary(category_dict) => {
                for (name, value) in category_dict.iter() {
                    entries.insert(
                        name.clone(),
                        pdf_object_signature(pdf, value, &mut HashSet::new())?,
                    );
                }
            }
            // ProcSet and other non-dictionary resource values are compared as
            // whole values using the empty key, which cannot be a valid name.
            _ => {
                entries.insert(
                    Vec::new(),
                    pdf_object_signature(pdf, category_object, &mut HashSet::new())?,
                );
            }
        }
        out.insert(category.clone(), entries);
    }
    Ok(out)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn page_references_image_size(
    pdf: &PdfDocument,
    page_number: u32,
    width: i64,
    height: i64,
) -> bool {
    let Some(page_id) = pdf.get_pages().get(&page_number).copied() else {
        return false;
    };
    let Some(resources) = pdf
        .get_dictionary(page_id)
        .ok()
        .and_then(|page| page.get(b"Resources").ok())
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    else {
        return false;
    };
    let Some(xobjects) = resources
        .get(b"XObject")
        .ok()
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    else {
        return false;
    };
    let Ok(bytes) = pdf.get_page_content(page_id) else {
        return false;
    };
    let Ok(content) = Content::decode(&bytes) else {
        return false;
    };
    content.operations.iter().any(|operation| {
        operation.operator == "Do"
            && operation
                .operands
                .first()
                .and_then(|object| object.as_name().ok())
                .and_then(|name| xobjects.get(name).ok())
                .and_then(|object| pdf.dereference(object).ok())
                .is_some_and(|(_, object)| {
                    object.as_stream().is_ok_and(|stream| {
                        stream.dict.get(b"Subtype").is_ok_and(|subtype| {
                            subtype.as_name().is_ok_and(|name| name == b"Image")
                        }) && stream
                            .dict
                            .get(b"Width")
                            .is_ok_and(|value| value.as_i64().is_ok_and(|value| value == width))
                            && stream.dict.get(b"Height").is_ok_and(|value| {
                                value.as_i64().is_ok_and(|value| value == height)
                            })
                    })
                })
    })
}

fn number(object: &Object) -> Option<f64> {
    object
        .as_float()
        .map(f64::from)
        .or_else(|_| object.as_i64().map(|value| value as f64))
        .ok()
}

fn point_matches(
    operation: &lopdf::content::Operation,
    operator: &str,
    point: [f64; 2],
    epsilon: f64,
) -> bool {
    operation.operator == operator
        && operation.operands.len() >= 2
        && number(&operation.operands[0]).is_some_and(|value| (value - point[0]).abs() <= epsilon)
        && number(&operation.operands[1]).is_some_and(|value| (value - point[1]).abs() <= epsilon)
}

fn content_has_crossing_stroke(content: &Content) -> bool {
    content_has_stroke_segment(content, [100.0, 1_460.0], [300.0, 1_550.0], 0.0)
}

fn content_has_stroke_segment(
    content: &Content,
    start: [f64; 2],
    end: [f64; 2],
    epsilon: f64,
) -> bool {
    let mut started = false;
    let mut completed = false;
    for operation in &content.operations {
        if point_matches(operation, "m", start, epsilon) {
            started = true;
            completed = false;
        } else if started && point_matches(operation, "l", end, epsilon) {
            completed = true;
        } else if completed && operation.operator == "S" {
            return true;
        } else if matches!(
            operation.operator.as_str(),
            "m" | "l" | "c" | "v" | "y" | "re"
        ) {
            started = false;
            completed = false;
        }
    }
    false
}

fn form_has_stroke_segment(
    pdf: &PdfDocument,
    form: &lopdf::Stream,
    inherited_resources: &Dictionary,
    depth: usize,
    start: [f64; 2],
    end: [f64; 2],
    epsilon: f64,
) -> bool {
    if depth > 16 {
        return false;
    }
    let Ok(bytes) = form.decompressed_content() else {
        return false;
    };
    let Ok(content) = Content::decode(&bytes) else {
        return false;
    };
    if content_has_stroke_segment(&content, start, end, epsilon) {
        return true;
    }

    let resources = form
        .dict
        .get(b"Resources")
        .ok()
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
        .unwrap_or(inherited_resources);
    let Some(xobjects) = resources
        .get(b"XObject")
        .ok()
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    else {
        return false;
    };
    content.operations.iter().any(|operation| {
        operation.operator == "Do"
            && operation
                .operands
                .first()
                .and_then(|object| object.as_name().ok())
                .and_then(|name| xobjects.get(name).ok())
                .and_then(|object| pdf.dereference(object).ok())
                .and_then(|(_, object)| object.as_stream().ok())
                .is_some_and(|nested| {
                    nested
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|subtype| subtype.as_name().is_ok_and(|name| name == b"Form"))
                        && form_has_stroke_segment(
                            pdf,
                            nested,
                            resources,
                            depth + 1,
                            start,
                            end,
                            epsilon,
                        )
                })
    })
}

fn page_invokes_crossing_stroke_form(pdf: &PdfDocument, page_number: u32) -> bool {
    page_invokes_stroke_segment_form(pdf, page_number, [100.0, 1_460.0], [300.0, 1_550.0], 0.0)
}

fn page_invokes_stroke_segment_form(
    pdf: &PdfDocument,
    page_number: u32,
    start: [f64; 2],
    end: [f64; 2],
    epsilon: f64,
) -> bool {
    let Some(page_id) = pdf.get_pages().get(&page_number).copied() else {
        return false;
    };
    let Some(resources) = pdf
        .get_dictionary(page_id)
        .ok()
        .and_then(|page| page.get(b"Resources").ok())
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    else {
        return false;
    };
    let Some(xobjects) = resources
        .get(b"XObject")
        .ok()
        .and_then(|object| pdf.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    else {
        return false;
    };
    let Ok(bytes) = pdf.get_page_content(page_id) else {
        return false;
    };
    let Ok(content) = Content::decode(&bytes) else {
        return false;
    };
    content.operations.iter().any(|operation| {
        operation.operator == "Do"
            && operation
                .operands
                .first()
                .and_then(|object| object.as_name().ok())
                .and_then(|name| xobjects.get(name).ok())
                .and_then(|object| pdf.dereference(object).ok())
                .and_then(|(_, object)| object.as_stream().ok())
                .is_some_and(|form| {
                    form.dict
                        .get(b"Subtype")
                        .is_ok_and(|subtype| subtype.as_name().is_ok_and(|name| name == b"Form"))
                        && form_has_stroke_segment(pdf, form, resources, 0, start, end, epsilon)
                })
    })
}

fn page_has_stroke_segment(
    pdf: &PdfDocument,
    page_number: u32,
    start: [f64; 2],
    end: [f64; 2],
    epsilon: f64,
) -> bool {
    pdf.get_pages()
        .get(&page_number)
        .copied()
        .and_then(|page_id| pdf.get_page_content(page_id).ok())
        .and_then(|bytes| Content::decode(&bytes).ok())
        .is_some_and(|content| content_has_stroke_segment(&content, start, end, epsilon))
}

fn page_has_crossing_stroke(pdf: &PdfDocument, page_number: u32) -> bool {
    let Some(page_id) = pdf.get_pages().get(&page_number).copied() else {
        return false;
    };
    pdf.get_page_content(page_id)
        .ok()
        .and_then(|bytes| Content::decode(&bytes).ok())
        .is_some_and(|content| content_has_crossing_stroke(&content))
        || page_invokes_crossing_stroke_form(pdf, page_number)
}

#[test]
fn public_pdf_export_preserves_source_page_resources_and_native_physical_continuations() {
    let document = document(true, None, 0, 1);
    let layout = sdocx::layout_document(&document);
    assert_eq!(layout.pages.len(), 1);
    let output = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[0],
        &Default::default(),
        &PdfOptions::default(),
        &FontBook::default(),
    )
    .unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), 11);
    assert_eq!(
        page_box(&pdf, 1),
        [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)]
    );
    for page_number in 2..=11 {
        assert_eq!(page_box(&pdf, page_number), [0.0, 0.0, 600.0, 505.0]);
    }
    assert_eq!(output.pages.len(), pages.len());
    assert!(output.pages.iter().all(|page| page.page_index == 0));
    assert!(output.pages.iter().all(|page| page.source_page_index == 0));

    // The source's vector path and image invocation survive once on its imported
    // page; the repeated overlay strokes remain page-local vector content.
    let first_content = Content::decode(&pdf.get_page_content(pages[&1]).unwrap()).unwrap();
    assert_eq!(
        first_content
            .operations
            .iter()
            .filter(|op| op.operator == "S")
            .count(),
        1
    );
    assert_eq!(
        first_content
            .operations
            .iter()
            .filter(|op| {
                op.operator == "Do"
                    && op
                        .operands
                        .last()
                        .is_some_and(|object| object.as_name().is_ok_and(|name| name == b"Im0"))
            })
            .count(),
        1
    );
    let image_count = pdf
        .objects
        .values()
        .filter(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|subtype| subtype.as_name().is_ok_and(|name| name == b"Image"))
            })
        })
        .count();
    assert_eq!(image_count, 1);

    assert!(
        page_invokes_crossing_stroke_form(&pdf, 1),
        "the imported source page should include the first vector-rendered stroke segment"
    );
    let continuation_content = Content::decode(&pdf.get_page_content(pages[&2]).unwrap()).unwrap();
    assert!(
        content_has_crossing_stroke(&continuation_content) || page_has_crossing_stroke(&pdf, 2),
        "the first continuation should paint the other vector-rendered segment of the crossing stroke"
    );
}

#[test]
fn public_pdf_paper_preserves_generated_strokes_with_identical_native_selection_geometry() {
    for generated in [false, true] {
        let document = document_with_generated(true, None, 0, 1, generated);
        let output = export(&document, &[0], 96.0).unwrap();
        let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
        let pages = pdf.get_pages();
        assert_eq!(pages.len(), 11, "generated={generated}");
        assert_eq!(
            page_box(&pdf, 1),
            [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)]
        );
        assert_eq!(page_box(&pdf, 2), [0.0, 0.0, 600.0, 505.0]);

        let source = Content::decode(&pdf.get_page_content(pages[&1]).unwrap()).unwrap();
        assert_eq!(
            source
                .operations
                .iter()
                .filter(|op| op.operator == "S")
                .count(),
            1,
            "original source vector path must remain once, generated={generated}"
        );
        assert_eq!(
            source
                .operations
                .iter()
                .filter(|op| {
                    op.operator == "Do"
                        && op
                            .operands
                            .last()
                            .is_some_and(|object| object.as_name().is_ok_and(|name| name == b"Im0"))
                })
                .count(),
            1,
            "original source image must remain once, generated={generated}"
        );
        assert!(page_invokes_crossing_stroke_form(&pdf, 1));
        assert!(
            page_has_crossing_stroke(&pdf, 2),
            "the crossing saved stroke must remain vector-painted on its continuation, generated={generated}"
        );
    }
}

#[test]
fn public_pdf_paper_physical_boxes_do_not_depend_on_overlay_dpi() {
    let document = document(true, None, 0, 1);
    for dpi in [72.0, 96.0, 144.0] {
        let output = export(&document, &[0], dpi).unwrap();
        let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 11, "dpi={dpi}");
        for page_number in 2..=11 {
            assert_eq!(
                page_box(&pdf, page_number),
                [0.0, 0.0, 600.0, 505.0],
                "dpi={dpi}"
            );
        }
    }
}

#[test]
fn public_pdf_paper_fixed_axis_profile_scales_note_geometry_but_not_source_pdf_boxes() {
    let document = document(true, None, 0, 1);
    let default_output = export(&document, &[0], 96.0).unwrap();
    let explicit_default = export_with_fixed_axis(&document, &[0], 96.0, None).unwrap();
    assert_eq!(explicit_default.bytes, default_output.bytes);

    let fixed_axis = export_with_fixed_axis(&document, &[0], 96.0, Some(1_080)).unwrap();
    let pdf = PdfDocument::load_mem(&fixed_axis.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 7);
    assert_eq!(fixed_axis.pages.len(), 7);
    assert_eq!(
        page_box(&pdf, 1),
        [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)],
        "the imported source PDF remains in its physical page coordinates"
    );
    for page_number in 2..=6 {
        assert_eq!(page_box(&pdf, page_number), [0.0, 0.0, 600.0, 848.0]);
    }
    assert_eq!(page_box(&pdf, 7), [0.0, 0.0, 600.0, 848.0]);
    let scale = 1_080.0_f32 / 1_812.0_f32;
    let start = [f64::from(100.0_f32 * scale), f64::from(1_460.0_f32 * scale)];
    let end = [f64::from(300.0_f32 * scale), f64::from(1_550.0_f32 * scale)];
    assert!(page_invokes_stroke_segment_form(&pdf, 1, start, end, 0.01));
    assert!(page_has_stroke_segment(&pdf, 2, start, end, 0.01));
    assert!(matches!(
        export_with_fixed_axis(&document, &[0], 96.0, Some(31)),
        Err(PdfError::InvalidPdfPaperFixedAxisSize)
    ));
}

#[test]
fn public_pdf_export_without_paper_keeps_the_ordinary_single_page_route() {
    let document = document(false, None, 0, 1);
    let output = export(&document, &[0], 96.0).unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    assert_eq!(output.pages.len(), 1);
    assert_eq!(output.pages[0].page_index, 0);
    assert_eq!(output.pages[0].source_page_index, 0);
}

#[test]
fn public_pdf_paper_accepts_styled_blank_body_and_rejects_visible_inline_image_body() {
    let mut blank = document(true, None, 0, 1);
    let mut body = rich_text("\n".repeat(488).as_str(), Vec::new());
    // Native archive spans retain an eight-byte payload; the value occupies
    // its first four bytes, as decoded by RichTextSpan's typed accessors.
    for span in &mut body.spans {
        span.payload.extend_from_slice(&[0; 4]);
        span.interval_type = SpanIntervalType::ClosedClosed;
    }
    body.color = Some(sdocx::Color {
        r: 37,
        g: 37,
        b: 37,
    });
    body.font_size = Some(17.0);
    body.margins = Some([16.0, 10.0, 16.0, 10.0]);
    body.gravity = Some(0);
    body.text_sections = vec![sdocx::RichTextSection {
        start_utf16: 0,
        length_utf16: 488,
    }];
    blank.metadata.note_text = Some(body);
    let output = export(&blank, &[0], 96.0).unwrap();
    assert_eq!(
        PdfDocument::load_mem(&output.bytes)
            .unwrap()
            .get_pages()
            .len(),
        11
    );

    let mut image_body = document(true, None, 0, 1);
    image_body.metadata.note_text = Some(rich_text(
        "\u{fffc}",
        vec![RichTextObjectSpan {
            object_type: sdocx::ObjectType::Image,
            object_data: Vec::new(),
            content: None,
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }],
    ));
    assert!(matches!(
        export(&image_body, &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
}

#[test]
#[cfg(feature = "serde")]
fn public_pdf_paper_keeps_resolved_image_bodies_and_the_source_pdf_resource() {
    for image_count in [1, 2] {
        let mut document = document(true, None, 0, 1);
        let anchor = 112;
        let mut body = rich_text(&format!("{}\u{fffc}\n\n\n", "\n".repeat(anchor)), vec![]);
        body.font_size = Some(17.0);
        body.margins = Some([16.0, 10.0, 16.0, 10.0]);
        body.gravity = Some(0);
        let placed_image: sdocx::PlacedImage = serde_json::from_value(serde_json::json!({
            "bbox": {
                "x_min": 48.0,
                "y_min": 5473.2021484375,
                "x_max": 567.48206,
                "y_max": 6154.563965,
            },
            "rotation_degrees": null,
            "media_id": 0,
            "media_index": 0,
            "crop_rect": null,
            "original_bbox": null,
            "border_media_id": null,
            "original_media_id": null,
        }))
        .unwrap();
        body.object_spans.push(RichTextObjectSpan {
            object_type: sdocx::ObjectType::Image,
            object_data: Vec::new(),
            content: Some(sdocx::RichTextObjectContent::Image(Box::new(placed_image))),
            text_index_utf16: anchor as i32,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        });
        if image_count == 2 {
            let mut second = body.object_spans[0].clone();
            second.text_index_utf16 = body.text.encode_utf16().count() as i32;
            body.text.push('\u{fffc}');
            let Some(sdocx::RichTextObjectContent::Image(image)) = second.content.as_mut() else {
                panic!("test image span has image content");
            };
            image.bbox = bbox(24.0, 0.0, 44.0, 20.0);
            image.media_id = Some(1);
            image.media_index = Some(1);
            body.object_spans.push(second);
        }
        document.metadata.note_text = Some(body);
        document.metadata.media_assets = vec![sdocx::MediaAsset {
            name: "0.png".to_owned(),
            archive_id: Some(0),
            mime_type: "image/png".to_owned(),
            data: include_bytes!("assets/crop-quadrants.png").to_vec(),
        }];

        if image_count == 2 {
            let mut encoded = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut encoded, 2, 1);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&[0, 0, 255, 0, 255, 0])
                    .unwrap();
            }
            document.metadata.media_assets.push(sdocx::MediaAsset {
                name: "1.png".to_owned(),
                archive_id: Some(1),
                mime_type: "image/png".to_owned(),
                data: encoded,
            });
        }

        let output = export_with_fixed_axis(&document, &[0], 96.0, Some(1_080)).unwrap();
        let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 7);
        assert_eq!(output.pages.len(), 7);
        assert_eq!(
            page_box(&pdf, 1),
            [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)],
            "the retained source PDF keeps its original physical page box"
        );
        assert!(
            (2..=7).any(|page_number| page_references_image_size(&pdf, page_number, 4, 3)),
            "the resolved source PNG must be present in a rendered continuation page"
        );
        if image_count == 2 {
            assert!(
                (2..=7).any(|page_number| page_references_image_size(&pdf, page_number, 2, 1)),
                "the independently resolved second PNG must survive alongside the first"
            );
        }
        assert_eq!(
            document.metadata.media_assets[0].data,
            include_bytes!("assets/crop-quadrants.png"),
            "PDF export must leave the caller-owned PNG bytes intact"
        );
    }
}

#[test]
fn public_pdf_paper_route_rejects_unproven_objects_transforms_and_partial_selection() {
    assert!(matches!(
        export(&document(true, Some(15.0), 0, 1), &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
    assert!(matches!(
        export(&document(true, None, 15, 1), &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
    assert!(matches!(
        export(&document(true, None, 0, 2), &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));

    let mut with_image = document(true, None, 0, 1);
    with_image.pages[0].objects.push(
        PageElement::Image {
            bbox: bbox(10.0, 10.0, 50.0, 50.0),
            media_index: 0,
        }
        .into(),
    );
    assert!(matches!(
        export(&with_image, &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));

    // An incomplete visible-page selection cannot be interpreted as the full
    // document's expanded SingleX output topology.
    let document = document(true, None, 0, 1);
    let mut layout = sdocx::layout_document(&document);
    layout.pages.push(layout.pages[0].clone());
    let result = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[0],
        &Default::default(),
        &PdfOptions::default(),
        &FontBook::default(),
    );
    assert!(matches!(result, Err(PdfError::UnsupportedPdfPaper { .. })));
}

#[test]
fn public_pdf_paper_maps_contiguous_records_to_their_source_pages_and_physical_boxes() {
    let records = [
        [PDF_BINDING as i32, 0, 0, 0, NOTE_WIDTH as i32, 1485],
        [PDF_BINDING as i32, 1, 0, 1485, NOTE_WIDTH as i32, 3012],
        [PDF_BINDING as i32, 2, 0, 3012, NOTE_WIDTH as i32, 4539],
    ];
    let sizes = [
        [SOURCE_WIDTH, SOURCE_HEIGHT],
        [640.0, 480.0],
        [612.0, 792.0],
    ];
    let document = document_with_records(true, None, 0, 1, true, &records, &sizes);
    let output = export(&document, &[0], 96.0).unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 11);
    assert_eq!(output.pages.len(), 11);
    for (index, [width, height]) in sizes.iter().enumerate() {
        assert_eq!(
            page_box(&pdf, index as u32 + 1),
            [0.0, 0.0, f64::from(*width), f64::from(*height)]
        );
    }
    assert_eq!(page_box(&pdf, 4), [0.0, 0.0, 600.0, 505.0]);
    assert!(page_invokes_crossing_stroke_form(&pdf, 1));
    assert!(page_invokes_crossing_stroke_form(&pdf, 2));

    // The first selected source page need not be page zero. Its physical box
    // also distinguishes the source index from an output ordinal.
    let offset_records = records.map(|mut record| {
        record[1] += 1;
        record
    });
    let offset_sizes = [[100.0, 200.0], sizes[0], sizes[1], sizes[2]];
    let offset_document =
        document_with_records(true, None, 0, 1, false, &offset_records, &offset_sizes);
    let offset_output = export(&offset_document, &[0], 96.0).unwrap();
    let offset_pdf = PdfDocument::load_mem(&offset_output.bytes).unwrap();
    assert_eq!(offset_pdf.get_pages().len(), 11);
    for (index, [width, height]) in sizes.iter().enumerate() {
        assert_eq!(
            page_box(&offset_pdf, index as u32 + 1),
            [0.0, 0.0, f64::from(*width), f64::from(*height)]
        );
    }
    assert!(page_invokes_crossing_stroke_form(&offset_pdf, 1));
    assert!(page_invokes_crossing_stroke_form(&offset_pdf, 2));

    for indices in [[0, 0, 2], [2, 1, 0], [2, 0, 2]] {
        let reordered = std::array::from_fn::<_, 3, _>(|index| {
            let mut record = records[index];
            record[1] = indices[index];
            record
        });
        let document = document_with_records(true, None, 0, 1, true, &reordered, &sizes);
        let output = export(&document, &[0], 96.0).unwrap();
        let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 11);
        for (index, source_index) in indices.into_iter().enumerate() {
            let [width, height] = sizes[source_index as usize];
            assert_eq!(
                page_box(&pdf, index as u32 + 1),
                [0.0, 0.0, f64::from(width), f64::from(height)]
            );
        }
        assert!(page_invokes_crossing_stroke_form(&pdf, 1));
        assert!(page_invokes_crossing_stroke_form(&pdf, 2));
        let page_ids = pdf.get_pages();
        assert_ne!(page_ids[&1], page_ids[&2]);
        assert_ne!(page_ids[&1], page_ids[&3]);
    }
    let mut out_of_range = records;
    out_of_range[1][1] = 7;
    let invalid = document_with_records(true, None, 0, 1, true, &out_of_range, &sizes);
    assert!(matches!(
        export(&invalid, &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
    let mut gap = records;
    gap[1][3] += 1;
    let invalid = document_with_records(true, None, 0, 1, true, &gap, &sizes);
    assert!(matches!(
        export(&invalid, &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
}

#[test]
fn public_pdf_paper_accepts_indirect_page_box_coordinates() {
    let mut document = document(true, None, 90, 1);
    let ArchiveResourceContent::Opaque { data } =
        &mut document.metadata.archive_resources[0].content
    else {
        panic!("test source PDF must be retained bytes");
    };
    let mut source = PdfDocument::load_mem(data).unwrap();
    let page_id = source.get_pages()[&1];
    let coordinates = [20.into(), 35.0_f64.into(), 570.into(), 535.0_f64.into()]
        .map(|value: Object| Object::Reference(source.add_object(value)));
    let crop_id = source.add_object(Object::Array(coordinates.to_vec()));
    source
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("CropBox", Object::Reference(crop_id));
    let mut bytes = Vec::new();
    source.save_to(&mut bytes).unwrap();
    *data = bytes;

    let output = export(&document, &[0], 96.0).unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 11);
    assert!(page_invokes_crossing_stroke_form(&pdf, 1));
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let crop = pdf
        .dereference(page.get(b"CropBox").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    let values = crop
        .iter()
        .map(|value| {
            let value = pdf.dereference(value).unwrap().1;
            value
                .as_float()
                .unwrap_or_else(|_| value.as_i64().unwrap() as f32)
        })
        .collect::<Vec<_>>();
    assert_eq!(values, [20.0, 35.0, 570.0, 535.0]);
}

#[test]
fn public_pdf_paper_imports_multiple_page_tree_only_sources_in_record_order() {
    let records = [
        [PDF_BINDING as i32, 0, 0, 0, NOTE_WIDTH as i32, 1485],
        [43, 0, 0, 1485, NOTE_WIDTH as i32, 3012],
        [PDF_BINDING as i32, 0, 0, 3012, NOTE_WIDTH as i32, 4539],
    ];
    let mut document = document_with_records(
        true,
        None,
        0,
        1,
        true,
        &records,
        &[[SOURCE_WIDTH, SOURCE_HEIGHT]],
    );
    document.metadata.media_manifest = Some(media_manifest_entries(&[
        (PDF_BINDING, "paper.pdf"),
        (43, "other.pdf"),
    ]));
    document.metadata.archive_resources.push(ArchiveResource {
        name: "media/other.pdf".into(),
        archive_id: None,
        content: ArchiveResourceContent::Opaque {
            data: source_pdf_pages(0, 1, &[[400.0, 300.0]]),
        },
    });
    let output = export(&document, &[0], 96.0).unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 11);
    assert_eq!(
        page_box(&pdf, 1),
        [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)]
    );
    assert_eq!(page_box(&pdf, 2), [0.0, 0.0, 400.0, 300.0]);
    assert_eq!(page_box(&pdf, 3), page_box(&pdf, 1));
    assert!(page_invokes_crossing_stroke_form(&pdf, 1));
    assert!(page_invokes_crossing_stroke_form(&pdf, 2));
    assert_ne!(pdf.get_pages()[&1], pdf.get_pages()[&3]);

    let ArchiveResourceContent::Opaque { data } =
        &mut document.metadata.archive_resources[1].content
    else {
        unreachable!()
    };
    let mut source = PdfDocument::load_mem(data).unwrap();
    let root = source.trailer.get(b"Root").unwrap().as_reference().unwrap();
    source
        .get_dictionary_mut(root)
        .unwrap()
        .set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    let mut bytes = Vec::new();
    source.save_to(&mut bytes).unwrap();
    *data = bytes;
    assert!(matches!(
        export(&document, &[0], 96.0),
        Err(PdfError::UnsupportedPdfPaper { .. })
    ));
}

#[test]
fn public_pdf_paper_uses_native_crop_and_quarter_turn_placement() {
    // Native AddPages/body-writer controls establish these source-to-display
    // inverse matrices. Object rotations remain a separate unsupported case.
    let crop = [20.0_f64, 35.0, 570.0, 535.0];
    for rotation in [0, 90, 180, 270] {
        let mut document = document(true, None, rotation, 1);
        let ArchiveResourceContent::Opaque { data } =
            &mut document.metadata.archive_resources[0].content
        else {
            panic!("test source PDF must be retained bytes");
        };
        let mut source = PdfDocument::load_mem(data).unwrap();
        let page_id = source.get_pages()[&1];
        source
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("CropBox", crop.map(Object::from).to_vec());
        let mut source_bytes = Vec::new();
        source.save_to(&mut source_bytes).unwrap();
        *data = source_bytes;
        for dpi in [72.0_f32, 144.0] {
            let output = export(&document, &[0], dpi).unwrap();
            let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
            assert_eq!(pdf.get_pages().len(), 11);
            let page_id = pdf.get_pages()[&1];
            let page = pdf.get_dictionary(page_id).unwrap();
            assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), rotation);
            assert_eq!(
                page_box(&pdf, 1),
                [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)]
            );
            let saved_crop = page.get(b"CropBox").unwrap().as_array().unwrap();
            let values = saved_crop
                .iter()
                .map(|v| f64::from(v.as_float().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(values, crop);
            assert_eq!(page_box(&pdf, 2), [0.0, 0.0, 600.0, 505.0]);
            assert!(page_invokes_crossing_stroke_form(&pdf, 1));

            let (width, height) = if rotation % 180 == 0 {
                (550.0, 500.0)
            } else {
                (500.0, 550.0)
            };
            let scale = width / f64::from(NOTE_WIDTH);
            let scene_height = (height / scale).ceil();
            let k = scale / (72.0 / f64::from(dpi));
            let dy = height - scale * scene_height;
            let expected = match rotation {
                0 => [k, 0.0, 0.0, k, crop[0], crop[1] + dy],
                90 => [0.0, k, -k, 0.0, crop[2] - dy, crop[1]],
                180 => [-k, 0.0, 0.0, -k, crop[2], crop[3] - dy],
                270 => [0.0, -k, k, 0.0, crop[0] + dy, crop[3]],
                _ => unreachable!(),
            };
            let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
            let matrix = content
                .operations
                .iter()
                .rev()
                .find(|op| op.operator == "cm")
                .unwrap();
            assert_eq!(matrix.operands.len(), 6);
            for (actual, expected) in matrix.operands.iter().zip(expected) {
                let actual = actual.as_float().map(f64::from).unwrap();
                assert!(
                    (actual - expected).abs() < 0.0001,
                    "rotation={rotation} dpi={dpi}: actual={actual} expected={expected}"
                );
            }
        }
    }
}

#[test]
#[ignore = "opt-in real-corpus structural transport check; set both SDOCX_REAL_*_SDOCX paths"]
fn production_real_pdf_paper_transport_preserves_source_graph() {
    struct CorpusCase {
        label: &'static str,
        env_var: &'static str,
        archive_sha256: &'static str,
        resource_name: &'static str,
        source_pdf_sha256: &'static str,
        source_page_count: usize,
        output_page_count: usize,
        grouped_source_page_count: usize,
        fixed_axis_size: Option<u32>,
    }

    let cases = [
        CorpusCase {
            label: "quiz",
            env_var: "SDOCX_REAL_QUIZ_SDOCX",
            archive_sha256: "38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a",
            resource_name: "media/0@pdf_1692322553368.pdf",
            source_pdf_sha256: "f01243e2d712d47ad08735ec4e47da2a94974818f2cd7c28da2db3ee326c4279",
            source_page_count: 1,
            output_page_count: 7,
            grouped_source_page_count: 0,
            fixed_axis_size: Some(1080),
        },
        CorpusCase {
            label: "cs61bl_su22",
            env_var: "SDOCX_REAL_CS61BL_SDOCX",
            archive_sha256: "fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0",
            resource_name: "media/0@pdf_1691716745867.pdf",
            source_pdf_sha256: "1cd5ae14ddbcccc29ab7e0e8842aa2777d3c1d5c35463fcc7e6d0e7469afb689",
            source_page_count: 20,
            output_page_count: 20,
            grouped_source_page_count: 10,
            fixed_axis_size: None,
        },
    ];

    for case in cases {
        let path = env::var_os(case.env_var).unwrap_or_else(|| {
            panic!(
                "explicit real-corpus run requires {} to name the retained {}.sdocx archive",
                case.env_var, case.label
            )
        });
        let archive_bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!("{}: cannot read archive {:?}: {error}", case.label, path)
        });
        assert_eq!(
            sha256_hex(&archive_bytes),
            case.archive_sha256,
            "{} archive identity changed: {:?}",
            case.label,
            path
        );
        let document = sdocx::parse_bytes(&archive_bytes)
            .unwrap_or_else(|error| panic!("{} archive parse failed: {error}", case.label));
        assert_eq!(document.pages.len(), 1, "{} stored page count", case.label);
        let records = document.pages[0]
            .background
            .pdf_paper
            .as_deref()
            .unwrap_or_else(|| panic!("{} has no retained PDF-paper records", case.label));
        assert_eq!(
            records.len(),
            case.source_page_count,
            "{} records",
            case.label
        );
        for (index, record) in records.iter().enumerate() {
            assert_eq!(
                record.media_id, 0,
                "{} record {index} media binding",
                case.label
            );
            assert_eq!(
                record.page_index, index as i32,
                "{} record {index} source-page mapping",
                case.label
            );
        }
        if case.label == "quiz" {
            assert_eq!(
                records[0].rectangle,
                PdfPaperRectangle::Integer([0, 0, 1812, 1485]),
                "quiz source placement record"
            );
        } else {
            let mut expected_top = 0_i32;
            for (index, record) in records.iter().enumerate() {
                let PdfPaperRectangle::Integer([left, top, right, bottom]) = record.rectangle
                else {
                    panic!("cs61bl_su22 record {index} is not an integer rectangle");
                };
                assert_eq!(left, 0, "cs61bl_su22 record {index} left");
                assert_eq!(top, expected_top, "cs61bl_su22 record {index} top/order");
                assert_eq!(right, 1080, "cs61bl_su22 record {index} width");
                assert!(
                    matches!(bottom - top, 1397 | 1398),
                    "cs61bl_su22 record {index} height: {}",
                    bottom - top
                );
                expected_top = bottom;
            }
            assert_eq!(expected_top, 27_952, "cs61bl_su22 final placement edge");
        }

        let resolved = document
            .metadata
            .resolve_archive_resource(0)
            .unwrap_or_else(|error| {
                panic!("{} PDF binding resolution failed: {error}", case.label)
            });
        assert!(
            !resolved.inferred,
            "{} PDF binding must be authoritative",
            case.label
        );
        assert_eq!(
            resolved.resource.name, case.resource_name,
            "{} PDF resource name",
            case.label
        );
        assert_eq!(
            sha256_hex(resolved.data),
            case.source_pdf_sha256,
            "{} embedded source PDF identity",
            case.label
        );
        let source = PdfDocument::load_mem(resolved.data)
            .unwrap_or_else(|error| panic!("{} source PDF parse failed: {error}", case.label));
        let source_pages = source.get_pages();
        assert_eq!(
            source_pages.len(),
            case.source_page_count,
            "{} PDF page count",
            case.label
        );

        // This is the public production route: it performs PDF-paper planning,
        // scene export and source-page assembly. Checks below are structural;
        // they make no pixel or Samsung-placement parity claim.
        let layout = sdocx::layout_document(&document);
        if case.label == "quiz" {
            let default_profile = render_layout_pages_pdf_detailed_with_fonts(
                &document,
                &layout,
                &[0],
                &Default::default(),
                &PdfOptions::default(),
                &FontBook::default(),
            );
            assert!(
                matches!(
                    &default_profile,
                    Err(PdfError::UnsupportedPdfPaper {
                        output_page_index: 0,
                        source_page_index: 0,
                        record_count: 1,
                    })
                ),
                "quiz stored 1812-wide default profile must retain its exact bounded admission error; got {default_profile:?}"
            );
        }
        let mut options = PdfOptions::default();
        options.dpi = 96.0;
        options.pdf_paper_fixed_axis_size = case.fixed_axis_size;
        let output = render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[0],
            &Default::default(),
            &options,
            &FontBook::default(),
        )
        .unwrap_or_else(|error| panic!("{} production PDF export failed: {error}", case.label));
        let output_pdf = PdfDocument::load_mem(&output.bytes).unwrap_or_else(|error| {
            panic!("{} production output parse failed: {error}", case.label)
        });
        let output_pages = output_pdf.get_pages();
        assert_eq!(
            output_pages.len(),
            case.output_page_count,
            "{} output page count for {} source pages",
            case.label,
            case.source_page_count
        );
        assert_eq!(layout.pages.len(), 1, "{} visible layout pages", case.label);
        let output_page_structures = output_pages
            .iter()
            .map(|(number, id)| {
                (
                    *number,
                    page_content_streams(&output_pdf, *id).unwrap_or_else(|error| {
                        panic!("{} output page {number} Contents: {error}", case.label)
                    }),
                    pdf_resource_signatures(&output_pdf, *id).unwrap_or_else(|error| {
                        panic!("{} output page {number} resources: {error}", case.label)
                    }),
                )
            })
            .collect::<Vec<_>>();

        let mut previous_output_page = 0_u32;
        let mut grouped_source_page_count = 0_usize;
        for source_index in 0..case.source_page_count {
            let source_number = u32::try_from(source_index + 1).unwrap();
            let source_id = source_pages[&source_number];
            let source_streams = page_content_streams(&source, source_id).unwrap_or_else(|error| {
                panic!(
                    "{} source page {source_index} Contents: {error}",
                    case.label
                )
            });
            assert!(
                !source_streams.is_empty(),
                "{} source page {source_index} has no content streams",
                case.label
            );
            let source_resources =
                pdf_resource_signatures(&source, source_id).unwrap_or_else(|error| {
                    panic!(
                        "{} source page {source_index} resources: {error}",
                        case.label
                    )
                });

            let mut matches = Vec::new();
            let mut candidate_diagnostics = Vec::new();
            for (output_number, output_streams, output_resources) in &output_page_structures {
                let decoded_content_is_preserved =
                    output_streams.windows(source_streams.len()).any(|window| {
                        window
                            .iter()
                            .zip(&source_streams)
                            .all(|(actual, expected)| actual.decoded == expected.decoded)
                    });
                if !decoded_content_is_preserved {
                    continue;
                }
                let encoded_content_is_preserved = output_streams
                    .windows(source_streams.len())
                    .any(|window| window == source_streams);
                let resource_difference =
                    source_resources
                        .iter()
                        .find_map(|(category, source_entries)| {
                            match output_resources.get(category) {
                                None => Some(format!("missing category {category:?}")),
                                Some(output_entries) => {
                                    source_entries.iter().find_map(|(name, expected)| {
                                        (output_entries.get(name) != Some(expected)).then(|| {
                                            format!("category {category:?}, resource {name:?}")
                                        })
                                    })
                                }
                            }
                        });
                candidate_diagnostics.push(format!(
                    "output_page={output_number}, exact_encoded_streams={encoded_content_is_preserved}, first_resource_difference={:?}",
                    resource_difference
                ));
                if encoded_content_is_preserved && resource_difference.is_none() {
                    matches.push((*output_number, *output_pages.get(output_number).unwrap()));
                }
            }
            assert_eq!(
                matches.len(),
                1,
                "{} source page {source_index} should map to exactly one output page with its exact ordered content streams and source resource graph; matches={:?}, decoded-content candidates={candidate_diagnostics:?}",
                case.label,
                matches.iter().map(|(number, _)| number).collect::<Vec<_>>()
            );
            let (output_number, _) = matches[0];
            assert!(
                output_number > previous_output_page,
                "{} source page {source_index} mapped out of order: output page {output_number} follows {previous_output_page}",
                case.label
            );
            previous_output_page = output_number;
            assert_eq!(
                page_box(&output_pdf, output_number),
                page_box(&source, source_number),
                "{} source page {source_index} MediaBox changed",
                case.label
            );
            let source_group =
                pdf_page_group_signature(&source, source_id).unwrap_or_else(|error| {
                    panic!("{} source page {source_index} Group: {error}", case.label)
                });
            let output_id = output_pages[&output_number];
            let output_group =
                pdf_page_group_signature(&output_pdf, output_id).unwrap_or_else(|error| {
                    panic!("{} output page {output_number} Group: {error}", case.label)
                });
            assert_eq!(
                output_group, source_group,
                "{} source page {source_index} page-level Group changed",
                case.label
            );
            grouped_source_page_count += usize::from(source_group.is_some());
        }
        assert_eq!(
            grouped_source_page_count, case.grouped_source_page_count,
            "{} source grouped-page count in pinned corpus",
            case.label
        );
    }
}

#[cfg(feature = "serde")]
fn add_captured_ink_and_airbrush(document: &mut Document) {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/fountain-v16-scale-native.json")).unwrap();
    let mut fountain: sdocx::Stroke = serde_json::from_value(evidence["stroke"].clone()).unwrap();
    fountain.bbox = bbox(115.0, 205.0, 134.0, 230.0);
    fountain.rendering.as_mut().unwrap().metadata = document.pages[0]
        .strokes()
        .next()
        .unwrap()
        .rendering
        .as_ref()
        .unwrap()
        .metadata
        .clone();
    fountain
        .rendering
        .as_mut()
        .unwrap()
        .metadata
        .as_mut()
        .unwrap()
        .bbox = fountain.bbox;
    document.pages[0].objects.push(fountain.into());
    let mut airbrush = document.pages[0].strokes().next().unwrap().clone();
    airbrush.points = (0..5)
        .map(|i| sdocx::Point {
            x: 300.0 + f64::from(i) * 3.0,
            y: 300.0 + f64::from(i * i) * 0.5,
        })
        .collect();
    airbrush.pressures = vec![0.3; 5];
    airbrush.tilts = vec![0.12; 5];
    airbrush.orientations = vec![0.2; 5];
    airbrush.timestamps = vec![300, 315, 330, 345, 360];
    airbrush.pen_width = 3.144;
    airbrush.bbox = bbox(300.0, 300.0, 312.0, 308.0);
    let rendering = airbrush.rendering.as_mut().unwrap();
    rendering.metadata.as_mut().unwrap().bbox = airbrush.bbox;
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.AirBrushPen".into());
    rendering.advanced_settings = Some(String::new());
    rendering.tool_type_raw = 1;
    rendering.properties.compressed = true;
    rendering.properties.fixed_width = false;
    rendering.style.pen_size = Some(3.144);
    rendering.style.color_argb = Some(0xff44_aaee);
    rendering.style.stroke_type_raw = Some(0);
    rendering.style.color_type_raw = Some(0);
    document.pages[0].objects.push(airbrush.into());
    let mut ink = document.pages[0].strokes().next().unwrap().clone();
    ink.points = vec![
        sdocx::Point { x: 200.0, y: 250.0 },
        sdocx::Point { x: 208.0, y: 250.0 },
    ];
    ink.pressures = vec![0.25, 0.5];
    ink.timestamps = vec![100, 110];
    ink.tilts = vec![0.0; 2];
    ink.orientations = vec![0.0; 2];
    ink.bbox = bbox(200.0, 250.0, 208.0, 250.0);
    ink.pen_width = 4.0;
    let rendering = ink.rendering.as_mut().unwrap();
    rendering.metadata.as_mut().unwrap().bbox = ink.bbox;
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.InkPen2".into());
    rendering.advanced_settings = Some("9;".into());
    rendering.tool_type_raw = 2;
    rendering.properties.compressed = true;
    rendering.properties.stylus_channels = true;
    rendering.properties.binary_added = true;
    rendering.properties.millisecond_timestamps = false;
    rendering.properties.fixed_width = false;
    rendering.style = Default::default();
    rendering.style.color_argb = Some(0xff336699);
    rendering.style.pen_size = Some(4.0);
    rendering.style.initial_tolerance = Some(0.0);
    rendering.style.line_type_raw = Some(0);
    rendering.style.stroke_type_raw = Some(0);
    document.pages[0].objects.push(ink.into());
}

#[cfg(feature = "serde")]
#[test]
fn airbrush_pdf_paper_keeps_source_and_native_ink() {
    let mut document = document(true, None, 0, 1);
    add_captured_ink_and_airbrush(&mut document);
    let layout = sdocx::layout_document(&document);
    let mut options = sdocx::RenderOptions::default();
    options.airbrush_profile = Some(sdocx::AirBrushRenderProfile::SavedV2);
    options.inkpen2_profile = Some(sdocx::InkPen2RenderProfile {
        screen_width: 1920,
        screen_height: 1080,
        enhanced_antialias: false,
    });
    let mut pdf_options = PdfOptions::default();
    pdf_options.pdf_paper_fixed_axis_size = Some(1080);
    let output = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[0],
        &options,
        &pdf_options,
        &FontBook::default(),
    )
    .unwrap();
    let pdf = PdfDocument::load_mem(&output.bytes).unwrap();
    assert_eq!(
        page_box(&pdf, 1),
        [0.0, 0.0, f64::from(SOURCE_WIDTH), f64::from(SOURCE_HEIGHT)]
    );
    let source = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    assert!(source.operations.iter().any(|op| {
        op.operator == "Do"
            && op
                .operands
                .last()
                .is_some_and(|o| o.as_name().ok() == Some(b"Im0"))
    }));
    assert!(source.operations.iter().any(|op| op.operator == "S"));
    assert!(
        pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|value| value.as_name().ok())
                    == Some(b"Image")
                    && stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone())
                        == [0x31, 0x62, 0x93]
            }))
    );
    let functions: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"FunctionType").ok() == Some(&Object::Integer(2)) && d.len() == 6)
        .map(|d| format!("{d:?}"))
        .collect();
    let mut fallback_options = options.clone();
    fallback_options.airbrush_profile = None;
    let fallback = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[0],
        &fallback_options,
        &pdf_options,
        &FontBook::default(),
    )
    .unwrap();
    let fallback_pdf = PdfDocument::load_mem(&fallback.bytes).unwrap();
    let fallback_count = fallback_pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"FunctionType").ok() == Some(&Object::Integer(2)) && d.len() == 6)
        .count();
    assert!(
        fallback_count > functions.len(),
        "AirBrush activates function reuse"
    );
    assert!(!functions.is_empty());
    assert_eq!(
        functions.len(),
        functions.iter().collect::<HashSet<_>>().len()
    );
}

#[cfg(feature = "serde")]
#[test]
fn retained_text_airbrush_and_native_ink_export_together() {
    let mut document = document(false, None, 0, 1);
    document.metadata.page_mode = Some(0);
    document.pages[0].width = 600;
    document.pages[0].height = 600;
    add_captured_ink_and_airbrush(&mut document);
    let ink = document.pages[0]
        .strokes_mut()
        .last()
        .unwrap()
        .rendering
        .as_mut()
        .unwrap();
    ink.properties.rainbow_effect = true;
    ink.style.color_type_raw = Some(1);
    ink.style.gradient_colors_argb = Some(vec![0xff336699, 0xffcc8844]);
    ink.style.rainbow_distance = Some(100);
    ink.style.rainbow_offset = Some(0.0);
    let mut text = rich_text("Paper text", Vec::new());
    text.bbox = bbox(50.0, 50.0, 450.0, 100.0);
    document.pages[0]
        .objects
        .push(PageElement::TextBox(text).into());
    let mut options = sdocx::RenderOptions::default();
    options.airbrush_profile = Some(sdocx::AirBrushRenderProfile::SavedV2);
    options.inkpen2_profile = Some(sdocx::InkPen2RenderProfile {
        screen_width: 1920,
        screen_height: 1080,
        enhanced_antialias: false,
    });
    let bytes = sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap();
    let pdf = PdfDocument::load_mem(&bytes).unwrap();
    assert_eq!(
        pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
        "Papertext"
    );
    let type4 = pdf
        .objects
        .values()
        .filter(|o| {
            o.as_stream()
                .is_ok_and(|s| s.dict.get(b"FunctionType").ok() == Some(&Object::Integer(4)))
        })
        .count();
    assert!(
        type4 > 0,
        "per-stamp native ink must lower to calculator functions"
    );
    let functions: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"FunctionType").ok() == Some(&Object::Integer(2)) && d.len() == 6)
        .map(|d| format!("{d:?}"))
        .collect();
    assert!(!functions.is_empty());
    assert_eq!(
        functions.len(),
        functions.iter().collect::<HashSet<_>>().len()
    );
}
