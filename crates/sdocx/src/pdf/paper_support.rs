//! Private source-preserving PDF paper grafting and SingleX planning support.

use std::collections::{HashMap, HashSet};

use super::{paper_segments, paper_selection};
use crate::render::ContinuousBodyQuerySnapshot;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, content::Content, dictionary};

use crate::{ArchiveResourceError, DocumentMetadata, PdfPaperRecord, PdfPaperRectangle};

fn obj_dict<'a>(doc: &'a Document, object: &'a Object) -> Result<&'a Dictionary, String> {
    match object {
        Object::Reference(id) => doc
            .get_object(*id)
            .map_err(|e| e.to_string())?
            .as_dict()
            .map_err(|e| e.to_string()),
        Object::Dictionary(d) => Ok(d),
        _ => Err("expected dictionary or indirect reference".into()),
    }
}

fn inherited<'a>(
    doc: &'a Document,
    mut page: &'a Dictionary,
    key: &[u8],
) -> Result<&'a Object, String> {
    let mut seen = HashSet::<ObjectId>::new();
    loop {
        if let Ok(value) = page.get(key) {
            return Ok(value);
        }
        let parent = page
            .get(b"Parent")
            .map_err(|_| format!("missing inherited {:?}", String::from_utf8_lossy(key)))?;
        let Object::Reference(parent_id) = parent else {
            return Err("page Parent must be an indirect reference".into());
        };
        if !seen.insert(*parent_id) {
            return Err("cycle in page Parent chain".into());
        }
        page = obj_dict(doc, parent)?;
    }
}

fn deref<'a>(doc: &'a Document, mut object: &'a Object) -> Result<&'a Object, String> {
    let mut seen = HashSet::<ObjectId>::new();
    for _ in 0..64 {
        match object {
            Object::Reference(id) => {
                if !seen.insert(*id) {
                    return Err("cycle in indirect object references".into());
                }
                object = doc.get_object(*id).map_err(|e| e.to_string())?;
            }
            _ => return Ok(object),
        }
    }
    Err("indirect object reference depth exceeds 64".into())
}

fn inherited_optional<'a>(
    doc: &'a Document,
    page: &'a Dictionary,
    key: &[u8],
) -> Result<Option<&'a Object>, String> {
    match inherited(doc, page, key) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.starts_with("missing inherited") => Ok(None),
        Err(error) => Err(error),
    }
}

fn rect_value(doc: &Document, value: &Object) -> Result<[f64; 4], String> {
    let arr = deref(doc, value)?.as_array().map_err(|e| e.to_string())?;
    if arr.len() != 4 {
        return Err("page box must have four coordinates".into());
    }
    let mut out = [0.0; 4];
    for (dst, value) in out.iter_mut().zip(arr) {
        let value = deref(doc, value)?;
        *dst = value
            .as_float()
            .or_else(|_| value.as_i64().map(|v| v as f32))
            .map_err(|e| e.to_string())? as f64;
    }
    if !out.iter().all(|value| value.is_finite()) {
        return Err("page box contains non-finite coordinates".into());
    }
    if out[0] > out[2] {
        out.swap(0, 2);
    }
    if out[1] > out[3] {
        out.swap(1, 3);
    }
    Ok(out)
}

fn rect(doc: &Document, page: &Dictionary, key: &[u8]) -> Result<[f64; 4], String> {
    rect_value(doc, inherited(doc, page, key)?)
}

fn page_rotation(doc: &Document, page: &Dictionary) -> Result<i64, String> {
    match inherited_optional(doc, page, b"Rotate")? {
        Some(value) => Ok(deref(doc, value)?
            .as_i64()
            .map_err(|e| e.to_string())?
            .rem_euclid(360)),
        None => Ok(0),
    }
}

fn page_user_unit(doc: &Document, page: &Dictionary) -> Result<f64, String> {
    match page.get(b"UserUnit") {
        Ok(value) => {
            let value = deref(doc, value)?;
            value
                .as_float()
                .or_else(|_| value.as_i64().map(|n| n as f32))
                .map(|n| n as f64)
                .map_err(|e| e.to_string())
        }
        Err(_) => Ok(1.0),
    }
}

fn effective_crop(doc: &Document, page: &Dictionary) -> Result<[f64; 4], String> {
    let media = rect(doc, page, b"MediaBox")?;
    let crop = match inherited_optional(doc, page, b"CropBox")? {
        Some(value) => rect_value(doc, value)?,
        None => media,
    };
    let r = [
        media[0].max(crop[0]),
        media[1].max(crop[1]),
        media[2].min(crop[2]),
        media[3].min(crop[3]),
    ];
    if r[2] <= r[0] || r[3] <= r[1] {
        return Err("empty effective page box".into());
    }
    Ok(r)
}

/// PDF affine matrix [a b c d e f], mapping page user coordinates to the
/// origin-normalized displayed page. `/UserUnit` is incorporated for Form use.
fn page_to_display(doc: &Document, page: &Dictionary) -> Result<([f64; 6], [f64; 4]), String> {
    let r = effective_crop(doc, page)?;
    let unit = page_user_unit(doc, page)?;
    if !unit.is_finite() || unit <= 0.0 {
        return Err("invalid UserUnit".into());
    }
    let m = match page_rotation(doc, page)? {
        0 => [unit, 0.0, 0.0, unit, -r[0] * unit, -r[1] * unit],
        90 => [0.0, -unit, unit, 0.0, -r[1] * unit, r[2] * unit],
        180 => [-unit, 0.0, 0.0, -unit, r[2] * unit, r[3] * unit],
        270 => [0.0, unit, -unit, 0.0, r[3] * unit, -r[0] * unit],
        _ => return Err("non-quarter-turn page rotation is unsupported".into()),
    };
    Ok((m, r))
}

fn contents_bytes(doc: &Document, page: &Dictionary) -> Result<Vec<u8>, String> {
    fn append(
        doc: &Document,
        object: &Object,
        out: &mut Vec<u8>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("page Contents nesting exceeds 64".into());
        }
        match object {
            Object::Reference(id) => append(
                doc,
                doc.get_object(*id).map_err(|e| e.to_string())?,
                out,
                depth + 1,
            ),
            Object::Array(items) => {
                for item in items {
                    append(doc, item, out, depth + 1)?;
                    out.push(b'\n');
                }
                Ok(())
            }
            Object::Stream(stream) => {
                out.extend(stream.decompressed_content().map_err(|e| e.to_string())?);
                Ok(())
            }
            other => Err(format!("unsupported page Contents object: {other:?}")),
        }
    }
    let mut out = Vec::new();
    if let Ok(contents) = page.get(b"Contents")
        && !matches!(deref(doc, contents)?, Object::Null)
    {
        append(doc, contents, &mut out, 0)?;
    }
    Ok(out)
}

/// Guard the page caller from residual transforms/clips. Close unmatched q
/// operators at page end; reject unmatched Q because its semantics are invalid.
fn source_graphics_state_depth(bytes: &[u8]) -> Result<usize, String> {
    let decoded =
        Content::decode(bytes).map_err(|e| format!("cannot parse source page content: {e}"))?;
    let mut depth = 0usize;
    for op in &decoded.operations {
        match op.operator.as_str() {
            "q" => {
                depth = depth
                    .checked_add(1)
                    .ok_or("graphics-state depth overflow")?
            }
            "Q" if depth == 0 => return Err("source page content has unmatched Q".into()),
            "Q" => depth -= 1,
            _ => {}
        }
    }
    Ok(depth)
}

fn content_items(doc: &Document, object: &Object) -> Result<Vec<Object>, String> {
    fn flatten(doc: &Document, object: &Object, depth: usize) -> Result<Vec<Object>, String> {
        if depth > 64 {
            return Err("page Contents nesting exceeds 64".into());
        }
        match object {
            Object::Reference(id) => match doc.get_object(*id).map_err(|e| e.to_string())? {
                Object::Stream(_) => Ok(vec![Object::Reference(*id)]),
                Object::Array(items) => items.iter().try_fold(Vec::new(), |mut out, item| {
                    out.extend(flatten(doc, item, depth + 1)?);
                    Ok(out)
                }),
                _ => Err("Contents reference is not a stream or array".into()),
            },
            Object::Array(items) => items.iter().try_fold(Vec::new(), |mut out, item| {
                out.extend(flatten(doc, item, depth + 1)?);
                Ok(out)
            }),
            Object::Stream(_) => Err("page Contents stream must be indirect".into()),
            _ => Err("invalid page Contents object".into()),
        }
    }
    if matches!(deref(doc, object)?, Object::Null) {
        Ok(Vec::new())
    } else {
        flatten(doc, object, 0)
    }
}

/// Graft one overlay PDF page onto a page in the original source PDF.
/// `display_to_source` maps origin-normalized overlay display coordinates
/// (including the overlay page's CropBox/Rotate/UserUnit) into source page user
/// coordinates. `clip` is the corresponding clip rectangle in source user
/// coordinates. Keeping this explicit avoids asserting Samsung's unresolved
/// object-placement rule.
#[cfg(test)]
fn graft_overlay_page(
    source_bytes: &[u8],
    source_page_number: u32,
    overlay_bytes: &[u8],
    overlay_page_number: u32,
    display_to_source: [f64; 6],
    clip: [f64; 4],
) -> Result<Vec<u8>, String> {
    if !display_to_source.iter().all(|value| value.is_finite()) {
        return Err("overlay placement matrix contains non-finite values".into());
    }
    if !clip.iter().all(|value| value.is_finite()) {
        return Err("overlay clip contains non-finite values".into());
    }
    let clip_width = clip[2] - clip[0];
    let clip_height = clip[3] - clip[1];
    if !clip_width.is_finite()
        || !clip_height.is_finite()
        || clip_width <= 0.0
        || clip_height <= 0.0
    {
        return Err("overlay clip must have positive finite width and height".into());
    }
    let mut source = Document::load_mem(source_bytes).map_err(|e| e.to_string())?;
    let source_pages = source.get_pages();
    let source_page_id = *source_pages
        .get(&source_page_number)
        .ok_or("source page number missing")?;
    let source_page = source
        .get_object(source_page_id)
        .map_err(|e| e.to_string())?
        .as_dict()
        .map_err(|e| e.to_string())?
        .clone();
    let source_content_bytes = contents_bytes(&source, &source_page)?;
    let source_q_depth = source_graphics_state_depth(&source_content_bytes)?;
    let original_content_items = content_items(
        &source,
        source_page.get(b"Contents").map_err(|e| e.to_string())?,
    )?;
    let _ = page_to_display(&source, &source_page)?;

    let mut overlay = Document::load_mem(overlay_bytes).map_err(|e| e.to_string())?;
    let overlay_page_id = *overlay
        .get_pages()
        .get(&overlay_page_number)
        .ok_or("overlay page number missing")?;
    let overlay_page = overlay
        .get_object(overlay_page_id)
        .map_err(|e| e.to_string())?
        .as_dict()
        .map_err(|e| e.to_string())?
        .clone();
    let (overlay_matrix, overlay_crop) = page_to_display(&overlay, &overlay_page)?;
    let overlay_content = contents_bytes(&overlay, &overlay_page)?;
    let has_group = overlay_page.get(b"Group").is_ok();

    let start = source.max_id + 1;
    overlay.renumber_objects_with(start);
    let overlay_max_id = overlay.max_id;
    let remapped_page_id = *overlay
        .get_pages()
        .get(&overlay_page_number)
        .ok_or("renumbered overlay page missing")?;
    let remapped_page = overlay
        .get_object(remapped_page_id)
        .map_err(|e| e.to_string())?
        .as_dict()
        .map_err(|e| e.to_string())?;
    let remapped_resources = inherited(&overlay, remapped_page, b"Resources")?.clone();
    let remapped_group = if has_group {
        Some(inherited(&overlay, remapped_page, b"Group")?.clone())
    } else {
        None
    };
    for (id, object) in overlay.objects {
        source.objects.insert(id, object);
    }
    source.max_id = source.max_id.max(overlay_max_id);

    let mut form_dict = lopdf::Dictionary::new();
    form_dict.set("Type", "XObject");
    form_dict.set("Subtype", "Form");
    form_dict.set("FormType", 1);
    form_dict.set(
        "BBox",
        overlay_crop
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect::<Vec<_>>(),
    );
    form_dict.set(
        "Matrix",
        overlay_matrix
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect::<Vec<_>>(),
    );
    form_dict.set("Resources", remapped_resources);
    if let Some(group) = remapped_group {
        form_dict.set("Group", group);
    }
    let form_id = source.add_object(Stream::new(form_dict, overlay_content));

    let mut resources = match inherited_optional(&source, &source_page, b"Resources")? {
        Some(object) => obj_dict(&source, object)?.clone(),
        None => Dictionary::new(),
    };
    let mut xobjects = resources
        .get(b"XObject")
        .ok()
        .map(|o| obj_dict(&source, o).cloned())
        .transpose()?
        .unwrap_or_default();
    let mut name = b"SDOCXOverlay".to_vec();
    let mut suffix = 0;
    while xobjects.has(&name) {
        suffix += 1;
        name = format!("SDOCXOverlay{suffix}").into_bytes();
    }
    xobjects.set(name.clone(), form_id);
    resources.set("XObject", xobjects);
    source
        .get_object_mut(source_page_id)
        .map_err(|e| e.to_string())?
        .as_dict_mut()
        .map_err(|e| e.to_string())?
        .set("Resources", resources);

    let command = format!(
        "n q {} {} {} {} re W n {} {} {} {} {} {} cm /{} Do Q\n",
        clip[0],
        clip[1],
        clip[2] - clip[0],
        clip[3] - clip[1],
        display_to_source[0],
        display_to_source[1],
        display_to_source[2],
        display_to_source[3],
        display_to_source[4],
        display_to_source[5],
        String::from_utf8_lossy(&name),
    );
    let prefix_id = source.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let mut suffix = Vec::with_capacity((source_q_depth + 2) * 2);
    suffix.extend_from_slice(b"n\n");
    for _ in 0..=source_q_depth {
        suffix.extend_from_slice(b"Q\n");
    }
    let suffix_id = source.add_object(Stream::new(Dictionary::new(), suffix));
    let overlay_content_id =
        source.add_object(Stream::new(Dictionary::new(), command.into_bytes()));
    let page = source
        .get_object_mut(source_page_id)
        .map_err(|e| e.to_string())?
        .as_dict_mut()
        .map_err(|e| e.to_string())?;
    let mut contents = vec![prefix_id.into()];
    contents.extend(original_content_items);
    contents.push(suffix_id.into());
    contents.push(overlay_content_id.into());
    page.set("Contents", contents);

    let mut bytes = Vec::new();
    source.save_to(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// A source document admitted to the output assembler. `key` is the retained
/// resource identity from the note archive, not a guessed page relationship.
#[cfg(test)]
struct SourcePdf<'a> {
    key: &'a str,
    bytes: &'a [u8],
}

/// One output page. Imported pages carry explicit native-derived placement;
/// continuation pages contain the SDK-rendered note page only.
enum OutputPage<'a> {
    Imported {
        source_key: &'a str,
        source_page_index: i32,
        overlay_bytes: &'a [u8],
        overlay_page_number: u32,
        display_to_source: [f64; 6],
        clip: [f64; 4],
    },
    OverlayOnly {
        overlay_bytes: &'a [u8],
        overlay_page_number: u32,
    },
}

struct ParsedSource {
    doc: Document,
    pages: std::collections::BTreeMap<u32, ObjectId>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum PdfPaperAdmissionError {
    #[error("unsupported SingleX PDF paper at output page {output_page_index}: {reason}")]
    UnsupportedTopology {
        output_page_index: usize,
        source_page_index: usize,
        record_count: usize,
        reason: &'static str,
    },
    #[error(
        "PDF paper record {record_index} refers to external PDF paper (-1000), which has no archive source"
    )]
    ExternalPdfUnavailable { record_index: usize },
    #[error("PDF paper record {record_index} has invalid negative media ID {media_id}")]
    NegativeBinding { record_index: usize, media_id: i32 },
    #[error("PDF paper record {record_index} media ID {media_id} could not be resolved: {source}")]
    Binding {
        record_index: usize,
        media_id: u32,
        #[source]
        source: ArchiveResourceError,
    },
    #[error(
        "PDF paper record {record_index} source {resource:?} exceeds the 1 GiB admission limit"
    )]
    SourceTooLarge {
        record_index: usize,
        resource: String,
    },
    #[error("PDF paper record {record_index} source {resource:?} is not a readable PDF: {message}")]
    InvalidPdf {
        record_index: usize,
        resource: String,
        message: String,
    },
    #[error("PDF paper record {record_index} source {resource:?} has no pages")]
    EmptyPdf {
        record_index: usize,
        resource: String,
    },
    #[error("PDF paper record {record_index} has negative source page index {page_index}")]
    NegativePageIndex {
        record_index: usize,
        page_index: i32,
    },
    #[error(
        "PDF paper record {record_index} source page index {page_index} is out of range for {resource:?} ({page_count} pages)"
    )]
    PageIndexOutOfRange {
        record_index: usize,
        resource: String,
        page_index: i32,
        page_count: usize,
    },
}

struct PreparedPdfPaperRecord {
    source_key: String,
    source_page_index: i32,
    inferred: bool,
}

struct PreparedPdfPaperRecords {
    sources: std::collections::BTreeMap<String, ParsedSource>,
    records: Vec<PreparedPdfPaperRecord>,
}

#[derive(Clone)]
struct ImportedSourcePage {
    source_key: String,
    source_page_index: i32,
    source_to_display: [f64; 6],
    source_crop: [f64; 4],
    physical_surface: [f64; 2],
}

/// A page in the measured SingleX output stream. `query_height` controls native
/// object admission; `scene_height` covers the independently measured physical
/// surface so selected geometry can cross the query edge and be clipped by the
/// final page box.
struct SingleXOutputPage {
    query_y: u32,
    scene_height: u32,
    physical_width: f64,
    physical_height: f64,
    imported_source: Option<ImportedSourcePage>,
    selected_objects: Vec<usize>,
}

pub(super) struct SingleXPaperPlan {
    parsed_sources: std::collections::BTreeMap<String, ParsedSource>,
    note_width: u32,
    note_height: u32,
    output_pages: Vec<SingleXOutputPage>,
}

impl PdfPaperAdmissionError {
    pub(super) fn to_pdf_error(&self) -> super::PdfError {
        let (output_page_index, source_page_index, record_count) = match self {
            Self::UnsupportedTopology {
                output_page_index,
                source_page_index,
                record_count,
                ..
            } => (*output_page_index, *source_page_index, *record_count),
            Self::ExternalPdfUnavailable { record_index }
            | Self::NegativeBinding { record_index, .. }
            | Self::Binding { record_index, .. }
            | Self::SourceTooLarge { record_index, .. }
            | Self::InvalidPdf { record_index, .. }
            | Self::EmptyPdf { record_index, .. }
            | Self::NegativePageIndex { record_index, .. }
            | Self::PageIndexOutOfRange { record_index, .. } => (*record_index, 0, 1),
        };
        super::PdfError::UnsupportedPdfPaper {
            output_page_index,
            source_page_index,
            record_count,
        }
    }
}

/// Plan the positive, source-preserving SingleX subset. The source PDF is parsed
/// here so admission and rendering use the same bound resource and page identity.
pub(super) fn plan_singlex_pdf_paper(
    document: &crate::Document,
    layout: &crate::LayoutDocument,
    page_indices: &[usize],
    fixed_axis_profile: Option<super::paper_scale::FixedAxisProfile>,
    body_query_snapshot: Option<&ContinuousBodyQuerySnapshot>,
) -> Result<Option<SingleXPaperPlan>, PdfPaperAdmissionError> {
    if page_indices.is_empty() {
        return Ok(None);
    }
    let selected = page_indices
        .first()
        .and_then(|index| layout.pages.get(*index))
        .ok_or(PdfPaperAdmissionError::UnsupportedTopology {
            output_page_index: 0,
            source_page_index: 0,
            record_count: 0,
            reason: "SingleX PDF paper requires a selected visible page",
        })?;
    let records = selected
        .page
        .background
        .pdf_paper
        .as_deref()
        .unwrap_or_default();
    if records.is_empty() {
        return Ok(None);
    }
    let record_count = records.len();
    let reject = |output_page_index, reason| PdfPaperAdmissionError::UnsupportedTopology {
        output_page_index,
        source_page_index: selected.source_page_index,
        record_count,
        reason,
    };

    if document.metadata.page_mode != Some(1)
        || document.metadata.orientation != Some(0)
        || document.pages.len() != 1
        || layout.pages.len() != 1
        || page_indices != [0]
        || selected.source_page_index != 0
    {
        return Err(reject(
            0,
            "unsupported page mode, orientation, or page selection",
        ));
    }
    // The renderer inserts a validated inspection TextBox into layout output
    // for the source-owned continuous body. It is metadata plumbing rather
    // than a saved page object; exclude only the node whose association with
    // the authoritative body has been checked by LayoutPage.
    let verified_body = selected.single_page_continuous_body(document);
    let inspection_object_index = verified_body
        .as_ref()
        .and_then(|_| selected.body_text_slice().map(|body| body.object_index));
    let body_requires_query = document
        .metadata
        .note_text
        .as_ref()
        .is_some_and(|text| !rich_text_is_benign_blank(text));
    let body_has_unowned_source =
        selected.body_text.is_some() && document.metadata.note_text.is_none();
    let body_query_axis_matches = document
        .metadata
        .flow_dimensions
        .is_some_and(|(flow_width, _)| flow_width == selected.page.width);
    let body_has_unsupported_content = body_has_unowned_source
        || (body_requires_query && (body_query_snapshot.is_none() || !body_query_axis_matches));
    if body_has_unsupported_content {
        return Err(reject(
            0,
            "visible continuous body text or inline objects are not supported by this PDF-paper route",
        ));
    }
    let native_default_color = |color: Option<crate::Color>| {
        color.is_none_or(|color| {
            color
                == (crate::Color {
                    r: 252,
                    g: 252,
                    b: 252,
                })
        })
    };
    let first_source_page = u32::try_from(records[0].page_index).ok();
    let supported_template = match selected.page.template {
        None => true,
        Some(crate::PageTemplate {
            source: crate::PageTemplateSource::CustomPdf { page_index },
            ..
        }) => first_source_page == Some(page_index),
        Some(_) => false,
    };
    if !native_default_color(document.metadata.background_color)
        || !native_default_color(selected.page.background_color)
        || document.metadata.dark_mode_compatibility == Some(true)
        || selected.page.background.template_uri.is_some()
        || selected.page.background.image_id.is_some()
        || selected
            .page
            .background
            .template_type
            .is_some_and(|kind| kind != 0)
        || !supported_template
    {
        return Err(reject(
            0,
            "only the native default page background and PDF template are supported",
        ));
    }
    let Some((note_width, note_height)) = document.metadata.page_dimensions else {
        return Err(reject(0, "native note dimensions are missing"));
    };
    let Some((_, default_height)) = document.metadata.default_page_dimensions else {
        return Err(reject(0, "native default page dimensions are missing"));
    };
    if note_width == 0 || note_height == 0 || default_height == 0 {
        return Err(reject(0, "native note dimensions must be positive"));
    }
    let [record_validation_width, record_validation_height] = fixed_axis_profile
        .map(|profile| profile.source_dimensions)
        .unwrap_or([note_width, note_height]);
    let default_height_i32 = i32::try_from(default_height)
        .map_err(|_| reject(0, "default page height exceeds native range"))?;
    let record_validation_width_i32 = i32::try_from(record_validation_width)
        .map_err(|_| reject(0, "source note width exceeds native range"))?;
    let record_validation_height_i32 = i32::try_from(record_validation_height)
        .map_err(|_| reject(0, "source note height exceeds native range"))?;
    let mut expected_y = 0_i32;
    for (record_index, record) in records.iter().enumerate() {
        let PdfPaperRectangle::Integer([0, top, right, bottom]) = record.rectangle else {
            return Err(reject(
                record_index,
                "PDF-paper records must use modern integer rectangles",
            ));
        };
        if top != expected_y
            || right != record_validation_width_i32
            || bottom <= top
            || bottom > record_validation_height_i32
        {
            return Err(reject(
                record_index,
                "PDF-paper records must be contiguous, top-aligned, full-width, and within the note",
            ));
        }
        if record.page_index < 0 {
            return Err(reject(
                record_index,
                "PDF-paper source page indexes must be nonnegative",
            ));
        }
        expected_y = bottom;
    }

    let prepared = prepare_pdf_paper_records(&document.metadata, records)?;
    for (record_index, prepared_record) in prepared.records.iter().enumerate() {
        if prepared_record.inferred {
            return Err(reject(
                record_index,
                "PDF-paper media binding must be authoritative",
            ));
        }
        if prepared_record.source_page_index != records[record_index].page_index {
            return Err(reject(
                record_index,
                "resolved PDF-paper source page differs from its saved record",
            ));
        }
    }
    if prepared.sources.len() > 1 {
        for source in prepared.sources.values() {
            if !catalog_is_page_tree_only(&source.doc).unwrap_or(false) {
                return Err(reject(
                    0,
                    "multiple PDF-paper sources require page-tree-only catalogs",
                ));
            }
        }
    }
    let mut imported_sources = Vec::with_capacity(prepared.records.len());
    for (record_index, prepared_record) in prepared.records.iter().enumerate() {
        let source = prepared
            .sources
            .get(&prepared_record.source_key)
            .ok_or_else(|| reject(record_index, "resolved PDF-paper source is missing"))?;
        let source_number = u32::try_from(prepared_record.source_page_index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or_else(|| reject(record_index, "source page index is invalid"))?;
        let source_id = *source
            .pages
            .get(&source_number)
            .ok_or_else(|| reject(record_index, "source page index is out of range"))?;
        let source_page = source
            .doc
            .get_dictionary(source_id)
            .map_err(|_| reject(record_index, "source page dictionary is invalid"))?;
        let (source_to_display, crop) = page_to_display(&source.doc, source_page)
            .map_err(|_| reject(record_index, "source page geometry is unsupported"))?;
        let unit = page_user_unit(&source.doc, source_page)
            .map_err(|_| reject(record_index, "source page UserUnit is invalid"))?;
        let rotation = page_rotation(&source.doc, source_page)
            .map_err(|_| reject(record_index, "source page rotation is invalid"))?;
        if unit != 1.0 {
            return Err(reject(
                record_index,
                "source pages with UserUnit other than 1 are not supported",
            ));
        }
        let crop_width = crop[2] - crop[0];
        let crop_height = crop[3] - crop[1];
        let source_surface = match rotation {
            0 | 180 => [crop_width, crop_height],
            90 | 270 => [crop_height, crop_width],
            _ => {
                return Err(reject(record_index, "source page geometry is unsupported"));
            }
        };
        if source_surface
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(reject(
                record_index,
                "source physical dimensions are invalid",
            ));
        }
        imported_sources.push(ImportedSourcePage {
            source_key: prepared_record.source_key.clone(),
            source_page_index: prepared_record.source_page_index,
            source_to_display,
            source_crop: crop,
            physical_surface: source_surface,
        });
    }

    let source_objects = selected
        .page
        .objects
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != inspection_object_index)
        .collect::<Vec<_>>();
    if source_objects.iter().any(|(_, object)| {
        object.render_layer != crate::ObjectRenderLayer::Base
            || !matches!(object.content, crate::PageObjectContent::Stroke(_))
    }) {
        return Err(reject(
            0,
            "only root-level base-layer saved strokes are supported",
        ));
    }
    for (_, object) in &source_objects {
        let crate::PageObjectContent::Stroke(stroke) = &object.content else {
            unreachable!("non-stroke objects rejected above")
        };
        let Some(rendering) = stroke.rendering.as_ref() else {
            return Err(reject(0, "saved stroke rendering metadata is required"));
        };
        // Native ObjectStroke selection uses geometry/pen size, independent
        // of IsGenerated; retain this serialized provenance bit while applying
        // the same supported-style gates as ordinary saved strokes.
        if rendering.properties.replay_only
            || rendering.properties.eraser
            || rendering.properties.rainbow_effect
            || rendering.properties.reveal_mode
        {
            return Err(reject(0, "non-ordinary stroke styles are not supported"));
        }
    }

    struct BodyQueries<'a> {
        snapshot: Option<&'a ContinuousBodyQuerySnapshot>,
    }
    impl paper_segments::SegmentQueries for BodyQueries<'_> {
        fn body_cut(
            &self,
            band: [f32; 4],
        ) -> Result<paper_segments::BodyCutObservation, paper_segments::PlanError> {
            let Some(snapshot) = self.snapshot else {
                return Ok(paper_segments::BodyCutObservation {
                    visible: false,
                    line_index: None,
                    line_count: 0,
                    line_rect: [0.0; 4],
                });
            };
            let observation: crate::render::BodyQueryObservation = snapshot.query(band);
            Ok(paper_segments::BodyCutObservation {
                visible: observation.visible,
                line_index: observation
                    .line_index
                    .map(i32::try_from)
                    .transpose()
                    .map_err(|_| paper_segments::PlanError::ArithmeticOverflow)?,
                line_count: i32::try_from(observation.line_count)
                    .map_err(|_| paper_segments::PlanError::ArithmeticOverflow)?,
                line_rect: observation.line_rect,
            })
        }
    }
    let resolved = records
        .iter()
        .enumerate()
        .map(|(record_index, record)| {
            Ok(paper_segments::ResolvedRecord {
                source_key: &prepared.records[record_index].source_key,
                page_index: record.page_index,
                rectangle: {
                    let rectangle = paper_segments::RuntimeRect::from_saved(record.rectangle)
                        .map_err(|_| {
                            reject(record_index, "PDF-paper record geometry is invalid")
                        })?;
                    match fixed_axis_profile {
                        Some(profile) => rectangle.scaled(profile.scale).map_err(|_| {
                            reject(record_index, "scaled PDF-paper record geometry is invalid")
                        })?,
                        None => rectangle,
                    }
                },
                source_record_index: record_index,
            })
        })
        .collect::<Result<Vec<_>, PdfPaperAdmissionError>>()?;
    let segments = paper_segments::plan_segments(
        i32::try_from(note_width).map_err(|_| reject(0, "note width exceeds native range"))?,
        i32::try_from(note_height).map_err(|_| reject(0, "note height exceeds native range"))?,
        default_height_i32,
        &resolved,
        &BodyQueries {
            snapshot: body_query_snapshot,
        },
    )
    .map_err(|_| reject(0, "native page segmentation is unsupported"))?;

    let note_width_i32 =
        i32::try_from(note_width).map_err(|_| reject(0, "note width exceeds native range"))?;
    let note_height_i32 =
        i32::try_from(note_height).map_err(|_| reject(0, "note height exceeds native range"))?;
    let mut output_pages = Vec::new();
    let mut imported_record_count = 0_usize;
    let mut ordinary_pages_started = false;
    for segment in segments {
        let imported = !segment.record_indices.is_empty();
        if imported
            && (ordinary_pages_started || segment.record_indices.len() != segment.slices.len())
        {
            return Err(reject(
                output_pages.len(),
                "PDF-paper records must map one-to-one to an imported prefix segment",
            ));
        }
        if !imported {
            ordinary_pages_started = true;
        }
        let segment_records = if imported {
            segment.record_indices
        } else {
            Vec::new()
        };
        for (slice_index, slice) in segment.slices.into_iter().enumerate() {
            let imported_source = if imported {
                let record_index = segment_records[slice_index];
                if record_index != imported_record_count {
                    return Err(reject(
                        output_pages.len(),
                        "PDF-paper segment records are not in source order",
                    ));
                }
                let rect = paper_segments::RuntimeRect::from_saved(records[record_index].rectangle)
                    .and_then(|rect| match fixed_axis_profile {
                        Some(profile) => rect.scaled(profile.scale),
                        None => Ok(rect),
                    })
                    .map_err(|_| reject(record_index, "PDF-paper record geometry is invalid"))?;
                let matches_slice =
                    if fixed_axis_profile.is_some_and(|profile| profile.scale != 1.0) {
                        rect.matches_native_slice(slice.y, slice.height, default_height_i32)
                    } else {
                        match records[record_index].rectangle {
                            PdfPaperRectangle::Integer([0, top, _, bottom]) => {
                                slice.y == top && slice.height == bottom - top
                            }
                            _ => false,
                        }
                    };
                if !matches_slice {
                    return Err(reject(
                        output_pages.len(),
                        "native imported slice does not match its saved PDF-paper rectangle",
                    ));
                }
                imported_record_count += 1;
                Some(imported_sources[record_index].clone())
            } else {
                None
            };
            let query = [
                0.0,
                slice.y as f32,
                note_width as f32,
                slice.y.checked_add(slice.height).ok_or_else(|| {
                    reject(
                        output_pages.len(),
                        "query band overflows native coordinates",
                    )
                })? as f32,
            ];
            let mut selected_objects = Vec::new();
            for (object_index, object) in &source_objects {
                let crate::PageObjectContent::Stroke(stroke) = &object.content else {
                    unreachable!("non-stroke objects rejected above")
                };
                match paper_selection::admit_stroke(stroke, query) {
                    paper_selection::StrokeAdmission::Admit => selected_objects.push(*object_index),
                    paper_selection::StrokeAdmission::Reject => {}
                    paper_selection::StrokeAdmission::Unsupported(_) => {
                        return Err(reject(
                            output_pages.len(),
                            "stroke admission geometry is unsupported",
                        ));
                    }
                }
            }
            let (physical_width, physical_height) = if let Some(imported_source) = &imported_source
            {
                (
                    imported_source.physical_surface[0],
                    imported_source.physical_surface[1],
                )
            } else {
                let height =
                    native_surface_height(600, slice.height, note_width_i32).map_err(|_| {
                        reject(
                            output_pages.len(),
                            "continuation surface dimensions overflow",
                        )
                    })?;
                (600.0, f64::from(height))
            };
            let note_scale = physical_width / f64::from(note_width);
            let scene_height_f64 = (physical_height / note_scale).ceil();
            if !scene_height_f64.is_finite()
                || scene_height_f64 <= 0.0
                || scene_height_f64 > f64::from(u32::MAX)
            {
                return Err(reject(
                    output_pages.len(),
                    "physical scene viewport is invalid",
                ));
            }
            let scene_height = scene_height_f64 as u32;
            if scene_height == 0 || slice.y < 0 || slice.height <= 0 || slice.y >= note_height_i32 {
                return Err(reject(
                    output_pages.len(),
                    "query slice is outside the note",
                ));
            }
            output_pages.push(SingleXOutputPage {
                query_y: slice.y as u32,
                scene_height,
                physical_width,
                physical_height,
                imported_source,
                selected_objects,
            });
        }
    }
    if imported_record_count != records.len() {
        return Err(reject(
            output_pages.len(),
            "native segmentation did not import every contiguous PDF-paper record",
        ));
    }
    if output_pages.is_empty() {
        return Err(reject(0, "native segmentation produced no output pages"));
    }

    Ok(Some(SingleXPaperPlan {
        parsed_sources: prepared.sources,
        note_width,
        note_height,
        output_pages,
    }))
}

fn rich_text_is_benign_blank(text: &crate::RichTextBox) -> bool {
    // Native controls establish empty and LF-only text as not visible. Spaces,
    // tabs, CR, and other Unicode whitespace are deliberately not generalized
    // from that result: they may still affect native line measurement/cuts.
    // Widget HasVisibleText scans source UTF-16, independent of foreground,
    // font size or margins. Its paragraph fallback additionally admits bullets;
    // keep paragraph kinds restricted to Alignment and ParsingState below.
    let utf16_len = text.text.encode_utf16().count();
    text.text.chars().all(|character| character == '\n')
        && text.object_spans.is_empty()
        && text.runs.is_empty()
        && text.text_area_type.is_none()
        && text.bbox == crate::BoundingBox::default()
        && text.rotation_degrees.is_none()
        && text.highlight_color.is_none()
        && !text.underline
        && text
            .font_size
            .is_none_or(|size| size.is_finite() && size > 0.0)
        && text.text_sections.iter().all(|section| {
            section.start_utf16 >= 0
                && section.length_utf16 >= 0
                && section
                    .start_utf16
                    .checked_add(section.length_utf16)
                    .and_then(|end| usize::try_from(end).ok())
                    .is_some_and(|end| end <= utf16_len)
        })
        && text.margins.is_none_or(|margins| {
            margins
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
        })
        && text.gravity.is_none_or(|gravity| gravity == 0)
        && text.spans.iter().all(|span| match span.kind {
            crate::RichTextSpanType::FontSize => {
                matches!(span.payload.len(), 4 | 8)
                    && span
                        .font_size_value()
                        .is_some_and(|size| size.is_finite() && size > 0.0)
            }
            crate::RichTextSpanType::ForegroundColor => matches!(span.payload.len(), 4 | 8),
            _ => false,
        })
        && text.spans.iter().all(|span| {
            span.start_utf16 <= span.end_utf16
                && usize::try_from(span.end_utf16).is_ok_and(|end| end <= utf16_len)
        })
        && text
            .paragraphs
            .iter()
            .all(|paragraph| match paragraph.kind {
                crate::RichTextParagraphType::Alignment => {
                    matches!(
                        paragraph.alignment(),
                        Some(
                            crate::ParagraphAlignment::Left
                                | crate::ParagraphAlignment::Right
                                | crate::ParagraphAlignment::Center
                                | crate::ParagraphAlignment::Both
                        )
                    )
                }
                crate::RichTextParagraphType::ParsingState => paragraph.is_parsed().is_some(),
                _ => false,
            })
}

type PaperRunOwners = std::collections::HashMap<(usize, usize, usize), (usize, f64)>;

fn consider_paper_run_owner(
    owners: &mut PaperRunOwners,
    key: (usize, usize, usize),
    page_index: usize,
    ink: (f64, f64),
    clip: (f64, f64),
) {
    let overlap = (ink.1.min(clip.1) - ink.0.max(clip.0)).max(0.0);
    if overlap <= 0.0 {
        return;
    }
    let owner = owners.entry(key).or_insert((page_index, overlap));
    if overlap > owner.1 {
        *owner = (page_index, overlap);
    }
}

/// Render complete admitted objects into physical-covering per-page scenes, then
/// graft each scene once over its retained vector source page or continuation page.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_singlex_pdf_paper_plan(
    document: &crate::Document,
    layout: &crate::LayoutDocument,
    plan: SingleXPaperPlan,
    render_options: &crate::RenderOptions,
    pdf_options: &super::PdfOptions,
    fonts: &crate::fonts::FontBook,
    cache: &mut crate::render::DocumentTextCache,
    observer: &mut dyn FnMut(crate::Progress),
) -> Result<super::PdfOutput, super::PdfError> {
    if !pdf_options.dpi.is_finite() || pdf_options.dpi <= 0.0 {
        return Err(super::PdfError::InvalidDpi);
    }
    let source_layout = layout.pages.first().ok_or(super::PdfError::EmptyDocument)?;
    let mut scene_page = source_layout.clone();
    scene_page.page.width = plan.note_width;
    scene_page.page.height = plan.note_height;
    scene_page.page.background.pdf_paper = None;
    let inspection_object_index = source_layout
        .single_page_continuous_body(document)
        .and_then(|_| {
            source_layout
                .body_text_slice()
                .map(|body| body.object_index)
        });
    // Each scene puts the validated inspection node first, regardless of its
    // position in the source layout. PreparedBodyText intercepts it and paints
    // the full source body into this slice.
    let mut body_text = scene_page.body_text.clone();
    if let (Some(_), Some(body)) = (inspection_object_index, body_text.as_mut()) {
        body.object_index = 0;
    } else {
        body_text = None;
    }
    scene_page.body_text = body_text;
    let original_objects = &source_layout.page.objects;
    // PageSceneRequest borrows each page. Keep the page clones alive for the render call.
    let mut scene_pages = Vec::with_capacity(plan.output_pages.len());
    for output in &plan.output_pages {
        let mut page = scene_page.clone();
        page.page.objects = Vec::with_capacity(
            output.selected_objects.len() + usize::from(inspection_object_index.is_some()),
        );
        if let Some(index) = inspection_object_index {
            page.page.objects.push(original_objects[index].clone());
        }
        page.page.objects.extend(
            output
                .selected_objects
                .iter()
                .map(|index| original_objects[*index].clone()),
        );
        scene_pages.push(page);
    }
    let requests = plan
        .output_pages
        .iter()
        .zip(&scene_pages)
        .map(|(output, page)| {
            crate::render::PageSceneRequest::sliced(
                page,
                output.query_y,
                output.scene_height,
                false,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| super::PdfError::InvalidPageSize { page_index: 0 })?;
    let mut scenes = cache.render_layout_page_scene_requests(
        document,
        &requests,
        render_options,
        fonts,
        observer,
    );
    // The scene viewport rounds up to cover the physical page. Choose one
    // extraction owner per source line using the actual physical clip, while
    // retaining every intersecting scene's visible glyph outlines.
    let mut owners = PaperRunOwners::new();
    for (page_index, (scene, output)) in scenes.iter().zip(&plan.output_pages).enumerate() {
        let clip_top = f64::from(output.query_y);
        let clip_bottom =
            clip_top + output.physical_height * f64::from(plan.note_width) / output.physical_width;
        for (_, line, start, end, top, bottom) in scene.text.paper_lines() {
            consider_paper_run_owner(
                &mut owners,
                (line, start, end),
                page_index,
                (top, bottom),
                (clip_top, clip_bottom),
            );
        }
    }
    for (page_index, scene) in scenes.iter_mut().enumerate() {
        let nonowners = scene
            .text
            .paper_lines()
            .filter_map(|(id, line, start, end, _, _)| {
                let owner = owners.get(&(line, start, end)).map(|owner| owner.0);
                (owner != Some(page_index)).then_some((id, owner.is_none()))
            })
            .collect::<Vec<_>>();
        for (id, outside_all_clips) in nonowners {
            if outside_all_clips && scene.text.omit_outside_paper_clip(id) {
                continue;
            }
            scene.text.set_visual_only(id);
        }
    }
    let overlay_bytes = super::render_scenes_pdf_with_progress(&scenes, pdf_options, observer)?;
    let overlay_units = 72.0 / f64::from(pdf_options.dpi);
    let mut output_overlay = Document::load_mem(&overlay_bytes)
        .map_err(|_| super::PdfError::InvalidPageSize { page_index: 0 })?;
    for (index, output) in plan.output_pages.iter().enumerate() {
        if output.imported_source.is_some() {
            continue;
        }
        let page_number = u32::try_from(index + 1)
            .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?;
        constrain_overlay_page_in_document(
            &mut output_overlay,
            page_number,
            0,
            i32::try_from(plan.note_width)
                .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?,
            i32::try_from(output.scene_height)
                .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?,
            [output.physical_width as i32, output.physical_height as i32],
            overlay_units,
        )
        .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?;
    }
    let mut constrained_overlay_bytes = Vec::new();
    output_overlay
        .save_to(&mut constrained_overlay_bytes)
        .map_err(|_| super::PdfError::InvalidPageSize { page_index: 0 })?;
    let mut outputs = Vec::with_capacity(plan.output_pages.len());
    for (index, output) in plan.output_pages.iter().enumerate() {
        let overlay_page_number = u32::try_from(index + 1)
            .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?;
        if let Some(imported_source) = &output.imported_source {
            let (display_to_source, clip) = imported_overlay_placement(
                imported_source.source_to_display,
                imported_source.source_crop,
                [output.physical_width, output.physical_height],
                [f64::from(plan.note_width), f64::from(output.scene_height)],
                0.0,
                overlay_units,
            )
            .map_err(|_| super::PdfError::InvalidPageSize { page_index: index })?;
            outputs.push(OutputPage::Imported {
                source_key: &imported_source.source_key,
                source_page_index: imported_source.source_page_index,
                overlay_bytes: &constrained_overlay_bytes,
                overlay_page_number,
                display_to_source,
                clip,
            });
        } else {
            outputs.push(OutputPage::OverlayOnly {
                overlay_bytes: &constrained_overlay_bytes,
                overlay_page_number,
            });
        }
    }
    let assembled = assemble_prepared_output_pages(&plan.parsed_sources, &outputs)
        .map_err(|_| super::PdfError::InvalidPageSize { page_index: 0 })?;
    let pages = scenes
        .into_iter()
        .map(|scene| super::PdfPageDiagnostics {
            page_index: 0,
            source_page_index: scene.page.source_page_index,
            text_diagnostics: scene.page.text_diagnostics,
            object_diagnostics: scene.page.object_diagnostics,
            geometry_diagnostics: scene.page.geometry_diagnostics,
            paint_diagnostics: scene.page.paint_diagnostics,
            background_diagnostics: scene.page.background_diagnostics,
        })
        .collect();
    Ok(super::PdfOutput::new(assembled, pages))
}

/// Resolve saved PDF records in order and parse each bound source once.
fn prepare_pdf_paper_records(
    metadata: &DocumentMetadata,
    records: &[PdfPaperRecord],
) -> Result<PreparedPdfPaperRecords, PdfPaperAdmissionError> {
    let resolver = metadata.archive_resource_resolver();
    let mut sources = std::collections::BTreeMap::new();
    let mut prepared = Vec::with_capacity(records.len());
    for (record_index, record) in records.iter().enumerate() {
        if record.media_id == -1000 {
            return Err(PdfPaperAdmissionError::ExternalPdfUnavailable { record_index });
        }
        let media_id = u32::try_from(record.media_id).map_err(|_| {
            PdfPaperAdmissionError::NegativeBinding {
                record_index,
                media_id: record.media_id,
            }
        })?;
        let resolved =
            resolver
                .resolve(media_id)
                .map_err(|source| PdfPaperAdmissionError::Binding {
                    record_index,
                    media_id,
                    source,
                })?;
        let resource_name = resolved.resource.name.clone();
        if !sources.contains_key(&resource_name) {
            if resolved.data.len() > 1024 * 1024 * 1024 {
                return Err(PdfPaperAdmissionError::SourceTooLarge {
                    record_index,
                    resource: resource_name,
                });
            }
            let doc = Document::load_mem(resolved.data).map_err(|error| {
                PdfPaperAdmissionError::InvalidPdf {
                    record_index,
                    resource: resource_name.clone(),
                    message: error.to_string(),
                }
            })?;
            let pages = doc.get_pages();
            if pages.is_empty() {
                return Err(PdfPaperAdmissionError::EmptyPdf {
                    record_index,
                    resource: resource_name,
                });
            }
            sources.insert(resource_name.clone(), ParsedSource { doc, pages });
        }
        if record.page_index < 0 {
            return Err(PdfPaperAdmissionError::NegativePageIndex {
                record_index,
                page_index: record.page_index,
            });
        }
        let source = sources
            .get(&resource_name)
            .expect("source inserted or already cached");
        let page_number = u32::try_from(record.page_index)
            .ok()
            .and_then(|index| index.checked_add(1));
        if !page_number.is_some_and(|number| source.pages.contains_key(&number)) {
            return Err(PdfPaperAdmissionError::PageIndexOutOfRange {
                record_index,
                resource: resource_name,
                page_index: record.page_index,
                page_count: source.pages.len(),
            });
        }
        prepared.push(PreparedPdfPaperRecord {
            source_key: resource_name,
            source_page_index: record.page_index,
            inferred: resolved.inferred,
        });
    }
    Ok(PreparedPdfPaperRecords {
        sources,
        records: prepared,
    })
}

/// Assemble the exact output topology selected by the document-aware caller.
/// No fit, scale, page index, or clipping decision is inferred here.
#[cfg(test)]
fn assemble_output_pages(
    sources: &[SourcePdf<'_>],
    output_pages: &[OutputPage<'_>],
) -> Result<Vec<u8>, String> {
    let mut parsed = std::collections::BTreeMap::<String, ParsedSource>::new();
    for source in sources {
        if parsed.contains_key(source.key) {
            return Err(format!("duplicate source PDF key {:?}", source.key));
        }
        parsed.insert(
            source.key.to_owned(),
            parse_source_pdf(source.key, source.bytes)?,
        );
    }
    assemble_prepared_output_pages(&parsed, output_pages)
}

#[cfg(test)]
fn parse_source_pdf(key: &str, bytes: &[u8]) -> Result<ParsedSource, String> {
    if bytes.len() > 1024 * 1024 * 1024 {
        return Err(format!(
            "source PDF {key:?} exceeds the 1 GiB admission limit"
        ));
    }
    let doc = Document::load_mem(bytes).map_err(|error| error.to_string())?;
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err(format!("source PDF {key:?} has no pages"));
    }
    Ok(ParsedSource { doc, pages })
}

fn assemble_prepared_output_pages(
    parsed: &std::collections::BTreeMap<String, ParsedSource>,
    output_pages: &[OutputPage<'_>],
) -> Result<Vec<u8>, String> {
    if output_pages.is_empty() {
        return Err("PDF paper output must contain at least one page".into());
    }
    if output_pages.len() > 10_000 {
        return Err("PDF paper output exceeds 10,000 pages".into());
    }
    let mut overlays = Vec::<Document>::new();
    let mut overlay_indices = Vec::with_capacity(output_pages.len());
    let mut overlay_by_buffer = HashMap::<(usize, usize), usize>::new();
    let mut selected_sources = HashSet::<String>::new();
    for page in output_pages {
        let (bytes, number) = match page {
            OutputPage::Imported {
                source_key,
                source_page_index,
                overlay_bytes,
                overlay_page_number,
                display_to_source,
                clip,
            } => {
                if !display_to_source.iter().all(|v| v.is_finite()) {
                    return Err("overlay placement matrix contains non-finite values".into());
                }
                if !clip.iter().all(|v| v.is_finite())
                    || !(clip[2] - clip[0]).is_finite()
                    || !(clip[3] - clip[1]).is_finite()
                    || clip[2] <= clip[0]
                    || clip[3] <= clip[1]
                {
                    return Err("overlay clip must have positive finite width and height".into());
                }
                let source = parsed
                    .get(*source_key)
                    .ok_or_else(|| format!("missing source PDF binding {:?}", source_key))?;
                let number = u32::try_from(*source_page_index)
                    .ok()
                    .and_then(|v| v.checked_add(1))
                    .ok_or("source page index must be a nonnegative 32-bit index")?;
                if !source.pages.contains_key(&number) {
                    return Err(format!(
                        "source page index {} is out of range for {:?}",
                        source_page_index, source_key
                    ));
                }
                selected_sources.insert((*source_key).to_owned());
                (overlay_bytes, *overlay_page_number)
            }
            OutputPage::OverlayOnly {
                overlay_bytes,
                overlay_page_number,
            } => (overlay_bytes, *overlay_page_number),
        };
        if bytes.len() > 1024 * 1024 * 1024 {
            return Err("overlay PDF exceeds the 1 GiB admission limit".into());
        }
        let buffer_key = (bytes.as_ptr() as usize, bytes.len());
        let overlay_index = if let Some(index) = overlay_by_buffer.get(&buffer_key) {
            *index
        } else {
            let overlay = Document::load_mem(bytes).map_err(|e| e.to_string())?;
            let index = overlays.len();
            overlays.push(overlay);
            overlay_by_buffer.insert(buffer_key, index);
            index
        };
        let overlay = &overlays[overlay_index];
        if !overlay.get_pages().contains_key(&number) {
            return Err(format!("overlay page number {number} is missing"));
        }
        let overlay_page = overlay
            .get_dictionary(overlay.get_pages()[&number])
            .map_err(|e| e.to_string())?;
        if matches!(page, OutputPage::Imported { .. }) && overlay_page.has(b"Annots") {
            return Err(
                "overlay page annotations cannot be represented by the imported-page Form".into(),
            );
        }
        if !overlay_catalog_is_graftable(overlay)? {
            return Err(
                "overlay PDF has document-level context that cannot be retained by page grafting"
                    .into(),
            );
        }
        overlay_indices.push(overlay_index);
    }
    if selected_sources.is_empty() {
        return assemble_overlay_only(output_pages, &overlays, &overlay_indices);
    }

    // A single source catalog can be retained wholesale. Combining unrelated
    // catalogs would silently lose document-level rendering context (OCGs,
    // structure trees, output intents, AcroForm, etc.), so reject that case.
    if selected_sources.len() > 1 {
        for key in &selected_sources {
            let source = parsed.get(key).expect("selected source was parsed");
            if !catalog_is_page_tree_only(&source.doc)? {
                return Err(format!(
                    "cannot combine source PDF {:?}: document catalog has global context that cannot be merged safely",
                    key
                ));
            }
        }
    }

    let mut output = Document::with_version("1.7");
    let pages_root_id = output.new_object_id();
    let output_page_ids = (0..output_pages.len())
        .map(|_| output.new_object_id())
        .collect::<Vec<_>>();
    let mut tagged = TaggedOutput::new(&mut output);
    for (index, spec) in output_pages.iter().enumerate() {
        let overlay = &overlays[overlay_indices[index]];
        let overlay_number = match spec {
            OutputPage::Imported {
                overlay_page_number,
                ..
            }
            | OutputPage::OverlayOnly {
                overlay_page_number,
                ..
            } => *overlay_page_number,
        };
        tagged.reserve_page_keys(overlay, overlay.get_pages()[&overlay_number])?;
        if let OutputPage::Imported {
            source_key,
            source_page_index,
            ..
        } = spec
        {
            let source = parsed.get(*source_key).expect("validated source binding");
            let number = u32::try_from(*source_page_index).unwrap() + 1;
            tagged.reserve_page_keys(&source.doc, source.pages[&number])?;
        }
    }

    // A selected source page may be repeated. Cross-page references resolve to
    // its first output occurrence; each explicit output entry still owns a
    // distinct page object and annotation /P target.
    let mut page_remaps =
        std::collections::BTreeMap::<String, std::collections::HashMap<ObjectId, ObjectId>>::new();
    let mut object_maps =
        std::collections::BTreeMap::<String, std::collections::HashMap<ObjectId, ObjectId>>::new();
    for (spec, output_id) in output_pages.iter().zip(&output_page_ids) {
        if let OutputPage::Imported {
            source_key,
            source_page_index,
            ..
        } = spec
        {
            let source = parsed.get(*source_key).expect("validated source binding");
            let page_number = u32::try_from(*source_page_index).unwrap() + 1;
            let source_id = source.pages[&page_number];
            page_remaps
                .entry((*source_key).to_owned())
                .or_default()
                .entry(source_id)
                .or_insert(*output_id);
        }
    }

    // Clone the retained source catalog before page graphs so cycles and
    // references to selected pages can be remapped consistently.
    let first_source_key = selected_sources.iter().next().expect("nonempty");
    let source = parsed.get(first_source_key).expect("selected source");
    ensure_catalog_context_supported(&source.doc)?;
    let root_id = source
        .doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let source_catalog = source
        .doc
        .get_dictionary(root_id)
        .map_err(|e| e.to_string())?;
    let mut catalog = Dictionary::new();
    for (key, value) in source_catalog.iter() {
        if key == b"Type" || key == b"Pages" {
            continue;
        }
        let mut cloner = GraphCloner::new(
            &source.doc,
            &mut output,
            page_remaps.get(first_source_key),
            object_maps.entry(first_source_key.clone()).or_default(),
            pages_root_id,
        );
        catalog.set(key.clone(), cloner.clone_object(value, None)?);
    }
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_root_id);
    let catalog_id = output.add_object(catalog);

    for (index, (spec, output_id)) in output_pages.iter().zip(&output_page_ids).enumerate() {
        let overlay = &overlays[overlay_indices[index]];
        let overlay_number = match spec {
            OutputPage::Imported {
                overlay_page_number,
                ..
            }
            | OutputPage::OverlayOnly {
                overlay_page_number,
                ..
            } => *overlay_page_number,
        };
        match spec {
            OutputPage::Imported {
                source_key,
                source_page_index,
                display_to_source,
                clip,
                ..
            } => {
                let source = parsed.get(*source_key).expect("validated source binding");
                let source_page_number = u32::try_from(*source_page_index).unwrap() + 1;
                let source_page_id = source.pages[&source_page_number];
                let page_dict = source
                    .doc
                    .get_dictionary(source_page_id)
                    .map_err(|e| e.to_string())?;
                let overlay_page_id = overlay.get_pages()[&overlay_number];
                let structure_key = if tagged.page_has_tags(overlay, overlay_page_id)? {
                    Some(tagged.allocate_key())
                } else {
                    None
                };
                let mut cloner = GraphCloner::new(
                    &source.doc,
                    &mut output,
                    page_remaps.get(*source_key),
                    object_maps.entry((*source_key).to_owned()).or_default(),
                    pages_root_id,
                );
                let mut page = cloner.clone_page(page_dict, *output_id)?;
                drop(cloner);
                let form_id = add_overlay_form(
                    &mut output,
                    &mut page,
                    overlay,
                    overlay_page_id,
                    *display_to_source,
                    *clip,
                    structure_key,
                )?;
                if let Some(key) = structure_key {
                    tagged.append_page(
                        &mut output,
                        overlay,
                        overlay_page_id,
                        *output_id,
                        Some(form_id),
                        key,
                    )?;
                }
                output.objects.insert(*output_id, Object::Dictionary(page));
            }
            OutputPage::OverlayOnly { .. } => {
                let overlay_page_id = overlay.get_pages()[&overlay_number];
                let overlay_page = overlay
                    .get_dictionary(overlay_page_id)
                    .map_err(|e| e.to_string())?;
                let mut cloned_objects = std::collections::HashMap::new();
                let mut cloner = GraphCloner::new(
                    overlay,
                    &mut output,
                    None,
                    &mut cloned_objects,
                    pages_root_id,
                );
                let mut page = cloner.clone_page(overlay_page, *output_id)?;
                drop(cloner);
                if tagged.page_has_tags(overlay, overlay_page_id)? {
                    let key = tagged.allocate_key();
                    page.set("StructParents", key);
                    tagged.append_page(
                        &mut output,
                        overlay,
                        overlay_page_id,
                        *output_id,
                        None,
                        key,
                    )?;
                }
                output.objects.insert(*output_id, Object::Dictionary(page));
            }
        }
    }

    output.objects.insert(pages_root_id, Object::Dictionary(lopdf::dictionary! {
        "Type" => "Pages", "Kids" => output_page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
        "Count" => output_page_ids.len() as i64,
    }));
    if tagged.has_tags {
        let root = tagged.finish(&mut output)?;
        output
            .objects
            .insert(tagged.root_id, Object::Dictionary(root));
        let catalog = output
            .get_object_mut(catalog_id)
            .map_err(|e| e.to_string())?
            .as_dict_mut()
            .map_err(|e| e.to_string())?;
        catalog.set("StructTreeRoot", tagged.root_id);
        catalog.set("MarkInfo", lopdf::dictionary! { "Marked" => true });
    }
    output.trailer.set("Root", catalog_id);
    if output.objects.len() > 2_000_000 {
        return Err("assembled PDF exceeds 2,000,000 reachable and structural objects".into());
    }
    let mut bytes = Vec::new();
    output.save_modern(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn catalog_is_page_tree_only(doc: &Document) -> Result<bool, String> {
    let root = doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    Ok(catalog
        .iter()
        .all(|(key, _)| key == b"Type" || key == b"Pages" || key == b"Version"))
}

/// SDK-generated overlay PDFs carry a producer XMP metadata stream. It is
/// document-description metadata, not page paint; page grafting keeps all page
/// streams/resources and may omit this document-level record. Other catalog
/// entries can change painted output or carry page-indexed semantics.
fn overlay_catalog_is_graftable(doc: &Document) -> Result<bool, String> {
    let root = doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    for (key, value) in catalog.iter() {
        match key.as_slice() {
            b"Type" | b"Pages" | b"Version" | b"Metadata" => {}
            b"StructTreeRoot" => reject_unsupported_structure_root(obj_dict(doc, value)?)?,
            b"MarkInfo" => {
                let mark = obj_dict(doc, value)?;
                if mark
                    .iter()
                    .any(|(key, _)| key != b"Marked" && key != b"Suspects")
                {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn ensure_catalog_context_supported(doc: &Document) -> Result<(), String> {
    let root = doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    for (key, value) in catalog.iter() {
        match key.as_slice() {
            b"Type" | b"Pages" | b"Version" | b"OCProperties" | b"Metadata" | b"OutputIntents"
            | b"Lang" | b"ViewerPreferences" | b"MarkInfo" => {}
            b"Names" => validate_catalog_names(doc, value)?,
            b"OpenAction" => validate_catalog_open_action(doc, value)?,
            b"PageMode" => {
                if !matches!(deref(doc, value)?, Object::Name(_)) {
                    return Err("source PDF catalog /PageMode must be a name".into());
                }
            }
            b"StructTreeRoot" => {
                let structure = obj_dict(doc, value)?;
                let nonempty_k = match structure.get(b"K") {
                    Ok(k) => match deref(doc, k)?.as_array() {
                        Ok(items) => !items.is_empty(),
                        Err(_) => true,
                    },
                    Err(_) => false,
                };
                if structure.has(b"ParentTree")
                    || structure.has(b"RoleMap")
                    || structure.has(b"ClassMap")
                    || nonempty_k
                {
                    return Err(
                        "source PDF has a nonempty structure tree that cannot be remapped safely"
                            .into(),
                    );
                }
            }
            _ => {
                return Err(format!(
                    "source PDF catalog key {:?} has page/document semantics that cannot be retained safely",
                    String::from_utf8_lossy(key)
                ));
            }
        }
    }
    Ok(())
}

/// The single-source catalog copier can safely retain the /Dests name tree:
/// GraphCloner remaps selected page references and rejects dangling ones.
/// Other name-tree contexts (JavaScript, embedded files, etc.) need their own
/// merge/remapping policy and remain explicit admission errors.
fn validate_catalog_names(doc: &Document, value: &Object) -> Result<(), String> {
    let names = obj_dict(doc, value)?;
    if names.iter().any(|(key, _)| key != b"Dests") {
        return Err("source PDF catalog /Names contains an unsupported name-tree context".into());
    }
    if let Ok(dests) = names.get(b"Dests") {
        validate_destination_name_tree(doc, dests, &mut HashSet::new(), 0)?;
    }
    Ok(())
}

fn validate_destination_name_tree(
    doc: &Document,
    object: &Object,
    seen: &mut HashSet<ObjectId>,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err("source PDF destination name tree nesting exceeds 64".into());
    }
    if let Object::Reference(id) = object
        && !seen.insert(*id)
    {
        return Err("cycle or repeated node in source PDF destination name tree".into());
    }
    let node = deref(doc, object)?
        .as_dict()
        .map_err(|e| format!("source PDF destination name-tree node is not a dictionary: {e}"))?;
    if node
        .iter()
        .any(|(key, _)| key != b"Names" && key != b"Kids" && key != b"Limits")
    {
        return Err("source PDF destination name tree has unsupported node entries".into());
    }
    let names = node.get(b"Names").ok();
    let kids = node.get(b"Kids").ok();
    if names.is_some() == kids.is_some() {
        return Err(
            "source PDF destination name-tree node must have exactly one of /Names or /Kids".into(),
        );
    }
    if let Some(names) = names {
        let names = deref(doc, names)?
            .as_array()
            .map_err(|e| format!("source PDF destination name-tree /Names is not an array: {e}"))?;
        if names.len() % 2 != 0 {
            return Err("source PDF destination name-tree /Names has an odd item count".into());
        }
        for pair in names.as_chunks::<2>().0 {
            let name = &pair[0];
            if !matches!(deref(doc, name)?, Object::String(_, _) | Object::Name(_)) {
                return Err("source PDF destination name-tree key is not a string or name".into());
            }
            validate_destination_value(doc, &pair[1])?;
        }
    } else if let Some(kids) = kids {
        let kids = deref(doc, kids)?
            .as_array()
            .map_err(|e| format!("source PDF destination name-tree /Kids is not an array: {e}"))?;
        for child in kids {
            validate_destination_name_tree(doc, child, seen, depth + 1)?;
        }
    }
    if let Ok(limits) = node.get(b"Limits") {
        let limits = deref(doc, limits)?.as_array().map_err(|e| {
            format!("source PDF destination name-tree /Limits is not an array: {e}")
        })?;
        if limits.len() != 2
            || limits
                .iter()
                .any(|value| !matches!(deref(doc, value), Ok(Object::String(_, _))))
        {
            return Err("source PDF destination name-tree /Limits must contain two strings".into());
        }
    }
    Ok(())
}

fn validate_destination_value(doc: &Document, value: &Object) -> Result<(), String> {
    let destination = match deref(doc, value)? {
        Object::Array(items) => items,
        Object::Dictionary(dict) => {
            if dict.iter().any(|(key, _)| key != b"D") {
                return Err("source PDF destination dictionary has unsupported entries".into());
            }
            deref(
                doc,
                dict.get(b"D")
                    .map_err(|e| format!("source PDF destination dictionary has no /D: {e}"))?,
            )?
            .as_array()
            .map_err(|e| format!("source PDF destination dictionary /D is not an array: {e}"))?
        }
        _ => {
            return Err(
                "source PDF named destination is not an array or destination dictionary".into(),
            );
        }
    };
    if destination.is_empty() || !matches!(destination.first(), Some(Object::Reference(_))) {
        return Err("source PDF destination array must begin with a page reference".into());
    }
    Ok(())
}

fn validate_catalog_open_action(doc: &Document, value: &Object) -> Result<(), String> {
    let action = obj_dict(doc, value).map_err(|_| {
        "source PDF catalog /OpenAction must be a supported /GoTo action".to_owned()
    })?;
    if action
        .iter()
        .any(|(key, _)| key != b"Type" && key != b"S" && key != b"D")
    {
        return Err("source PDF catalog /OpenAction has unsupported action entries".into());
    }
    if let Some(action_type) = action.get(b"Type").ok()
        && deref(doc, action_type)?.as_name().ok() != Some(b"Action".as_slice())
    {
        return Err("source PDF catalog /OpenAction /Type must be /Action".into());
    }
    let action_kind = action
        .get(b"S")
        .map_err(|e| format!("source PDF catalog /OpenAction has no /S action type: {e}"))?;
    if deref(doc, action_kind)?.as_name().ok() != Some(b"GoTo".as_slice()) {
        return Err("source PDF catalog /OpenAction must use /S /GoTo".into());
    }
    let destination = action
        .get(b"D")
        .map_err(|e| format!("source PDF catalog /OpenAction has no /D destination: {e}"))?;
    match deref(doc, destination)? {
        Object::Array(items) if !items.is_empty() => {
            if !matches!(items.first(), Some(Object::Reference(_))) {
                return Err(
                    "source PDF /OpenAction GoTo destination must begin with a page reference"
                        .into(),
                );
            }
        }
        Object::Name(_) | Object::String(_, _) => {}
        _ => {
            return Err(
                "source PDF /OpenAction GoTo /D must be a destination array or name".into(),
            );
        }
    }
    Ok(())
}

/// Output structure tree state. Each output page occurrence gets independent
/// structure elements and a fresh ParentTree key, including duplicate pages.
struct TaggedOutput {
    root_id: ObjectId,
    next_key: i64,
    reserved_keys: HashSet<i64>,
    root_kids: Vec<Object>,
    parent_entries: std::collections::BTreeMap<i64, Vec<Object>>,
    role_map: Dictionary,
    has_tags: bool,
}

impl TaggedOutput {
    fn new(output: &mut Document) -> Self {
        Self {
            root_id: output.new_object_id(),
            next_key: 0,
            reserved_keys: HashSet::new(),
            root_kids: Vec::new(),
            parent_entries: Default::default(),
            role_map: Dictionary::new(),
            has_tags: false,
        }
    }

    fn page_has_tags(&self, doc: &Document, page_id: ObjectId) -> Result<bool, String> {
        let page = doc.get_dictionary(page_id).map_err(|e| e.to_string())?;
        let Some(key) = page.get(b"StructParents").ok() else {
            return Ok(false);
        };
        let key = deref(doc, key)?
            .as_i64()
            .map_err(|e| format!("page StructParents is not an integer: {e}"))?;
        let tree = struct_parent_tree(doc)?
            .ok_or("page has StructParents but catalog has no ParentTree")?;
        let values = tree
            .get(&key)
            .ok_or_else(|| format!("ParentTree has no page key {key}"))?;
        let values = deref(doc, values)?
            .as_array()
            .map_err(|e| format!("ParentTree page key {key} is not an array: {e}"))?;
        for value in values {
            if !matches!(deref(doc, value)?, Object::Null) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn allocate_key(&mut self) -> i64 {
        while self.reserved_keys.contains(&self.next_key)
            || self.parent_entries.contains_key(&self.next_key)
        {
            self.next_key += 1;
        }
        let key = self.next_key;
        self.next_key += 1;
        key
    }

    fn reserve_page_keys(&mut self, doc: &Document, page_id: ObjectId) -> Result<(), String> {
        let page = doc.get_dictionary(page_id).map_err(|e| e.to_string())?;
        if let Ok(value) = page.get(b"StructParents") {
            self.reserved_keys.insert(
                deref(doc, value)?
                    .as_i64()
                    .map_err(|e| format!("StructParents is not an integer: {e}"))?,
            );
        }
        let Some(annots) = page.get(b"Annots").ok() else {
            return Ok(());
        };
        for annot in deref(doc, annots)?.as_array().map_err(|e| e.to_string())? {
            let dict = obj_dict(doc, annot)?;
            if let Ok(value) = dict.get(b"StructParent") {
                self.reserved_keys.insert(
                    deref(doc, value)?
                        .as_i64()
                        .map_err(|e| format!("annotation StructParent is not an integer: {e}"))?,
                );
            }
        }
        Ok(())
    }

    fn append_page(
        &mut self,
        output: &mut Document,
        source: &Document,
        source_page_id: ObjectId,
        output_page_id: ObjectId,
        form_id: Option<ObjectId>,
        output_key: i64,
    ) -> Result<(), String> {
        let source_page = source
            .get_dictionary(source_page_id)
            .map_err(|e| e.to_string())?;
        let source_key = source_page
            .get(b"StructParents")
            .map_err(|e| e.to_string())?;
        let source_key = deref(source, source_key)?
            .as_i64()
            .map_err(|e| e.to_string())?;
        let parent_tree = struct_parent_tree(source)?.ok_or("tagged page has no ParentTree")?;
        let parent_array = deref(
            source,
            parent_tree
                .get(&source_key)
                .ok_or_else(|| format!("ParentTree has no page key {source_key}"))?,
        )?
        .as_array()
        .map_err(|e| format!("ParentTree page key {source_key} is not an array: {e}"))?
        .clone();
        let root_id = struct_tree_root_id(source)?.ok_or("tagged page has no StructTreeRoot")?;
        let root = source.get_dictionary(root_id).map_err(|e| e.to_string())?;
        self.merge_role_map(source, root)?;
        reject_unsupported_structure_root(root)?;
        let roots = structure_kids(source, root.get(b"K").map_err(|e| e.to_string())?)?;
        let mut state = TagCloneState {
            source_page_id,
            output_page_id,
            form_id,
            parent_array,
            source_key,
            output_parent_array: Vec::new(),
            seen_mcids: HashSet::new(),
            seen_elements: HashSet::new(),
            graph_objects: Default::default(),
        };
        for item in roots {
            if let Some(item) = clone_structure_item(
                source,
                output,
                item,
                self.root_id,
                None,
                None,
                &mut state,
                0,
            )? {
                self.root_kids.push(item);
            }
        }
        if state.seen_mcids.is_empty() {
            return Err("page StructParents has no matching structure-tree MCIDs".into());
        }
        for (mcid, value) in state.parent_array.iter().enumerate() {
            if !matches!(deref(source, value)?, Object::Null)
                && !state.seen_mcids.contains(&(mcid as i64))
            {
                return Err(format!(
                    "ParentTree key {source_key} has an unrepresented MCID {mcid}"
                ));
            }
        }
        self.parent_entries
            .insert(output_key, state.output_parent_array);
        self.has_tags = true;
        Ok(())
    }

    fn merge_role_map(&mut self, doc: &Document, root: &Dictionary) -> Result<(), String> {
        let Some(role_map) = root.get(b"RoleMap").ok() else {
            return Ok(());
        };
        let role_map = obj_dict(doc, role_map)?;
        for (name, target) in role_map.iter() {
            let target = deref(doc, target)?
                .as_name()
                .map_err(|e| format!("RoleMap target for {:?} is not a name: {e}", name))?;
            if let Ok(existing) = self.role_map.get(name) {
                if existing.as_name().ok() != Some(target) {
                    return Err(format!(
                        "conflicting overlay RoleMap definition for {:?}",
                        String::from_utf8_lossy(name)
                    ));
                }
            } else {
                self.role_map
                    .set(name.clone(), Object::Name(target.to_vec()));
            }
        }
        Ok(())
    }

    fn finish(&self, output: &mut Document) -> Result<Dictionary, String> {
        let mut nums = Vec::with_capacity(self.parent_entries.len() * 2);
        for (key, values) in &self.parent_entries {
            nums.push(Object::Integer(*key));
            nums.push(output.add_object(Object::Array(values.clone())).into());
        }
        let mut parent_tree = Dictionary::new();
        parent_tree.set("Nums", nums);
        let mut root = Dictionary::new();
        root.set("Type", "StructTreeRoot");
        root.set("K", Object::Array(self.root_kids.clone()));
        root.set("ParentTree", output.add_object(parent_tree));
        root.set("ParentTreeNextKey", self.next_key);
        if !self.role_map.is_empty() {
            root.set("RoleMap", self.role_map.clone());
        }
        Ok(root)
    }
}

struct TagCloneState {
    source_page_id: ObjectId,
    output_page_id: ObjectId,
    form_id: Option<ObjectId>,
    parent_array: Vec<Object>,
    source_key: i64,
    output_parent_array: Vec<Object>,
    seen_mcids: HashSet<i64>,
    seen_elements: HashSet<ObjectId>,
    graph_objects: std::collections::HashMap<ObjectId, ObjectId>,
}

fn struct_tree_root_id(doc: &Document) -> Result<Option<ObjectId>, String> {
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .map_err(|e| e.to_string())?
        .as_reference()
        .map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(catalog_id).map_err(|e| e.to_string())?;
    let Some(root) = catalog.get(b"StructTreeRoot").ok() else {
        return Ok(None);
    };
    match root {
        Object::Reference(id) => Ok(Some(*id)),
        _ => Err("StructTreeRoot must be indirect".into()),
    }
}

fn struct_parent_tree(
    doc: &Document,
) -> Result<Option<std::collections::BTreeMap<i64, Object>>, String> {
    let Some(root_id) = struct_tree_root_id(doc)? else {
        return Ok(None);
    };
    let root = doc.get_dictionary(root_id).map_err(|e| e.to_string())?;
    let Some(tree) = root.get(b"ParentTree").ok() else {
        return Ok(None);
    };
    fn visit(
        doc: &Document,
        node: &Object,
        output: &mut std::collections::BTreeMap<i64, Object>,
        seen: &mut HashSet<ObjectId>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("ParentTree nesting exceeds 64".into());
        }
        if let Object::Reference(id) = node
            && !seen.insert(*id)
        {
            return Err("cycle or repeated node in ParentTree".into());
        }
        let node = deref(doc, node)?;
        let dict = node.as_dict().map_err(|e| e.to_string())?;
        let nums = dict.get(b"Nums").ok();
        let kids = dict.get(b"Kids").ok();
        if nums.is_some() == kids.is_some() {
            return Err("ParentTree node must have exactly one of Nums or Kids".into());
        }
        if let Some(nums) = nums {
            let nums = deref(doc, nums)?.as_array().map_err(|e| e.to_string())?;
            if nums.len() % 2 != 0 {
                return Err("ParentTree Nums has an odd item count".into());
            }
            for pair in nums.as_chunks::<2>().0 {
                let key = deref(doc, &pair[0])?.as_i64().map_err(|e| e.to_string())?;
                if output.insert(key, pair[1].clone()).is_some() {
                    return Err(format!("duplicate ParentTree key {key}"));
                }
                if output.len() > 1_000_000 {
                    return Err("ParentTree exceeds 1,000,000 entries".into());
                }
            }
        } else if let Some(kids) = kids {
            for child in deref(doc, kids)?.as_array().map_err(|e| e.to_string())? {
                visit(doc, child, output, seen, depth + 1)?;
            }
        }
        Ok(())
    }
    let mut output = std::collections::BTreeMap::new();
    visit(doc, tree, &mut output, &mut HashSet::new(), 0)?;
    Ok(Some(output))
}

fn reject_unsupported_structure_root(root: &Dictionary) -> Result<(), String> {
    for (key, _) in root.iter() {
        match key.as_slice() {
            b"Type" | b"K" | b"ParentTree" | b"ParentTreeNextKey" | b"RoleMap" => {}
            b"ClassMap" | b"Namespaces" | b"IDTree" => {
                return Err(format!(
                    "overlay StructTreeRoot {:?} is not yet supported",
                    String::from_utf8_lossy(key)
                ));
            }
            _ => {
                return Err(format!(
                    "overlay StructTreeRoot key {:?} is not yet supported",
                    String::from_utf8_lossy(key)
                ));
            }
        }
    }
    Ok(())
}

fn structure_kids(doc: &Document, object: &Object) -> Result<Vec<Object>, String> {
    if let Object::Array(items) = object {
        return Ok(items.clone());
    }
    if matches!(object, Object::Null) {
        return Ok(Vec::new());
    }
    if let Object::Reference(_) = object
        && let Object::Array(items) = deref(doc, object)?
    {
        return Ok(items.clone());
    }
    Ok(vec![object.clone()])
}

fn check_page_parent_slot(
    doc: &Document,
    state: &TagCloneState,
    mcid: i64,
    source_element: Option<ObjectId>,
) -> Result<(), String> {
    let index = usize::try_from(mcid).map_err(|_| "negative MCID")?;
    let parent = state.parent_array.get(index).ok_or_else(|| {
        format!(
            "ParentTree key {} has no slot for MCID {mcid}",
            state.source_key
        )
    })?;
    if matches!(deref(doc, parent)?, Object::Null) {
        return Err(format!("ParentTree slot for MCID {mcid} is null"));
    }
    if let Some(expected) = source_element {
        let actual = parent
            .as_reference()
            .map_err(|e| format!("ParentTree MCID {mcid} parent is not indirect: {e}"))?;
        if actual != expected {
            return Err(format!(
                "ParentTree MCID {mcid} points to a different StructElem"
            ));
        }
    }
    Ok(())
}

fn page_matches(
    doc: &Document,
    explicit: Option<&Object>,
    inherited_page: Option<ObjectId>,
    state: &TagCloneState,
    mcid: i64,
    element_id: Option<ObjectId>,
) -> Result<bool, String> {
    let page = if let Some(page) = explicit {
        Some(
            page.as_reference()
                .map_err(|e| format!("structure Pg is not indirect: {e}"))?,
        )
    } else {
        inherited_page
    };
    if let Some(page) = page {
        if page != state.source_page_id {
            return Ok(false);
        }
    } else if element_id.is_none() {
        return Err("MCID without inherited Pg has no indirect owning StructElem".into());
    }
    check_page_parent_slot(doc, state, mcid, element_id)?;
    Ok(true)
}

// These walkers carry the document pair, structural parent/page context, clone state,
// and recursion depth explicitly so each recursive edge remains auditable.
#[allow(clippy::too_many_arguments)]
fn clone_structure_item(
    source: &Document,
    target: &mut Document,
    item: Object,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState,
    depth: usize,
) -> Result<Option<Object>, String> {
    if depth > 128 {
        return Err("StructElem nesting exceeds 128".into());
    }
    match item {
        Object::Integer(mcid) => {
            if !page_matches(
                source,
                None,
                inherited_page,
                state,
                mcid,
                source_parent_element,
            )? {
                return Ok(None);
            }
            if !state.seen_mcids.insert(mcid) {
                return Err(format!("duplicate MCID {mcid} in selected overlay page"));
            }
            let entry = parent_slot_mut(&mut state.output_parent_array, mcid)?;
            *entry = Object::Reference(parent_target_id);
            if let Some(form_id) = state.form_id {
                let mut mcr = Dictionary::new();
                mcr.set("Type", "MCR");
                mcr.set("Pg", state.output_page_id);
                mcr.set("Stm", form_id);
                mcr.set("MCID", mcid);
                Ok(Some(Object::Dictionary(mcr)))
            } else {
                Ok(Some(Object::Integer(mcid)))
            }
        }
        Object::Array(items) => {
            let mut copied = Vec::new();
            for child in items {
                if let Some(child) = clone_structure_item(
                    source,
                    target,
                    child,
                    parent_target_id,
                    source_parent_element,
                    inherited_page,
                    state,
                    depth + 1,
                )? {
                    copied.push(child);
                }
            }
            Ok((!copied.is_empty()).then_some(Object::Array(copied)))
        }
        Object::Reference(id) => {
            if !state.seen_elements.insert(id) {
                return Err("cycle or repeated StructElem reference in overlay K tree".into());
            }
            let object = source.get_object(id).map_err(|e| e.to_string())?.clone();
            clone_structure_dictionary(
                source,
                target,
                object,
                Some(id),
                parent_target_id,
                source_parent_element,
                inherited_page,
                state,
                depth + 1,
            )
        }
        Object::Dictionary(dict) => clone_structure_dictionary(
            source,
            target,
            Object::Dictionary(dict),
            None,
            parent_target_id,
            source_parent_element,
            inherited_page,
            state,
            depth + 1,
        ),
        Object::Null => Ok(None),
        other => Err(format!("unsupported StructElem K item: {other:?}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn clone_structure_dictionary(
    source: &Document,
    target: &mut Document,
    object: Object,
    source_id: Option<ObjectId>,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState,
    depth: usize,
) -> Result<Option<Object>, String> {
    let dict = match object {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => stream.dict,
        other => {
            return Err(format!(
                "StructElem K reference is not a dictionary: {other:?}"
            ));
        }
    };
    if dict
        .get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|name| name == b"MCR")
    {
        return clone_mcr(
            source,
            dict,
            parent_target_id,
            source_parent_element,
            inherited_page,
            state,
        );
    }
    if dict
        .get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|name| name == b"OBJR")
    {
        return Err("overlay tagged annotations (OBJR) are not yet supported".into());
    }
    if !dict.has(b"S") {
        return Err("StructElem K dictionary has neither /S nor /Type /MCR".into());
    }
    if dict.has(b"ID") || dict.has(b"Ref") {
        return Err("StructElem /ID and /Ref remapping is not yet supported".into());
    }
    let explicit_page = dict.get(b"Pg").ok();
    let page = match explicit_page {
        Some(value) => Some(
            value
                .as_reference()
                .map_err(|e| format!("StructElem Pg is not indirect: {e}"))?,
        ),
        None => inherited_page,
    };
    if page.is_some_and(|page| page != state.source_page_id) {
        return Ok(None);
    }

    let target_id = target.new_object_id();
    target.objects.insert(target_id, Object::Null);
    let mut new_dict = Dictionary::new();
    let mut any_content = false;
    for (key, value) in dict.iter() {
        match key.as_slice() {
            b"P" => {}
            b"Pg" => new_dict.set("Pg", state.output_page_id),
            b"K" => {
                let children = structure_kids(source, value)?;
                let mut copied = Vec::new();
                for child in children {
                    if let Some(child) = clone_structure_item(
                        source,
                        target,
                        child,
                        target_id,
                        source_id,
                        page,
                        state,
                        depth + 1,
                    )? {
                        copied.push(child);
                    }
                }
                if !copied.is_empty() {
                    any_content = true;
                    new_dict.set("K", Object::Array(copied));
                }
            }
            _ => {
                let page_remap = HashMap::from([(state.source_page_id, state.output_page_id)]);
                let mut cloner = GraphCloner::new(
                    source,
                    target,
                    Some(&page_remap),
                    &mut state.graph_objects,
                    (0, 0),
                );
                new_dict.set(key.clone(), cloner.clone_object(value, None)?);
            }
        }
    }
    if !any_content {
        target.objects.remove(&target_id);
        return Ok(None);
    }
    new_dict.set("P", Object::Reference(parent_target_id));
    target
        .objects
        .insert(target_id, Object::Dictionary(new_dict));
    Ok(Some(Object::Reference(target_id)))
}

fn clone_mcr(
    source: &Document,
    dict: Dictionary,
    parent_target_id: ObjectId,
    source_parent_element: Option<ObjectId>,
    inherited_page: Option<ObjectId>,
    state: &mut TagCloneState,
) -> Result<Option<Object>, String> {
    if dict.has(b"Stm") {
        return Err("nested tagged Form content (/MCR /Stm) is not yet supported".into());
    }
    let mcid = dict
        .get(b"MCID")
        .map_err(|e| e.to_string())?
        .as_i64()
        .map_err(|e| e.to_string())?;
    if !page_matches(
        source,
        dict.get(b"Pg").ok(),
        inherited_page,
        state,
        mcid,
        source_parent_element,
    )? {
        return Ok(None);
    }
    if !state.seen_mcids.insert(mcid) {
        return Err(format!("duplicate MCID {mcid} in selected overlay page"));
    }
    let entry = parent_slot_mut(&mut state.output_parent_array, mcid)?;
    *entry = Object::Reference(parent_target_id);
    let mut mcr = Dictionary::new();
    mcr.set("Type", "MCR");
    mcr.set("Pg", state.output_page_id);
    if let Some(form_id) = state.form_id {
        mcr.set("Stm", form_id);
    }
    mcr.set("MCID", mcid);
    Ok(Some(Object::Dictionary(mcr)))
}

fn parent_slot_mut(array: &mut Vec<Object>, mcid: i64) -> Result<&mut Object, String> {
    let index = usize::try_from(mcid).map_err(|_| "negative MCID")?;
    if index > 1_000_000 {
        return Err("MCID exceeds 1,000,000".into());
    }
    if array.len() <= index {
        array.resize(index + 1, Object::Null);
    }
    Ok(&mut array[index])
}

fn assemble_overlay_only(
    specs: &[OutputPage<'_>],
    overlays: &[Document],
    overlay_indices: &[usize],
) -> Result<Vec<u8>, String> {
    let mut output = Document::with_version("1.7");
    let pages_root_id = output.new_object_id();
    let page_ids = (0..specs.len())
        .map(|_| output.new_object_id())
        .collect::<Vec<_>>();
    let mut tagged = TaggedOutput::new(&mut output);
    for (index, (spec, page_id)) in specs.iter().zip(&page_ids).enumerate() {
        let OutputPage::OverlayOnly {
            overlay_page_number,
            ..
        } = spec
        else {
            unreachable!("overlay-only assembly has no imported page")
        };
        let overlay = &overlays[overlay_indices[index]];
        let source_page_id = overlay.get_pages()[overlay_page_number];
        let source_page = overlay
            .get_dictionary(source_page_id)
            .map_err(|e| e.to_string())?;
        let mut cloned_objects = std::collections::HashMap::new();
        let mut cloner = GraphCloner::new(
            overlay,
            &mut output,
            None,
            &mut cloned_objects,
            pages_root_id,
        );
        let mut page = cloner.clone_page(source_page, *page_id)?;
        drop(cloner);
        if tagged.page_has_tags(overlay, source_page_id)? {
            let key = tagged.allocate_key();
            page.set("StructParents", key);
            tagged.append_page(&mut output, overlay, source_page_id, *page_id, None, key)?;
        }
        output.objects.insert(*page_id, Object::Dictionary(page));
    }
    output.objects.insert(pages_root_id, Object::Dictionary(lopdf::dictionary! {
        "Type" => "Pages", "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
        "Count" => page_ids.len() as i64,
    }));
    let catalog_id =
        output.add_object(lopdf::dictionary! {"Type" => "Catalog", "Pages" => pages_root_id});
    if tagged.has_tags {
        let root = tagged.finish(&mut output)?;
        output
            .objects
            .insert(tagged.root_id, Object::Dictionary(root));
        let catalog = output
            .get_object_mut(catalog_id)
            .map_err(|e| e.to_string())?
            .as_dict_mut()
            .map_err(|e| e.to_string())?;
        catalog.set("StructTreeRoot", tagged.root_id);
        catalog.set("MarkInfo", lopdf::dictionary! { "Marked" => true });
    }
    output.trailer.set("Root", catalog_id);
    if output.objects.len() > 2_000_000 {
        return Err("assembled PDF exceeds 2,000,000 reachable and structural objects".into());
    }
    let mut bytes = Vec::new();
    output.save_modern(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn is_annotation_dictionary(dict: &Dictionary) -> bool {
    dict.get(b"Type")
        .and_then(Object::as_name)
        .is_ok_and(|kind| kind == b"Annot")
        || (dict.has(b"Subtype") && dict.has(b"Rect"))
}

struct GraphCloner<'a> {
    source: &'a Document,
    target: &'a mut Document,
    page_remap: Option<&'a std::collections::HashMap<ObjectId, ObjectId>>,
    pages_root_id: ObjectId,
    objects: &'a mut std::collections::HashMap<ObjectId, ObjectId>,
    annotation_objects: std::collections::HashMap<ObjectId, ObjectId>,
    source_pages: HashSet<ObjectId>,
    cloned_count: usize,
}

impl<'a> GraphCloner<'a> {
    fn new(
        source: &'a Document,
        target: &'a mut Document,
        page_remap: Option<&'a std::collections::HashMap<ObjectId, ObjectId>>,
        objects: &'a mut std::collections::HashMap<ObjectId, ObjectId>,
        pages_root_id: ObjectId,
    ) -> Self {
        let source_pages = source.get_pages().into_values().collect();
        Self {
            source,
            target,
            page_remap,
            pages_root_id,
            objects,
            annotation_objects: Default::default(),
            source_pages,
            cloned_count: 0,
        }
    }

    fn clone_page(
        &mut self,
        page: &Dictionary,
        target_page: ObjectId,
    ) -> Result<Dictionary, String> {
        let mut result = Dictionary::new();
        for (key, value) in page.iter() {
            if key == b"Parent" {
                continue;
            }
            if key == b"Annots" {
                let annotations = self.clone_annotation_list(value, target_page, 0)?;
                result.set(key.clone(), annotations);
            } else {
                result.set(key.clone(), self.clone_object(value, Some(target_page))?);
            }
        }
        for key in [b"MediaBox" as &[u8], b"CropBox", b"Rotate", b"Resources"] {
            if result.has(key) {
                continue;
            }
            if let Some(value) = inherited_optional(self.source, page, key)? {
                result.set(key, self.clone_object(value, Some(target_page))?);
            }
        }
        if !result.has(b"MediaBox") {
            return Err("selected page has no inherited MediaBox".into());
        }
        result.set("Parent", Object::Reference(self.pages_root_id));
        Ok(result)
    }

    fn clone_object(
        &mut self,
        object: &Object,
        current_page: Option<ObjectId>,
    ) -> Result<Object, String> {
        self.clone_object_at(object, current_page, 0)
    }

    fn clone_annotation_list(
        &mut self,
        object: &Object,
        current_page: ObjectId,
        depth: usize,
    ) -> Result<Object, String> {
        if depth > 128 {
            return Err("annotation array nesting exceeds 128".into());
        }
        match object {
            Object::Array(items) => Ok(Object::Array(
                items
                    .iter()
                    .map(|item| self.clone_annotation_list(item, current_page, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            Object::Reference(id) => {
                if let Some(mapped) = self.annotation_objects.get(id) {
                    return Ok(Object::Reference(*mapped));
                }
                let source_object = self
                    .source
                    .get_object(*id)
                    .map_err(|e| e.to_string())?
                    .clone();
                if matches!(&source_object, Object::Array(_)) {
                    let target_id = self.target.new_object_id();
                    self.annotation_objects.insert(*id, target_id);
                    self.target.objects.insert(target_id, Object::Null);
                    let cloned =
                        self.clone_annotation_list(&source_object, current_page, depth + 1)?;
                    self.target.objects.insert(target_id, cloned);
                    Ok(Object::Reference(target_id))
                } else {
                    self.clone_object_at(object, Some(current_page), depth + 1)
                }
            }
            _ => self.clone_object_at(object, Some(current_page), depth + 1),
        }
    }

    fn clone_object_at(
        &mut self,
        object: &Object,
        current_page: Option<ObjectId>,
        depth: usize,
    ) -> Result<Object, String> {
        if depth > 128 {
            return Err("PDF object graph nesting exceeds 128".into());
        }
        match object {
            Object::Reference(id) => {
                if let Some(mapped) = self.page_remap.and_then(|m| m.get(id)) {
                    return Ok(Object::Reference(*mapped));
                }
                if self.source_pages.contains(id) {
                    return Err("reachable source graph references an unselected page".into());
                }
                let is_annotation = self
                    .source
                    .get_object(*id)
                    .ok()
                    .and_then(|o| o.as_dict().ok())
                    .is_some_and(is_annotation_dictionary);
                if is_annotation && let Some(mapped) = self.annotation_objects.get(id) {
                    return Ok(Object::Reference(*mapped));
                }
                if !is_annotation && let Some(mapped) = self.objects.get(id) {
                    return Ok(Object::Reference(*mapped));
                }
                self.cloned_count += 1;
                if self.cloned_count > 1_000_000 {
                    return Err("reachable PDF object graph exceeds 1,000,000 objects".into());
                }
                let target_id = self.target.new_object_id();
                if is_annotation {
                    self.annotation_objects.insert(*id, target_id);
                } else {
                    self.objects.insert(*id, target_id);
                }
                self.target.objects.insert(target_id, Object::Null);
                let source = self
                    .source
                    .get_object(*id)
                    .map_err(|e| e.to_string())?
                    .clone();
                let copied = self.clone_object_at(&source, current_page, depth + 1)?;
                self.target.objects.insert(target_id, copied);
                Ok(Object::Reference(target_id))
            }
            Object::Dictionary(dict) => {
                let mut copied = Dictionary::new();
                let is_annotation = is_annotation_dictionary(dict);
                for (key, value) in dict.iter() {
                    if key == b"P" && is_annotation && current_page.is_some() {
                        continue;
                    }
                    copied.set(
                        key.clone(),
                        self.clone_object_at(value, current_page, depth + 1)?,
                    );
                }
                if is_annotation && let Some(page) = current_page {
                    copied.set("P", Object::Reference(page));
                }
                Ok(Object::Dictionary(copied))
            }
            Object::Stream(stream) => {
                let mut copied = stream.clone();
                let mut dict = Dictionary::new();
                for (key, value) in stream.dict.iter() {
                    dict.set(
                        key.clone(),
                        self.clone_object_at(value, current_page, depth + 1)?,
                    );
                }
                copied.dict = dict;
                Ok(Object::Stream(copied))
            }
            Object::Array(items) => Ok(Object::Array(
                items
                    .iter()
                    .map(|item| self.clone_object_at(item, current_page, depth + 1))
                    .collect::<Result<_, _>>()?,
            )),
            other => Ok(other.clone()),
        }
    }
}

fn add_overlay_form(
    target: &mut Document,
    page: &mut Dictionary,
    overlay: &Document,
    overlay_page_id: ObjectId,
    display_to_source: [f64; 6],
    clip: [f64; 4],
    struct_parent_key: Option<i64>,
) -> Result<ObjectId, String> {
    let overlay_page = overlay
        .get_dictionary(overlay_page_id)
        .map_err(|e| e.to_string())?;
    let (overlay_matrix, overlay_crop) = page_to_display(overlay, overlay_page)?;
    let content = contents_bytes(overlay, overlay_page)?;
    let mut cloned_objects = std::collections::HashMap::new();
    let mut cloner = GraphCloner::new(overlay, target, None, &mut cloned_objects, (0, 0));
    let resources = match inherited_optional(overlay, overlay_page, b"Resources")? {
        Some(value) => cloner.clone_object(value, None)?,
        None => Object::Dictionary(Dictionary::new()),
    };
    let group = overlay_page
        .get(b"Group")
        .ok()
        .map(|v| cloner.clone_object(v, None))
        .transpose()?;
    drop(cloner);
    let mut form = Dictionary::new();
    form.set("Type", "XObject");
    form.set("Subtype", "Form");
    form.set("FormType", 1);
    form.set(
        "BBox",
        overlay_crop
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect::<Vec<_>>(),
    );
    form.set(
        "Matrix",
        overlay_matrix
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect::<Vec<_>>(),
    );
    form.set("Resources", resources);
    if let Some(key) = struct_parent_key {
        form.set("StructParents", key);
    }
    if let Some(group) = group {
        form.set("Group", group);
    }
    let form_id = target.add_object(Stream::new(form, content));
    let mut resources = match page.get(b"Resources") {
        Ok(value) => obj_dict(target, value)?.clone(),
        Err(_) => Dictionary::new(),
    };
    let mut xobjects = resources
        .get(b"XObject")
        .ok()
        .map(|v| obj_dict(target, v).cloned())
        .transpose()?
        .unwrap_or_default();
    let mut name = b"SDOCXOverlay".to_vec();
    let mut suffix = 0;
    while xobjects.has(&name) {
        suffix += 1;
        name = format!("SDOCXOverlay{suffix}").into_bytes();
    }
    xobjects.set(name.clone(), form_id);
    resources.set("XObject", xobjects);
    page.set("Resources", resources);
    let command = format!(
        "n q {} {} {} {} re W n {} {} {} {} {} {} cm /{} Do Q\n",
        clip[0],
        clip[1],
        clip[2] - clip[0],
        clip[3] - clip[1],
        display_to_source[0],
        display_to_source[1],
        display_to_source[2],
        display_to_source[3],
        display_to_source[4],
        display_to_source[5],
        String::from_utf8_lossy(&name),
    );
    let depth = source_graphics_state_depth(&contents_bytes(target, page)?)?;
    let original = match page.get(b"Contents") {
        Ok(contents) => content_items(target, contents)?,
        Err(_) => Vec::new(),
    };
    let prefix_id = target.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let mut balance = Vec::with_capacity((depth + 2) * 2);
    balance.extend_from_slice(b"n\n");
    for _ in 0..=depth {
        balance.extend_from_slice(b"Q\n");
    }
    let balance_id = target.add_object(Stream::new(Dictionary::new(), balance));
    let content_id = target.add_object(Stream::new(Dictionary::new(), command.into_bytes()));
    let mut contents = vec![prefix_id.into()];
    contents.extend(original);
    contents.push(balance_id.into());
    contents.push(content_id.into());
    page.set("Contents", contents);
    Ok(form_id)
}

#[cfg(test)]
const SDK_OVERLAY_PDF_UNITS_PER_NOTE_UNIT_AT_96_DPI: f64 = 0.75;

/// Reproduce the measured SingleX integer AddPage height calculation. Query height and
/// physical output height remain separate because this truncating scale can undershoot.
fn native_surface_height(
    output_width: i32,
    query_height: i32,
    note_width: i32,
) -> Result<i32, String> {
    if output_width <= 0 || query_height <= 0 || note_width <= 0 {
        return Err("native page dimensions must be positive".into());
    }
    let height = output_width
        .checked_mul(query_height)
        .ok_or("native output page height overflow")?
        / note_width;
    if height <= 0 {
        return Err("native output page height is empty".into());
    }
    Ok(height)
}

/// Map a full-note SDK overlay PDF page to one measured imported source surface.
/// `source_to_display` and `source_crop` come from the source PDF page tree; physical
/// dimensions and note dimensions are explicit caller inputs. This composes PDF geometry;
/// it does not establish that the native writer honors `/UserUnit` in the same way.
fn imported_overlay_placement(
    source_to_display: [f64; 6],
    source_crop: [f64; 4],
    source_surface: [f64; 2],
    note_size: [f64; 2],
    note_offset: f64,
    overlay_pdf_units_per_note_unit: f64,
) -> Result<([f64; 6], [f64; 4]), String> {
    if !source_to_display
        .iter()
        .chain(source_crop.iter())
        .chain(source_surface.iter())
        .chain(note_size.iter())
        .chain(std::iter::once(&note_offset))
        .chain(std::iter::once(&overlay_pdf_units_per_note_unit))
        .all(|value| value.is_finite())
        || source_surface.iter().any(|value| *value <= 0.0)
        || note_size.iter().any(|value| *value <= 0.0)
        || note_offset < 0.0
        || note_offset >= note_size[1]
        || overlay_pdf_units_per_note_unit <= 0.0
        || source_crop[2] <= source_crop[0]
        || source_crop[3] <= source_crop[1]
    {
        return Err("imported overlay geometry must be finite and positive".into());
    }
    let overlay_to_display = note_canvas_to_physical_affine(
        note_size,
        note_offset,
        source_surface,
        overlay_pdf_units_per_note_unit,
    )?;
    let display_to_source = compose_affine(
        invert_affine(source_to_display).ok_or("source display transform is singular")?,
        overlay_to_display,
    );
    if !display_to_source.iter().all(|value| value.is_finite()) {
        return Err("imported overlay matrix is non-finite".into());
    }
    Ok((display_to_source, source_crop))
}

/// Convert full-note SDK PDF coordinates to one physical output surface while
/// preserving the native note offset and width-derived scale.
fn note_canvas_to_physical_affine(
    note_size: [f64; 2],
    note_offset: f64,
    physical_surface: [f64; 2],
    overlay_pdf_units_per_note_unit: f64,
) -> Result<[f64; 6], String> {
    if !note_size
        .iter()
        .chain(physical_surface.iter())
        .chain(std::iter::once(&note_offset))
        .chain(std::iter::once(&overlay_pdf_units_per_note_unit))
        .all(|value| value.is_finite())
        || note_size.iter().any(|value| *value <= 0.0)
        || physical_surface.iter().any(|value| *value <= 0.0)
        || note_offset < 0.0
        || note_offset >= note_size[1]
        || overlay_pdf_units_per_note_unit <= 0.0
    {
        return Err("note and physical surface geometry must be finite and positive".into());
    }
    let scale = physical_surface[0] / note_size[0];
    let pdf_scale = scale / overlay_pdf_units_per_note_unit;
    let translation_y = physical_surface[1] - scale * (note_size[1] - note_offset);
    let matrix = [pdf_scale, 0.0, 0.0, pdf_scale, 0.0, translation_y];
    if matrix.iter().all(|value| value.is_finite()) {
        Ok(matrix)
    } else {
        Err("note-to-surface transform is non-finite".into())
    }
}

/// Wrap a full-note overlay with an independent physical page box and clip. The scene
/// retains complete selected object paths; only the PDF page's physical surface clips ink.
#[cfg(test)]
fn constrain_overlay_page(
    bytes: &[u8],
    page_number: u32,
    query_offset: i32,
    note_width: i32,
    note_height: i32,
    physical_size: [i32; 2],
    overlay_pdf_units_per_note_unit: f64,
) -> Result<Vec<u8>, String> {
    let mut pdf = Document::load_mem(bytes).map_err(|error| error.to_string())?;
    constrain_overlay_page_in_document(
        &mut pdf,
        page_number,
        query_offset,
        note_width,
        note_height,
        physical_size,
        overlay_pdf_units_per_note_unit,
    )?;
    let mut output = Vec::new();
    pdf.save_to(&mut output)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

fn constrain_overlay_page_in_document(
    pdf: &mut Document,
    page_number: u32,
    query_offset: i32,
    note_width: i32,
    note_height: i32,
    physical_size: [i32; 2],
    overlay_pdf_units_per_note_unit: f64,
) -> Result<(), String> {
    let [physical_width, physical_height] = physical_size;
    if query_offset < 0
        || note_width <= 0
        || note_height <= 0
        || physical_width <= 0
        || physical_height <= 0
        || query_offset >= note_height
    {
        return Err("note and physical surface dimensions must be positive".into());
    }
    let page_id = *pdf
        .get_pages()
        .get(&page_number)
        .ok_or("missing overlay page")?;
    let page = pdf
        .get_dictionary(page_id)
        .map_err(|error| error.to_string())?
        .clone();
    let (overlay_to_display, overlay_crop) = page_to_display(pdf, &page)?;
    if !overlay_pdf_units_per_note_unit.is_finite() || overlay_pdf_units_per_note_unit <= 0.0 {
        return Err("overlay PDF scale must be finite and positive".into());
    }
    let expected_crop = [
        0.0,
        0.0,
        f64::from(note_width) * overlay_pdf_units_per_note_unit,
        f64::from(note_height) * overlay_pdf_units_per_note_unit,
    ];
    if overlay_to_display != [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
        || overlay_crop
            .iter()
            .zip(expected_crop)
            .any(|(actual, expected)| (actual - expected).abs() > 1e-6)
    {
        return Err("SDK overlay page must have an origin-zero, unrotated 96-DPI scene box".into());
    }
    let content = contents_bytes(pdf, &page)?;
    let [pdf_scale, _, _, _, _, translate_y] = note_canvas_to_physical_affine(
        [f64::from(note_width), f64::from(note_height)],
        f64::from(query_offset),
        [f64::from(physical_width), f64::from(physical_height)],
        overlay_pdf_units_per_note_unit,
    )?;
    let prefix = format!(
        "q 0 0 {physical_width} {physical_height} re W n {pdf_scale:.12} 0 0 {pdf_scale:.12} 0 {translate_y:.12} cm\n"
    );
    let prefix_id = pdf.add_object(Stream::new(Dictionary::new(), prefix.into_bytes()));
    let body_id = pdf.add_object(Stream::new(Dictionary::new(), content));
    let suffix_id = pdf.add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));
    let dict = pdf
        .get_dictionary_mut(page_id)
        .map_err(|error| error.to_string())?;
    dict.set(
        "MediaBox",
        vec![
            0.into(),
            0.into(),
            physical_width.into(),
            physical_height.into(),
        ],
    );
    dict.set(
        "CropBox",
        vec![
            0.into(),
            0.into(),
            physical_width.into(),
            physical_height.into(),
        ],
    );
    dict.set("Rotate", 0);
    dict.set("UserUnit", 1);
    dict.set(
        "Contents",
        vec![prefix_id.into(), body_id.into(), suffix_id.into()],
    );
    Ok(())
}

fn invert_affine(m: [f64; 6]) -> Option<[f64; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if !det.is_finite() || det == 0.0 {
        return None;
    }
    let a = m[3] / det;
    let b = -m[1] / det;
    let c = -m[2] / det;
    let d = m[0] / det;
    Some([a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])])
}

fn compose_affine(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

#[cfg(test)]
fn transform_point(matrix: [f64; 6], point: [f64; 2]) -> [f64; 2] {
    [
        matrix[0] * point[0] + matrix[2] * point[1] + matrix[4],
        matrix[1] * point[0] + matrix[3] * point[1] + matrix[5],
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn paper_run_owners_follow_each_runs_actual_clip_and_ignore_empty_overlap() {
        let mut owners = super::PaperRunOwners::new();
        let first = (7, 10, 11);
        let second = (7, 11, 12);
        let repeated_line = (8, 12, 13);
        let fringe = (9, 13, 14);
        for (page, clip) in [(0, (0.0, 10.0)), (1, (10.0, 20.0))] {
            super::consider_paper_run_owner(&mut owners, first, page, (8.0, 11.0), clip);
            super::consider_paper_run_owner(&mut owners, second, page, (9.0, 13.0), clip);
            super::consider_paper_run_owner(&mut owners, repeated_line, page, (12.0, 13.0), clip);
            super::consider_paper_run_owner(&mut owners, fringe, page, (20.0, 21.0), clip);
        }
        assert_eq!(owners.get(&first).map(|owner| owner.0), Some(0));
        assert_eq!(owners.get(&second).map(|owner| owner.0), Some(1));
        assert_eq!(owners.get(&repeated_line).map(|owner| owner.0), Some(1));
        assert!(!owners.contains_key(&fringe));
    }
    use super::*;
    use crate::{
        ArchiveResource, ArchiveResourceContent, MediaManifest, MediaManifestEntry, PdfOptions,
        PdfPaperRectangle, RenderedPage,
    };
    use lopdf::{Object, dictionary};
    use std::sync::Arc;

    fn assert_modern_pdf_serialization(doc: &Document) {
        assert_eq!(doc.version, "1.7");
        assert!(doc.objects.values().any(|object| matches!(
            object,
            Object::Stream(stream)
                if stream.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|name| name == b"ObjStm")
        )), "final output should use compressed object streams");
        assert!(doc.objects.values().any(|object| matches!(
            object,
            Object::Stream(stream)
                if stream.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|name| name == b"XRef")
        )), "final output should use a cross-reference stream");
    }

    fn blank_rich_text(text: &str) -> crate::RichTextBox {
        crate::RichTextBox {
            text_area_type: None,
            bbox: crate::BoundingBox::default(),
            rotation_degrees: None,
            text: text.to_owned(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    #[test]
    fn blank_body_admission_is_limited_to_native_confirmed_empty_and_lf_controls() {
        assert!(rich_text_is_benign_blank(&blank_rich_text("")));
        assert!(rich_text_is_benign_blank(&blank_rich_text("\n\n")));

        for text in [" ", "\t", "\r", "\r\n", "\u{00a0}", "\u{fffc}"] {
            assert!(
                !rich_text_is_benign_blank(&blank_rich_text(text)),
                "unexpectedly admitted {text:?}"
            );
        }
    }

    #[test]
    fn blank_body_admission_allows_only_nonpainting_known_attributes() {
        let mut text = blank_rich_text("\n");
        text.spans = vec![
            crate::RichTextSpan {
                kind: crate::RichTextSpanType::FontSize,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: crate::SpanIntervalType::ClosedOpen,
                payload: 12.0_f32.to_le_bytes().to_vec(),
            },
            crate::RichTextSpan {
                kind: crate::RichTextSpanType::ForegroundColor,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: crate::SpanIntervalType::ClosedOpen,
                payload: [0, 0, 0, 0xff].to_vec(),
            },
        ];
        text.paragraphs = vec![
            crate::RichTextParagraph {
                kind: crate::RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: 0_u32.to_le_bytes().to_vec(),
            },
            crate::RichTextParagraph {
                kind: crate::RichTextParagraphType::ParsingState,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: 1_u32.to_le_bytes().to_vec(),
            },
        ];
        text.color = Some(crate::Color {
            r: 37,
            g: 37,
            b: 37,
        });
        text.font_size = Some(17.0);
        text.margins = Some([16.0, 10.0, 16.0, 10.0]);
        text.gravity = Some(0);
        text.text_sections = vec![crate::RichTextSection {
            start_utf16: 0,
            length_utf16: 1,
        }];
        assert!(rich_text_is_benign_blank(&text));

        let mut bullet = text.clone();
        bullet.paragraphs.push(crate::RichTextParagraph {
            kind: crate::RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 1_u32.to_le_bytes().to_vec(),
        });
        assert!(!rich_text_is_benign_blank(&bullet));

        text.spans.push(crate::RichTextSpan {
            kind: crate::RichTextSpanType::Underline,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload: vec![1, 0],
        });
        assert!(!rich_text_is_benign_blank(&text));
    }

    #[test]
    fn blank_body_admission_rejects_unknown_geometry_or_malformed_known_attributes() {
        let mut text = blank_rich_text("");
        text.gravity = Some(1);
        assert!(!rich_text_is_benign_blank(&text));

        text.gravity = None;
        text.bbox.x_max = 1.0;
        assert!(!rich_text_is_benign_blank(&text));

        text = blank_rich_text("\n");
        text.text_sections = vec![crate::RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        }];
        assert!(!rich_text_is_benign_blank(&text));
        text.text_sections.clear();
        text.margins = Some([f32::NAN, 0.0, 0.0, 0.0]);
        assert!(!rich_text_is_benign_blank(&text));
        text.margins = None;
        text.spans.push(crate::RichTextSpan {
            kind: crate::RichTextSpanType::FontSize,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload: f32::NAN.to_le_bytes().to_vec(),
        });
        assert!(!rich_text_is_benign_blank(&text));
    }

    fn source_pdf() -> Vec<u8> {
        let mut pdf = Document::with_version("1.7");
        let pages_id = pdf.new_object_id();
        let ocg_id = pdf.add_object(dictionary! {
            "Type" => "OCG", "Name" => "Hidden source layer",
        });
        let struct_id = pdf.add_object(dictionary! { "Type" => "StructTreeRoot", "K" => vec![] });
        let font_id = pdf.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let appearance_id = pdf.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "FormType" => 1,
                "BBox" => vec![0.into(), 0.into(), 20.into(), 12.into()],
                "Resources" => dictionary! {},
            },
            b"q 0 1 0 rg 0 0 20 12 re f Q".to_vec(),
        ));
        let stamp_id = pdf.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Stamp", "Rect" => vec![4.into(), 4.into(), 24.into(), 16.into()],
            "F" => 4, "AP" => dictionary! { "N" => appearance_id },
        });
        let shared_resources = dictionary! {
            "Font" => dictionary! { "F1" => font_id },
            "Properties" => dictionary! { "Layer" => ocg_id },
        };

        let first_content = pdf.add_object(Stream::new(
            Dictionary::new(),
            b"/OC /Layer BDC 0 0 1 rg 10 10 25 25 re f EMC\nBT /F1 10 Tf 10 60 Td (source one) Tj ET\n".to_vec(),
        ));
        let first_id = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => first_content,
            "CropBox" => vec![10.into(), 12.into(), 190.into(), 148.into()],
            "Rotate" => 90, "UserUnit" => 2,
            "Group" => dictionary! { "S" => "Transparency", "CS" => "DeviceCMYK", "I" => false, "K" => true },
            "Annots" => vec![stamp_id.into()], "StructParents" => 21,
        });

        let mut second_a = Stream::new(
            Dictionary::new(),
            b"0 0 1 rg 18 22 80 70 re f\nq 0 0 3 3 re W n 1 0 0 1 400 400 cm\n".to_vec(),
        );
        second_a.compress().unwrap();
        let second_a_id = pdf.add_object(second_a);
        let mut second_b = Stream::new(Dictionary::new(), b"q\n".to_vec());
        second_b.compress().unwrap();
        let second_b_id = pdf.add_object(second_b);
        let second_id = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![(-20).into(), (-10).into(), 180.into(), 140.into()],
            "CropBox" => vec![0.into(), 5.into(), 150.into(), 120.into()],
            "Rotate" => 90, "UserUnit" => 2,
            "Contents" => vec![second_a_id.into(), second_b_id.into()],
            "Annots" => vec![stamp_id.into()], "StructParents" => 22,
            "Group" => dictionary! { "S" => "Transparency", "CS" => "DeviceRGB", "I" => true },
        });
        let third_content = pdf.add_object(Stream::new(
            Dictionary::new(),
            b"0 0.5 0 rg 40 50 70 80 re f\nBT /F1 10 Tf 50 150 Td (source three) Tj ET\n".to_vec(),
        ));
        let crop_id = pdf.add_object(Object::Array(vec![
            10.into(),
            20.into(),
            210.into(),
            300.into(),
        ]));
        let third_id = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 240.into(), 320.into()],
            "CropBox" => crop_id, "Rotate" => 270, "UserUnit" => 1,
            "Contents" => third_content, "StructParents" => 23,
        });
        pdf.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![first_id.into(), second_id.into(), third_id.into()],
            "Count" => 3, "MediaBox" => vec![0.into(), 0.into(), 240.into(), 320.into()],
            "Resources" => shared_resources,
        }));
        let catalog_id = pdf.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
            "OCProperties" => dictionary! {
                "OCGs" => vec![ocg_id.into()],
                "D" => dictionary! { "OFF" => vec![ocg_id.into()], "Order" => vec![ocg_id.into()] },
            },
            "StructTreeRoot" => struct_id, "MarkInfo" => dictionary! { "Marked" => true },
        });
        pdf.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    fn sdk_overlay_pdf() -> Vec<u8> {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="115">
          <defs>
            <radialGradient id="shade"><stop offset="0" stop-color="#e02030" stop-opacity=".2"/><stop offset="1" stop-color="#2040e0" stop-opacity=".9"/></radialGradient>
            <mask id="fade"><circle cx="75" cy="55" r="48" fill="url(#shade)"/></mask>
          </defs>
          <rect x="10" y="10" width="130" height="90" fill="#eeeeee"/>
          <g mask="url(#fade)" opacity=".8"><circle cx="75" cy="55" r="48" fill="#40b070"/></g>
          <text x="8" y="110" font-family="sans-serif" font-size="10">SDK vector overlay</text>
        </svg>"##;
        let page = RenderedPage::new(0, 150, 115, svg);
        let options = PdfOptions::new(Arc::new(crate::pdf::fontdb::Database::new()));
        super::super::render_svg_pages_pdf(&[page], &options).unwrap()
    }

    #[test]
    fn page_boxes_accept_opposite_corners_in_either_order() {
        let mut doc = Document::load_mem(&source_pdf()).unwrap();
        let page_id = doc.get_pages()[&1];
        let parent_id = doc
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        let parent = doc.get_dictionary_mut(parent_id).unwrap();
        parent.set("MediaBox", vec![200.into(), 100.into(), 0.into(), 0.into()]);
        parent.set("CropBox", vec![180.into(), 10.into(), 10.into(), 90.into()]);
        let page = doc.get_dictionary_mut(page_id).unwrap();
        page.remove(b"MediaBox");
        page.remove(b"CropBox");
        page.set("Rotate", 90);
        page.set("UserUnit", 1);
        let page = doc.get_dictionary(page_id).unwrap();
        let (matrix, crop) = page_to_display(&doc, page).unwrap();
        assert_eq!(crop, [10.0, 10.0, 180.0, 90.0]);
        assert_eq!(matrix, [0.0, -1.0, 1.0, 0.0, -10.0, 180.0]);
    }

    #[test]
    fn native_singlex_physical_surface_height_uses_checked_integer_truncation() {
        assert_eq!(native_surface_height(600, 1527, 1812).unwrap(), 505);
        assert!(native_surface_height(0, 1527, 1812).is_err());
        assert!(native_surface_height(600, 1527, 0).is_err());
        assert!(native_surface_height(i32::MAX, i32::MAX, 1).is_err());
    }

    #[test]
    fn continuation_surface_clips_full_crossing_path_at_physical_bottom() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1812" height="15372" viewBox="0 0 1812 15372">
          <path d="M 100 3007 L 100 3025" fill="none" stroke="#111111" stroke-width="2"/>
        </svg>"##;
        let overlay = RenderedPage::new(0, 1812, 15372, svg);
        let options = PdfOptions::new(Arc::new(crate::pdf::fontdb::Database::new()));
        let full_note_pdf = super::super::render_svg_pages_pdf(&[overlay], &options).unwrap();
        let original = Document::load_mem(&full_note_pdf).unwrap();
        let original_page = original.get_dictionary(original.get_pages()[&1]).unwrap();
        let (orientation, crop) = page_to_display(&original, original_page).unwrap();
        assert_eq!(orientation, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(crop, [0.0, 0.0, 1359.0, 11529.0]);
        assert_eq!(page_rotation(&original, original_page).unwrap(), 0);
        assert_eq!(page_user_unit(&original, original_page).unwrap(), 1.0);
        let original_content = contents_bytes(&original, original_page).unwrap();

        let wrapped = constrain_overlay_page(
            &full_note_pdf,
            1,
            1485,
            1812,
            15372,
            [600, 505],
            SDK_OVERLAY_PDF_UNITS_PER_NOTE_UNIT_AT_96_DPI,
        )
        .unwrap();
        let constrained = Document::load_mem(&wrapped).unwrap();
        let page_id = constrained.get_pages()[&1];
        let page = constrained.get_dictionary(page_id).unwrap();
        let (orientation, physical_box) = page_to_display(&constrained, page).unwrap();
        assert_eq!(orientation, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(physical_box, [0.0, 0.0, 600.0, 505.0]);
        assert_eq!(page_rotation(&constrained, page).unwrap(), 0);
        assert_eq!(page_user_unit(&constrained, page).unwrap(), 1.0);
        let wrapped_content = contents_bytes(&constrained, page).unwrap();
        assert!(
            wrapped_content
                .windows(original_content.len())
                .any(|window| window == original_content)
        );

        let scale = 600.0 / 1812.0;
        let query_scaled_height = scale * 1527.0;
        assert!(query_scaled_height > 505.0);
        let path_start = 505.0 - scale * (3007.0 - 1485.0);
        let path_end = 505.0 - scale * (3025.0 - 1485.0);
        assert!(path_start > 0.0 && path_end < 0.0);
        assert!(wrapped_content.starts_with(b"q 0 0 600 505 re W n"));
    }

    #[test]
    fn imported_overlay_mapping_composes_crop_rotation_and_user_unit() {
        let source = Document::load_mem(&source_pdf()).unwrap();
        let page = source.get_dictionary(source.get_pages()[&1]).unwrap();
        let (source_to_display, crop) = page_to_display(&source, page).unwrap();
        assert_eq!(crop, [10.0, 12.0, 190.0, 148.0]);
        assert_eq!(page_rotation(&source, page).unwrap(), 90);
        assert_eq!(page_user_unit(&source, page).unwrap(), 2.0);
        assert_eq!(source_to_display, [0.0, -2.0, 2.0, 0.0, -24.0, 380.0]);

        // This checks generic CropBox/Rotate/UserUnit affine composition only. Native
        // SingleX `GetPageSize` was observed to ignore UserUnit, so production must reject
        // or separately gate UserUnit != 1 until native source-page placement is pinned.
        let source_surface = [272.0, 360.0];
        let note_size = [1812.0, 15372.0];
        let (overlay_to_source, clip) = imported_overlay_placement(
            source_to_display,
            crop,
            source_surface,
            note_size,
            1485.0,
            SDK_OVERLAY_PDF_UNITS_PER_NOTE_UNIT_AT_96_DPI,
        )
        .unwrap();
        assert_eq!(clip, crop);

        let scale = source_surface[0] / note_size[0];
        let expected_overlay_to_display = [
            scale / SDK_OVERLAY_PDF_UNITS_PER_NOTE_UNIT_AT_96_DPI,
            0.0,
            0.0,
            scale / SDK_OVERLAY_PDF_UNITS_PER_NOTE_UNIT_AT_96_DPI,
            0.0,
            source_surface[1] - scale * (note_size[1] - 1485.0),
        ];
        for point in [[0.0, 0.0], [13.25, 41.5], [1359.0, 11529.0]] {
            let source_user = transform_point(overlay_to_source, point);
            let displayed = transform_point(source_to_display, source_user);
            let expected = transform_point(expected_overlay_to_display, point);
            assert!(
                displayed
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| (actual - expected).abs() < 1e-9)
            );
        }
        let crop_corners = [
            [crop[0], crop[1]],
            [crop[0], crop[3]],
            [crop[2], crop[1]],
            [crop[2], crop[3]],
        ]
        .map(|point| transform_point(source_to_display, point));
        assert_eq!(
            crop_corners,
            [[0.0, 360.0], [272.0, 360.0], [0.0, 0.0], [272.0, 0.0]]
        );
    }

    fn sdk_retained_text_overlay_pdf() -> Vec<u8> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/native_body_table.sdocx"
        );
        let document = crate::parse(path).unwrap();
        let layout = crate::layout_document(&document);
        let mut database = crate::fonts::fontdb::Database::new();
        database.load_font_data(include_bytes!("../../assets/fonts/Roboto-Regular.ttf").to_vec());
        database.set_sans_serif_family("Roboto");
        let fonts = crate::fonts::FontBook::new(Arc::new(database));
        crate::pdf::render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[0],
            &Default::default(),
            &Default::default(),
            &fonts,
        )
        .unwrap()
        .bytes
    }

    fn nested_indirect_mcr_overlay(bytes: &[u8]) -> Vec<u8> {
        let mut doc = Document::load_mem(bytes).unwrap();
        let page_id = doc.get_pages()[&1];
        let catalog_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = doc.get_dictionary(catalog_id).unwrap().clone();
        let root_id = catalog
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        let root = doc.get_dictionary(root_id).unwrap().clone();
        let document_id = root.get(b"K").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        let document_elem = doc.get_dictionary(document_id).unwrap().clone();
        let span_id = document_elem.get(b"K").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        let mcr_id = doc.add_object(dictionary! {
            "Type" => "MCR", "Pg" => page_id, "MCID" => 0,
        });
        let child_id = doc.add_object(dictionary! {
            "Type" => "StructElem", "S" => "Span", "P" => span_id,
            "Pg" => page_id, "K" => mcr_id,
        });
        doc.get_object_mut(span_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("K", child_id);
        let parent_tree = root.get(b"ParentTree").unwrap().as_dict().unwrap();
        let nums = parent_tree.get(b"Nums").unwrap().as_array().unwrap();
        let parent_array_id = nums[1].as_reference().unwrap();
        doc.get_object_mut(parent_array_id)
            .unwrap()
            .as_array_mut()
            .unwrap()[0] = child_id.into();
        let mut output = Vec::new();
        doc.save_to(&mut output).unwrap();
        output
    }

    fn collect_mcrs(doc: &Document, object: &Object, output: &mut Vec<Dictionary>, depth: usize) {
        assert!(depth < 128);
        match object {
            Object::Reference(id) => {
                collect_mcrs(doc, doc.get_object(*id).unwrap(), output, depth + 1);
            }
            Object::Array(items) => {
                for item in items {
                    collect_mcrs(doc, item, output, depth + 1);
                }
            }
            Object::Dictionary(dict)
                if dict
                    .get(b"Type")
                    .and_then(Object::as_name)
                    .is_ok_and(|n| n == b"MCR") =>
            {
                output.push(dict.clone());
            }
            Object::Dictionary(dict) => {
                if let Ok(kids) = dict.get(b"K") {
                    collect_mcrs(doc, kids, output, depth + 1);
                }
            }
            _ => {}
        }
    }

    fn sdk_noop_overlay_pdf() -> Vec<u8> {
        let page = RenderedPage::new(
            0,
            150,
            115,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="150" height="115"/>"#,
        );
        let options = PdfOptions::new(Arc::new(crate::pdf::fontdb::Database::new()));
        super::super::render_svg_pages_pdf(&[page], &options).unwrap()
    }

    fn color_stroke(color: crate::Color, y: f64) -> crate::PageObject {
        crate::Stroke {
            rendering: None,
            bbox: crate::BoundingBox::default(),
            points: vec![crate::Point { x: 70.0, y }, crate::Point { x: 170.0, y }],
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: Some(color),
            pen_width: 18.0,
        }
        .into()
    }

    fn page_colors(bytes: &[u8], page_number: u32) -> Vec<[f32; 3]> {
        let pdf = Document::load_mem(bytes).unwrap();
        let page_id = pdf.get_pages()[&page_number];
        let content = pdf.get_page_content(page_id).unwrap();
        Content::decode(&content)
            .unwrap()
            .operations
            .into_iter()
            .filter_map(|operation| {
                if operation.operator != "rg" && operation.operator != "RG" {
                    return None;
                }
                let rgb = operation
                    .operands
                    .iter()
                    .map(|value| {
                        value
                            .as_float()
                            .or_else(|_| value.as_i64().map(|number| number as f32))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                (rgb.len() == 3).then(|| [rgb[0], rgb[1], rgb[2]])
            })
            .collect()
    }

    fn page_actual_texts(bytes: &[u8], page_number: u32) -> Vec<Vec<u8>> {
        let pdf = Document::load_mem(bytes).unwrap();
        let page_id = pdf.get_pages()[&page_number];
        let content = pdf.get_page_content(page_id).unwrap();
        Content::decode(&content)
            .unwrap()
            .operations
            .into_iter()
            .filter_map(|operation| {
                if operation.operator != "BDC" {
                    return None;
                }
                operation
                    .operands
                    .get(1)?
                    .as_dict()
                    .ok()?
                    .get(b"ActualText")
                    .ok()?
                    .as_str()
                    .ok()
                    .map(<[u8]>::to_vec)
            })
            .collect()
    }

    fn color_index(colors: &[[f32; 3]], expected: [f32; 3]) -> Option<usize> {
        colors.iter().position(|actual| {
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < 0.01)
        })
    }

    #[test]
    fn sliced_singlex_scenes_retain_validated_body_image_and_selected_stroke_order() {
        let page = crate::Page {
            uuid: "continuous-body".to_owned(),
            width: 1_812,
            height: 7_000,
            content_bbox: crate::BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![
                color_stroke(crate::Color { r: 0, g: 0, b: 255 }, 100.0),
                color_stroke(crate::Color { r: 0, g: 255, b: 0 }, 5_500.0),
                color_stroke(
                    crate::Color {
                        r: 255,
                        g: 255,
                        b: 0,
                    },
                    6_000.0,
                ),
            ],
        };
        let body = crate::RichTextBox {
            text_area_type: None,
            bbox: crate::BoundingBox {
                x_min: 10.0,
                y_min: 20.0,
                x_max: 95.0,
                y_max: 90.0,
            },
            rotation_degrees: None,
            text: format!("{}\u{fffc}tail", "\n".repeat(112)),
            color: Some(crate::Color { r: 0, g: 0, b: 0 }),
            highlight_color: None,
            underline: false,
            font_size: Some(12.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: vec![crate::RichTextObjectSpan {
                object_type: crate::ObjectType::Image,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::Image(Box::new(
                    crate::PlacedImage {
                        bbox: crate::BoundingBox {
                            x_min: 40.0,
                            y_min: 5_450.0,
                            x_max: 180.0,
                            y_max: 5_550.0,
                        },
                        rotation_degrees: None,
                        media_id: Some(7),
                        media_index: Some(0),
                        crop_rect: None,
                        original_bbox: None,
                        border_media_id: None,
                        original_media_id: None,
                        nine_patch: None,
                        fill_effect_transparency: 0.0,
                    },
                ))),
                text_index_utf16: 112,
                layout_option: crate::ObjectSpanLayoutOption::Block,
                layout_constraint: crate::ObjectSpanLayoutConstraint::Normal,
            }],
            text_sections: vec![crate::RichTextSection {
                start_utf16: 0,
                length_utf16: 36,
            }],
            margins: None,
            gravity: None,
        };
        let document = crate::Document {
            pages: vec![page],
            metadata: crate::DocumentMetadata {
                page_mode: Some(1),
                orientation: Some(0),
                page_dimensions: Some((1_812, 7_000)),
                default_page_dimensions: Some((1_080, 1_527)),
                flow_dimensions: Some((1_080, 7_000)),
                note_text: Some(body),
                media_assets: vec![crate::MediaAsset {
                    name: "0.svg".to_owned(),
                    archive_id: Some(0),
                    mime_type: "image/svg+xml".to_owned(),
                    data: br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>"##.to_vec(),
                }],
                ..crate::DocumentMetadata::default()
            },
        };
        let mut layout = crate::layout_document(&document);
        let layout_page = &mut layout.pages[0];
        let generated_inspection_index = layout_page.body_text_slice().unwrap().object_index;
        layout_page.page.objects.swap(generated_inspection_index, 1);
        layout_page.body_text.as_mut().unwrap().object_index = 1;
        assert!(layout_page.single_page_continuous_body(&document).is_some());
        let inspection = layout_page.body_text_slice().unwrap().object_index;
        assert_eq!(inspection, 1);
        let selected_objects = (0..layout_page.page.objects.len())
            .filter(|index| *index != inspection)
            .collect::<Vec<_>>();
        assert_eq!(selected_objects.len(), 3);

        let plan = SingleXPaperPlan {
            parsed_sources: std::collections::BTreeMap::new(),
            note_width: 1_812,
            note_height: 7_000,
            output_pages: [(0, 5_000, 505.0), (5_000, 2_000, 202.0)]
                .into_iter()
                .map(
                    |(query_y, scene_height, physical_height)| SingleXOutputPage {
                        query_y,
                        scene_height,
                        physical_width: 600.0,
                        physical_height,
                        imported_source: None,
                        selected_objects: selected_objects.clone(),
                    },
                )
                .collect(),
        };
        let fonts = crate::fonts::FontBook::default();
        let mut cache = crate::render::DocumentTextCache::default();
        let output = render_singlex_pdf_paper_plan(
            &document,
            &layout,
            plan,
            &crate::RenderOptions::default(),
            &PdfOptions::from_font_book(&fonts),
            &fonts,
            &mut cache,
            &mut |_| {},
        )
        .unwrap();

        let first_page = page_colors(&output.bytes, 1);
        let second_page = page_colors(&output.bytes, 2);
        let blue = [0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0];
        let yellow = [1.0, 1.0, 0.0];
        let red = [1.0, 0.0, 0.0];
        assert!(
            color_index(&first_page, blue).is_some(),
            "first selected stroke missing"
        );
        assert!(
            color_index(&first_page, red).is_none(),
            "later-slice body image leaked upward"
        );
        assert!(color_index(&first_page, green).is_none());
        assert!(
            color_index(&second_page, red).is_some(),
            "resolved body image missing from second slice"
        );
        let green_index = color_index(&second_page, green).expect("second selected stroke missing");
        let yellow_index =
            color_index(&second_page, yellow).expect("third selected stroke missing");
        assert!(
            green_index < yellow_index,
            "selected strokes changed source order"
        );
        assert!(
            color_index(&second_page, blue).is_none(),
            "first-slice stroke leaked downward"
        );
        assert!(
            page_actual_texts(&output.bytes, 2)
                .iter()
                .any(|source| source == b"tail"),
            "full-source body text beyond the inspection slice was omitted"
        );
        assert_eq!(output.pages.len(), 2);
    }

    fn source_without_resources() -> Vec<u8> {
        let mut pdf = Document::with_version("1.7");
        let pages_id = pdf.new_object_id();
        let content_id = pdf.add_object(Stream::new(
            Dictionary::new(),
            b"0.2 0.4 0.8 rg 12 14 40 30 re f\n".to_vec(),
        ));
        let page_id = pdf.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 80.into()],
            "Contents" => content_id,
        });
        pdf.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
            }),
        );
        let catalog_id = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        pdf.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    fn source_with_catalog_destinations() -> Vec<u8> {
        let mut pdf = Document::load_mem(&source_pdf()).unwrap();
        let pages = pdf.get_pages();
        let first = pages[&1];
        let second = pages[&2];
        let first_leaf = pdf.add_object(dictionary! {
            "Names" => vec![
                Object::string_literal("first"),
                dictionary! {
                    "D" => vec![first.into(), Object::Name(b"Fit".to_vec())],
                }
                .into(),
            ],
            "Limits" => vec![Object::string_literal("first"), Object::string_literal("first")],
        });
        let second_leaf = pdf.add_object(dictionary! {
            "Names" => vec![
                Object::string_literal("second"),
                dictionary! {
                    "D" => vec![second.into(), Object::Name(b"Fit".to_vec())],
                }
                .into(),
            ],
            "Limits" => vec![Object::string_literal("second"), Object::string_literal("second")],
        });
        let dest_tree = pdf.add_object(dictionary! {
            "Kids" => vec![first_leaf.into(), second_leaf.into()],
            "Limits" => vec![Object::string_literal("first"), Object::string_literal("second")],
        });
        let open_action = pdf.add_object(dictionary! {
            "Type" => "Action", "S" => "GoTo",
            "D" => vec![second.into(), Object::Name(b"Fit".to_vec())],
        });
        let catalog_id = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = pdf
            .get_object_mut(catalog_id)
            .unwrap()
            .as_dict_mut()
            .unwrap();
        catalog.set("Names", dictionary! { "Dests" => dest_tree });
        catalog.set("OpenAction", open_action);
        catalog.set("PageMode", "UseOutlines");
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    fn imported_spec(source_page_index: i32, overlay_bytes: &[u8]) -> OutputPage<'_> {
        OutputPage::Imported {
            source_key: "paper",
            source_page_index,
            overlay_bytes,
            overlay_page_number: 1,
            display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            clip: [0.0, 0.0, 100.0, 80.0],
        }
    }

    fn catalog_named_destination(doc: &Document, name: &[u8]) -> ObjectId {
        fn find(doc: &Document, node: &Object, name: &[u8]) -> Option<ObjectId> {
            let dict = obj_dict(doc, node).ok()?;
            if let Ok(items) = dict.get(b"Names") {
                let items = deref(doc, items).ok()?.as_array().ok()?;
                for pair in items.as_chunks::<2>().0 {
                    if deref(doc, &pair[0]).ok()?.as_str().ok()? == name {
                        let value = deref(doc, &pair[1]).ok()?;
                        let destination = match value {
                            Object::Array(items) => items,
                            Object::Dictionary(dict) => {
                                deref(doc, dict.get(b"D").ok()?).ok()?.as_array().ok()?
                            }
                            _ => return None,
                        };
                        return destination.first()?.as_reference().ok();
                    }
                }
            }
            if let Ok(kids) = dict.get(b"Kids") {
                for child in deref(doc, kids).ok()?.as_array().ok()? {
                    if let Some(page) = find(doc, child, name) {
                        return Some(page);
                    }
                }
            }
            None
        }
        let catalog_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = doc.get_dictionary(catalog_id).unwrap();
        let names = catalog.get(b"Names").unwrap();
        let names = obj_dict(doc, names).unwrap();
        find(doc, names.get(b"Dests").unwrap(), name).expect("named destination")
    }

    fn archive_metadata(
        resources: &[(&str, Option<u32>, &[u8])],
        manifest: Option<&[(u32, &str)]>,
    ) -> DocumentMetadata {
        DocumentMetadata {
            archive_resources: resources
                .iter()
                .map(|(name, archive_id, data)| ArchiveResource {
                    name: (*name).to_owned(),
                    archive_id: *archive_id,
                    content: ArchiveResourceContent::Opaque {
                        data: data.to_vec(),
                    },
                })
                .collect(),
            media_manifest: manifest.map(|entries| MediaManifest {
                format_version: 5500,
                entries: entries
                    .iter()
                    .map(|(bind_id, file_name)| MediaManifestEntry {
                        bind_id: *bind_id,
                        file_name: (*file_name).to_owned(),
                        sha256: None,
                        reference_count: 1,
                        modified_time_raw: 0,
                        is_attached: false,
                        trailing_data: Vec::new(),
                    })
                    .collect(),
                trailing_data: Vec::new(),
            }),
            ..DocumentMetadata::default()
        }
    }

    fn pdf_paper_record(media_id: i32, page_index: i32) -> PdfPaperRecord {
        PdfPaperRecord {
            media_id,
            page_index,
            rectangle: PdfPaperRectangle::Integer([0, 0, 1, 1]),
        }
    }

    #[test]
    fn paper_records_resolve_manifest_identity_and_cache_sources_in_record_order() {
        let bytes = source_pdf();
        let metadata = archive_metadata(
            &[("media/7@paper.pdf", Some(7), &bytes)],
            Some(&[(42, "7@paper.pdf")]),
        );
        let prepared = prepare_pdf_paper_records(
            &metadata,
            &[
                pdf_paper_record(42, 2),
                pdf_paper_record(42, 0),
                pdf_paper_record(42, 2),
            ],
        )
        .unwrap();
        assert_eq!(prepared.sources.len(), 1);
        assert!(prepared.sources.contains_key("media/7@paper.pdf"));
        assert_eq!(
            prepared
                .records
                .iter()
                .map(|record| (record.source_page_index, record.inferred))
                .collect::<Vec<_>>(),
            [(2, false), (0, false), (2, false)]
        );
        let overlay = sdk_noop_overlay_pdf();
        let first = &prepared.records[0];
        let output = assemble_prepared_output_pages(
            &prepared.sources,
            &[OutputPage::Imported {
                source_key: &first.source_key,
                source_page_index: first.source_page_index,
                overlay_bytes: &overlay,
                overlay_page_number: 1,
                display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                clip: [0.0, 0.0, 100.0, 80.0],
            }],
        )
        .unwrap();
        assert_eq!(Document::load_mem(&output).unwrap().get_pages().len(), 1);

        // Without a media manifest, the legacy numeric filename binding remains
        // usable and is explicitly marked as inferred.
        let legacy = archive_metadata(&[("media/7@paper.pdf", Some(7), &bytes)], None);
        let prepared = prepare_pdf_paper_records(&legacy, &[pdf_paper_record(7, 1)]).unwrap();
        assert_eq!(prepared.records[0].source_key, "media/7@paper.pdf");
        assert!(prepared.records[0].inferred);
    }

    #[test]
    fn paper_record_binding_errors_keep_manifest_and_legacy_ambiguity_distinct() {
        let bytes = source_pdf();
        let duplicate_id = archive_metadata(
            &[("media/a.pdf", None, &bytes), ("media/b.pdf", None, &bytes)],
            Some(&[(42, "a.pdf"), (42, "b.pdf")]),
        );
        assert!(matches!(
            prepare_pdf_paper_records(&duplicate_id, &[pdf_paper_record(42, 0)]),
            Err(PdfPaperAdmissionError::Binding {
                record_index: 0,
                media_id: 42,
                source: ArchiveResourceError::AmbiguousBinding { id: 42 },
            })
        ));

        let missing = archive_metadata(&[], Some(&[(43, "missing.pdf")]));
        assert!(matches!(
            prepare_pdf_paper_records(&missing, &[pdf_paper_record(43, 0)]),
            Err(PdfPaperAdmissionError::Binding {
                source: ArchiveResourceError::MissingEntry { id: 43, .. },
                ..
            })
        ));

        let unbound = archive_metadata(
            &[("media/7@paper.pdf", Some(7), &bytes)],
            Some(&[(42, "7@paper.pdf")]),
        );
        assert!(matches!(
            prepare_pdf_paper_records(&unbound, &[pdf_paper_record(99, 0)]),
            Err(PdfPaperAdmissionError::Binding {
                source: ArchiveResourceError::UnboundId { id: 99 },
                ..
            })
        ));

        let duplicate_path = archive_metadata(
            &[
                ("media/7@paper.pdf", Some(7), &bytes),
                ("media/7@paper.pdf", Some(7), &bytes),
            ],
            None,
        );
        assert!(matches!(
            prepare_pdf_paper_records(&duplicate_path, &[pdf_paper_record(7, 0)]),
            Err(PdfPaperAdmissionError::Binding {
                source: ArchiveResourceError::AmbiguousEntry { .. },
                ..
            })
        ));

        let legacy_duplicate = archive_metadata(
            &[
                ("media/5@a.pdf", Some(5), &bytes),
                ("media/5@b.pdf", Some(5), &bytes),
            ],
            None,
        );
        assert!(matches!(
            prepare_pdf_paper_records(&legacy_duplicate, &[pdf_paper_record(5, 0)]),
            Err(PdfPaperAdmissionError::Binding {
                source: ArchiveResourceError::AmbiguousBinding { id: 5 },
                ..
            })
        ));
    }

    #[test]
    fn paper_record_admission_rejects_external_negative_malformed_and_out_of_range_sources() {
        let bytes = source_pdf();
        let metadata = archive_metadata(
            &[("media/7@paper.pdf", Some(7), &bytes)],
            Some(&[(42, "7@paper.pdf")]),
        );
        assert!(matches!(
            prepare_pdf_paper_records(&metadata, &[pdf_paper_record(-1000, 0)]),
            Err(PdfPaperAdmissionError::ExternalPdfUnavailable { record_index: 0 })
        ));
        assert!(matches!(
            prepare_pdf_paper_records(&metadata, &[pdf_paper_record(-1, 0)]),
            Err(PdfPaperAdmissionError::NegativeBinding {
                record_index: 0,
                media_id: -1,
            })
        ));
        assert!(matches!(
            prepare_pdf_paper_records(&metadata, &[pdf_paper_record(42, -1)]),
            Err(PdfPaperAdmissionError::NegativePageIndex {
                record_index: 0,
                page_index: -1,
            })
        ));
        assert!(matches!(
            prepare_pdf_paper_records(&metadata, &[pdf_paper_record(42, 3)]),
            Err(PdfPaperAdmissionError::PageIndexOutOfRange {
                record_index: 0,
                page_index: 3,
                page_count: 3,
                ..
            })
        ));

        let malformed = archive_metadata(
            &[("media/7@paper.pdf", Some(7), b"not a PDF")],
            Some(&[(42, "7@paper.pdf")]),
        );
        assert!(matches!(
            prepare_pdf_paper_records(&malformed, &[pdf_paper_record(42, 0)]),
            Err(PdfPaperAdmissionError::InvalidPdf {
                record_index: 0,
                ..
            } | PdfPaperAdmissionError::EmptyPdf {
                record_index: 0,
                ..
            })
        ));
    }

    fn source_with_indirect_popup() -> Vec<u8> {
        let mut doc = Document::load_mem(&source_pdf()).unwrap();
        let page_id = doc.get_pages()[&2];
        let page = doc.get_dictionary(page_id).unwrap().clone();
        let annots = page.get(b"Annots").unwrap().as_array().unwrap().clone();
        let stamp_id = annots[0].as_reference().unwrap();
        let popup_id = doc.add_object(dictionary! {
            "Subtype" => "Popup", "Rect" => vec![24.into(), 4.into(), 44.into(), 24.into()],
            "Parent" => stamp_id,
        });
        let stamp = doc.get_object_mut(stamp_id).unwrap().as_dict_mut().unwrap();
        stamp.remove(b"Type");
        stamp.set("Popup", popup_id);
        let annots_id = doc.add_object(Object::Array(annots));
        doc.get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Annots", annots_id);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    fn as_dict(doc: &Document, object: &Object) -> Dictionary {
        match object {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            Object::Dictionary(dict) => dict.clone(),
            _ => panic!("expected dictionary/reference, got {object:?}"),
        }
    }

    #[test]
    fn graft_keeps_three_page_source_graph_and_scopes_real_sdk_vector_resources() {
        let source_bytes = source_pdf();
        let overlay_bytes = sdk_overlay_pdf();
        let source_before = Document::load_mem(&source_bytes).unwrap();
        let before_pages = source_before.get_pages();
        assert_eq!(before_pages.len(), 3);
        let before_page2 = source_before.get_dictionary(before_pages[&2]).unwrap();
        let before_sibling_contents = [1, 3].map(|number| {
            contents_bytes(
                &source_before,
                source_before.get_dictionary(before_pages[&number]).unwrap(),
            )
            .unwrap()
        });
        let original_stream_ids = before_page2
            .get(b"Contents")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|object| object.as_reference().unwrap())
            .collect::<Vec<_>>();
        let original_streams = original_stream_ids
            .iter()
            .map(|id| {
                source_before
                    .get_object(*id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .decompressed_content()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(original_streams[0].windows(2).any(|window| window == b"q "));

        let overlay_before = Document::load_mem(&overlay_bytes).unwrap();
        assert_eq!(overlay_before.get_pages().len(), 1);
        let overlay_page = overlay_before
            .get_dictionary(overlay_before.get_pages()[&1])
            .unwrap();
        let overlay_resources = as_dict(
            &overlay_before,
            inherited(&overlay_before, overlay_page, b"Resources").unwrap(),
        );
        let expected_resources = reachable_resource_kinds(&overlay_before, &overlay_resources);
        assert!(
            expected_resources.1 > 0,
            "SDK overlay should exercise vector shading"
        );
        assert!(
            expected_resources.2 > 0,
            "SDK overlay should exercise opacity/mask resources"
        );
        assert_eq!(
            expected_resources.3, 0,
            "SDK test overlay should not rasterize"
        );

        // Deliberately explicit placement: native object-to-source basis is not inferred here.
        let grafted = graft_overlay_page(
            &source_bytes,
            2,
            &overlay_bytes,
            1,
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 5.0, 150.0, 120.0],
        )
        .unwrap();
        if let Some(probe_dir) = std::env::var_os("SDOCX_PAPER_PROBE_DIR") {
            let probe_dir = std::path::PathBuf::from(probe_dir);
            std::fs::create_dir_all(&probe_dir).unwrap();
            std::fs::write(probe_dir.join("source.pdf"), &source_bytes).unwrap();
            std::fs::write(probe_dir.join("marked.pdf"), &grafted).unwrap();
            let zero = graft_overlay_page(
                &source_bytes,
                2,
                &sdk_noop_overlay_pdf(),
                1,
                [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                [0.0, 5.0, 150.0, 120.0],
            )
            .unwrap();
            std::fs::write(probe_dir.join("zero.pdf"), zero).unwrap();
        }
        let output = Document::load_mem(&grafted).unwrap();
        let pages = output.get_pages();
        assert_eq!(pages.len(), 3);

        // Untouched siblings still inherit resources and keep their page semantics.
        let page1 = output.get_dictionary(pages[&1]).unwrap();
        let page3 = output.get_dictionary(pages[&3]).unwrap();
        assert!(page1.get(b"Resources").is_err());
        assert!(page3.get(b"Resources").is_err());
        assert_eq!(page1.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
        assert_eq!(page3.get(b"Rotate").unwrap().as_i64().unwrap(), 270);
        assert_eq!(page1.get(b"StructParents").unwrap().as_i64().unwrap(), 21);
        assert_eq!(page3.get(b"StructParents").unwrap().as_i64().unwrap(), 23);
        assert_eq!(
            contents_bytes(&output, page1).unwrap(),
            before_sibling_contents[0],
            "page 1 source content must remain byte-for-byte after decode"
        );
        assert_eq!(
            contents_bytes(&output, page3).unwrap(),
            before_sibling_contents[1],
            "page 3 source content must remain byte-for-byte after decode"
        );

        let page2 = output.get_dictionary(pages[&2]).unwrap();
        assert_eq!(
            page2
                .get(b"CropBox")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>(),
            [0, 5, 150, 120]
        );
        assert_eq!(
            page2
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>(),
            [-20, -10, 180, 140]
        );
        assert_eq!(page2.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
        assert_eq!(page2.get(b"UserUnit").unwrap().as_i64().unwrap(), 2);
        assert_eq!(page2.get(b"StructParents").unwrap().as_i64().unwrap(), 22);
        let group = as_dict(&output, page2.get(b"Group").unwrap());
        assert_eq!(group.get(b"CS").unwrap().as_name().unwrap(), b"DeviceRGB");
        let annotations = page2.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annotations.len(), 1);
        let annotation = as_dict(&output, &annotations[0]);
        assert_eq!(
            annotation.get(b"Subtype").unwrap().as_name().unwrap(),
            b"Stamp"
        );
        let appearance = as_dict(&output, annotation.get(b"AP").unwrap());
        assert!(appearance.has(b"N"));

        let root = output.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = output.get_dictionary(root).unwrap();
        assert!(catalog.has(b"StructTreeRoot"));
        assert!(catalog.has(b"OCProperties"));
        let oc = as_dict(&output, catalog.get(b"OCProperties").unwrap());
        let default_config = as_dict(&output, oc.get(b"D").unwrap());
        assert_eq!(
            default_config
                .get(b"OFF")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );

        // The original streams remain direct page content references, in order and byte-for-byte decoded.
        let content_items = page2.get(b"Contents").unwrap().as_array().unwrap();
        assert_eq!(content_items.len(), 5); // isolation q, source A, source B, balancing Qs, overlay command
        for (item, (id, expected)) in content_items[1..3]
            .iter()
            .zip(original_stream_ids.iter().zip(&original_streams))
        {
            let actual_id = item.as_reference().unwrap();
            assert_eq!(actual_id, *id);
            assert_eq!(
                output
                    .get_object(actual_id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .decompressed_content()
                    .unwrap(),
                *expected
            );
        }
        let tail = output
            .get_object(content_items[4].as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        let tail = tail.decompressed_content().unwrap();
        assert!(
            tail.windows(b"SDOCXOverlay".len())
                .any(|w| w == b"SDOCXOverlay")
        );
        assert!(tail.windows(2).any(|w| w == b"cm"));

        // Page /f0 (Helvetica) does not capture the SDK Form's own resource names.
        let page_resources = as_dict(&output, page2.get(b"Resources").unwrap());
        let page_fonts = as_dict(&output, page_resources.get(b"Font").unwrap());
        assert!(page_fonts.has(b"F1"));
        let page_properties = as_dict(&output, page_resources.get(b"Properties").unwrap());
        assert!(page_properties.has(b"Layer"));
        let page_xobjects = as_dict(&output, page_resources.get(b"XObject").unwrap());
        let form_ref = page_xobjects
            .iter()
            .find_map(|(name, value)| {
                if name.starts_with(b"SDOCXOverlay") {
                    value.as_reference().ok()
                } else {
                    None
                }
            })
            .expect("scoped SDK overlay Form");
        let form = output.get_object(form_ref).unwrap().as_stream().unwrap();
        assert_eq!(
            form.dict.get(b"Subtype").unwrap().as_name().unwrap(),
            b"Form"
        );
        let form_resources = as_dict(&output, form.dict.get(b"Resources").unwrap());
        let (fonts, shadings, soft_masks, images) =
            reachable_resource_kinds(&output, &form_resources);
        assert_eq!(
            (fonts, shadings, soft_masks, images),
            expected_resources,
            "grafting must retain every resource reachable from the actual SDK overlay Form"
        );
        assert!(shadings > 0, "SDK vector shading must remain reachable");
        assert!(
            soft_masks > 0,
            "SDK opacity/mask resources must remain reachable"
        );
        assert_eq!(images, 0, "test overlay should remain vector-only");
    }

    #[test]
    fn graft_accepts_null_page_contents() {
        for indirect in [false, true] {
            let mut source = Document::load_mem(&source_pdf()).unwrap();
            let page_id = source.get_pages()[&2];
            let contents = if indirect {
                Object::Reference(source.add_object(Object::Null))
            } else {
                Object::Null
            };
            source
                .get_dictionary_mut(page_id)
                .unwrap()
                .set("Contents", contents);
            let mut bytes = Vec::new();
            source.save_to(&mut bytes).unwrap();
            let output = graft_overlay_page(
                &bytes,
                2,
                &sdk_overlay_pdf(),
                1,
                [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 100.0, 100.0],
            )
            .unwrap();
            let output = Document::load_mem(&output).unwrap();
            assert!(
                !contents_bytes(
                    &output,
                    output.get_dictionary(output.get_pages()[&2]).unwrap()
                )
                .unwrap()
                .is_empty()
            );
        }
    }

    #[test]
    fn malformed_parent_cycle_and_unmatched_page_restore_are_rejected() {
        let overlay = sdk_overlay_pdf();
        let mut cyclic = Document::load_mem(&source_pdf()).unwrap();
        let page_id = cyclic.get_pages()[&2];
        let parent_id = cyclic
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Parent")
            .unwrap()
            .as_reference()
            .unwrap();
        cyclic
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"MediaBox");
        cyclic
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"CropBox");
        cyclic
            .get_object_mut(parent_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Parent", parent_id);
        let mut cyclic_bytes = Vec::new();
        cyclic.save_to(&mut cyclic_bytes).unwrap();
        assert!(
            graft_overlay_page(
                &cyclic_bytes,
                2,
                &overlay,
                1,
                [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 1.0]
            )
            .unwrap_err()
            .contains("cycle in page Parent chain")
        );

        let mut unbalanced = Document::load_mem(&source_pdf()).unwrap();
        let page_id = unbalanced.get_pages()[&2];
        let content_id = unbalanced.add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));
        unbalanced
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Contents", content_id);
        let mut unbalanced_bytes = Vec::new();
        unbalanced.save_to(&mut unbalanced_bytes).unwrap();
        assert!(
            graft_overlay_page(
                &unbalanced_bytes,
                2,
                &overlay,
                1,
                [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 1.0]
            )
            .unwrap_err()
            .contains("unmatched Q")
        );
    }

    #[test]
    fn assembler_keeps_only_selected_ordered_duplicates_and_page_graph() {
        let source = source_with_indirect_popup();
        let overlay = sdk_noop_overlay_pdf();
        let output = assemble_output_pages(
            &[SourcePdf {
                key: "paper-a",
                bytes: &source,
            }],
            &[
                OutputPage::Imported {
                    source_key: "paper-a",
                    source_page_index: 1,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 5.0, 150.0, 120.0],
                },
                OutputPage::Imported {
                    source_key: "paper-a",
                    source_page_index: 1,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 5.0, 150.0, 120.0],
                },
            ],
        )
        .unwrap();
        let before = Document::load_mem(&source).unwrap();
        let before_page_id = before.get_pages()[&2];
        let before_page = before.get_dictionary(before_page_id).unwrap();
        let before_content = contents_bytes(&before, before_page).unwrap();
        let before_streams = content_items(&before, before_page.get(b"Contents").unwrap())
            .unwrap()
            .into_iter()
            .map(|item| {
                before
                    .get_object(item.as_reference().unwrap())
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .content
                    .clone()
            })
            .collect::<Vec<_>>();
        let output = Document::load_mem(&output).unwrap();
        assert_modern_pdf_serialization(&output);
        let pages = output.get_pages();
        assert_eq!(
            pages.len(),
            2,
            "unselected source pages must not remain in the output tree"
        );
        let root = output.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = output.get_dictionary(root).unwrap();
        let page_root = catalog.get(b"Pages").unwrap().as_reference().unwrap();
        let expected_ids = pages.values().copied().collect::<Vec<_>>();
        for (idx, page_id) in expected_ids.iter().copied().enumerate() {
            let page = output.get_dictionary(page_id).unwrap();
            assert_eq!(
                page.get(b"Parent").unwrap().as_reference().unwrap(),
                page_root
            );
            assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), 90);
            assert_eq!(page.get(b"UserUnit").unwrap().as_i64().unwrap(), 2);
            assert_eq!(page.get(b"StructParents").unwrap().as_i64().unwrap(), 22);
            let contents = page.get(b"Contents").unwrap().as_array().unwrap();
            for (idx, expected) in before_streams.iter().enumerate() {
                let stream = output
                    .get_object(contents[idx + 1].as_reference().unwrap())
                    .unwrap()
                    .as_stream()
                    .unwrap();
                assert_eq!(
                    &stream.content, expected,
                    "source content bytes must be retained"
                );
            }
            let annots = deref(&output, page.get(b"Annots").unwrap())
                .unwrap()
                .as_array()
                .unwrap();
            let annot = as_dict(&output, &annots[0]);
            assert_eq!(annot.get(b"P").unwrap().as_reference().unwrap(), page_id);
            let popup_id = annot.get(b"Popup").unwrap().as_reference().unwrap();
            let popup = output.get_dictionary(popup_id).unwrap();
            assert_eq!(
                popup.get(b"Parent").unwrap().as_reference().unwrap(),
                annots[0].as_reference().unwrap()
            );
            let ap = as_dict(&output, annot.get(b"AP").unwrap());
            assert!(ap.has(b"N"));
            if idx == 0 {
                let group = as_dict(&output, page.get(b"Group").unwrap());
                assert_eq!(group.get(b"CS").unwrap().as_name().unwrap(), b"DeviceRGB");
            }
        }
        let content0 =
            contents_bytes(&output, output.get_dictionary(expected_ids[0]).unwrap()).unwrap();
        assert!(
            content0
                .windows(before_content.len())
                .any(|window| window == before_content)
        );
        let content1 =
            contents_bytes(&output, output.get_dictionary(expected_ids[1]).unwrap()).unwrap();
        assert!(
            content1
                .windows(before_content.len())
                .any(|window| window == before_content)
        );
        let annots0 = deref(
            &output,
            output
                .get_dictionary(expected_ids[0])
                .unwrap()
                .get(b"Annots")
                .unwrap(),
        )
        .unwrap()
        .as_array()
        .unwrap();
        let annots1 = deref(
            &output,
            output
                .get_dictionary(expected_ids[1])
                .unwrap()
                .get(b"Annots")
                .unwrap(),
        )
        .unwrap()
        .as_array()
        .unwrap();
        assert_ne!(
            annots0[0].as_reference().unwrap(),
            annots1[0].as_reference().unwrap(),
            "duplicate output pages need distinct page-bound annotation graphs"
        );

        // Source catalog OCG identity and the page resource's Properties entry
        // must resolve to the same cloned OCG, not two visually divergent copies.
        let catalog_oc = as_dict(&output, catalog.get(b"OCProperties").unwrap());
        let ocg = catalog_oc.get(b"OCGs").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        let page = output.get_dictionary(expected_ids[0]).unwrap();
        let resources = as_dict(&output, page.get(b"Resources").unwrap());
        let fonts = as_dict(&output, resources.get(b"Font").unwrap());
        assert!(
            fonts.has(b"F1"),
            "inherited source font resource must remain reachable"
        );
        let properties = as_dict(&output, resources.get(b"Properties").unwrap());
        assert_eq!(
            properties.get(b"Layer").unwrap().as_reference().unwrap(),
            ocg
        );
    }

    #[test]
    fn assembler_remaps_single_source_catalog_destinations_for_reordered_duplicates() {
        let source = source_with_catalog_destinations();
        let overlay = sdk_noop_overlay_pdf();
        let output = assemble_output_pages(
            &[SourcePdf {
                key: "paper",
                bytes: &source,
            }],
            &[
                imported_spec(1, &overlay),
                imported_spec(0, &overlay),
                imported_spec(1, &overlay),
            ],
        )
        .unwrap();
        let output = Document::load_mem(&output).unwrap();
        let output_pages = output.get_pages();
        let output_page_ids = output_pages.values().copied().collect::<Vec<_>>();
        let catalog_id = output.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = output.get_dictionary(catalog_id).unwrap();

        let action = obj_dict(&output, catalog.get(b"OpenAction").unwrap()).unwrap();
        assert_eq!(action.get(b"S").unwrap().as_name().unwrap(), b"GoTo");
        let open_destination = deref(&output, action.get(b"D").unwrap())
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            open_destination[0].as_reference().unwrap(),
            output_page_ids[0],
            "OpenAction targets the first output occurrence of its source page"
        );
        assert_eq!(
            catalog.get(b"PageMode").unwrap().as_name().unwrap(),
            b"UseOutlines"
        );
        assert_eq!(
            catalog_named_destination(&output, b"first"),
            output_page_ids[1]
        );
        assert_eq!(
            catalog_named_destination(&output, b"second"),
            output_page_ids[0],
            "named destinations also target the first output occurrence"
        );
    }

    #[test]
    fn assembler_rejects_catalog_references_to_unselected_pages() {
        let source = source_with_catalog_destinations();
        let overlay = sdk_noop_overlay_pdf();
        let error = assemble_output_pages(
            &[SourcePdf {
                key: "paper",
                bytes: &source,
            }],
            &[imported_spec(0, &overlay)],
        )
        .unwrap_err();
        assert!(error.contains("unselected page"), "{error}");
    }

    #[test]
    fn catalog_admission_rejects_unsupported_name_and_open_action_contexts() {
        let source = source_with_catalog_destinations();
        let mut unsupported_names = Document::load_mem(&source).unwrap();
        let catalog_id = unsupported_names
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        unsupported_names
            .get_object_mut(catalog_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "Names",
                dictionary! { "Dests" => Dictionary::new(), "JavaScript" => Dictionary::new() },
            );
        assert!(
            ensure_catalog_context_supported(&unsupported_names)
                .unwrap_err()
                .contains("unsupported name-tree context")
        );

        let mut unsupported_action = Document::load_mem(&source).unwrap();
        let catalog_id = unsupported_action
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let action_id = unsupported_action.add_object(dictionary! {
            "S" => "URI", "URI" => Object::string_literal("https://example.invalid/"),
        });
        unsupported_action
            .get_object_mut(catalog_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("OpenAction", action_id);
        assert!(
            ensure_catalog_context_supported(&unsupported_action)
                .unwrap_err()
                .contains("unsupported action entries")
        );

        let mut named_action = Document::load_mem(&source).unwrap();
        let catalog_id = named_action
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let action_id = named_action.add_object(dictionary! {
            "S" => "GoTo", "D" => Object::string_literal("first"),
        });
        named_action
            .get_object_mut(catalog_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("OpenAction", action_id);
        ensure_catalog_context_supported(&named_action)
            .expect("named GoTo destinations are retained as name/string references");
    }

    #[test]
    fn assembler_combines_multiple_sources_and_overlay_only_tail_in_requested_order() {
        let source_a = source_without_resources();
        let mut source_b_doc = Document::load_mem(&source_without_resources()).unwrap();
        let source_b_page = source_b_doc.get_pages()[&1];
        let source_b_content = source_b_doc
            .get_dictionary(source_b_page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        source_b_doc
            .get_object_mut(source_b_content)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(b"1 0 0 rg 12 14 40 30 re f\n".to_vec());
        let mut source_b = Vec::new();
        source_b_doc.save_to(&mut source_b).unwrap();
        let overlay = sdk_noop_overlay_pdf();
        let output = assemble_output_pages(
            &[
                SourcePdf {
                    key: "a",
                    bytes: &source_a,
                },
                SourcePdf {
                    key: "b",
                    bytes: &source_b,
                },
            ],
            &[
                OutputPage::Imported {
                    source_key: "b",
                    source_page_index: 0,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 0.0, 100.0, 80.0],
                },
                OutputPage::Imported {
                    source_key: "a",
                    source_page_index: 0,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 0.0, 100.0, 80.0],
                },
                OutputPage::OverlayOnly {
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                },
            ],
        )
        .unwrap();
        let output = Document::load_mem(&output).unwrap();
        let pages = output.get_pages();
        assert_eq!(pages.len(), 3);
        let page_ids = pages.values().copied().collect::<Vec<_>>();
        let imported0 =
            contents_bytes(&output, output.get_dictionary(page_ids[0]).unwrap()).unwrap();
        let imported1 =
            contents_bytes(&output, output.get_dictionary(page_ids[1]).unwrap()).unwrap();
        assert!(
            imported0
                .windows(b"1 0 0 rg".len())
                .any(|window| window == b"1 0 0 rg")
        );
        assert!(
            imported1
                .windows(b"0.2 0.4".len())
                .any(|window| window == b"0.2 0.4")
        );
        let tail_page = output.get_dictionary(page_ids[2]).unwrap();
        assert!(tail_page.get(b"Parent").is_ok());
        assert!(tail_page.get(b"Contents").is_ok());
        assert!(tail_page.get(b"Resources").is_ok());
    }

    #[test]
    fn assembler_refuses_to_drop_unmergeable_catalog_context() {
        let source_with_ocg = source_pdf();
        let source_without_context = source_without_resources();
        let overlay = sdk_noop_overlay_pdf();
        let error = assemble_output_pages(
            &[
                SourcePdf {
                    key: "a",
                    bytes: &source_with_ocg,
                },
                SourcePdf {
                    key: "b",
                    bytes: &source_without_context,
                },
            ],
            &[
                OutputPage::Imported {
                    source_key: "a",
                    source_page_index: 1,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 5.0, 150.0, 120.0],
                },
                OutputPage::Imported {
                    source_key: "b",
                    source_page_index: 0,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 0.0, 100.0, 80.0],
                },
            ],
        )
        .unwrap_err();
        assert!(error.contains("global context"), "{error}");
    }

    #[test]
    fn actual_sdk_retained_text_tags_survive_page_and_form_grafts() {
        let overlay = sdk_retained_text_overlay_pdf();
        let source = source_without_resources();
        let output = assemble_output_pages(
            &[SourcePdf {
                key: "paper",
                bytes: &source,
            }],
            &[
                OutputPage::Imported {
                    source_key: "paper",
                    source_page_index: 0,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 0.0, 100.0, 80.0],
                },
                OutputPage::Imported {
                    source_key: "paper",
                    source_page_index: 0,
                    overlay_bytes: &overlay,
                    overlay_page_number: 1,
                    display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    clip: [0.0, 0.0, 100.0, 80.0],
                },
            ],
        )
        .unwrap();
        let output = Document::load_mem(&output).unwrap();
        let pages = output.get_pages();
        assert_eq!(pages.len(), 2);
        let catalog_id = output.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = output.get_dictionary(catalog_id).unwrap();
        let root_id = catalog
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        let root = output.get_dictionary(root_id).unwrap();
        let parent_tree = struct_parent_tree(&output).unwrap().unwrap();
        let mut form_ids = Vec::new();
        let mut structure_keys = Vec::new();
        let mut expected_mcids = Vec::new();
        for page_number in [1, 2] {
            let page_id = pages[&page_number];
            let page = output.get_dictionary(page_id).unwrap();
            let resources = as_dict(&output, page.get(b"Resources").unwrap());
            let xobjects = as_dict(&output, resources.get(b"XObject").unwrap());
            let form_id = xobjects
                .iter()
                .find_map(|(name, value)| {
                    if name.starts_with(b"SDOCXOverlay") {
                        value.as_reference().ok()
                    } else {
                        None
                    }
                })
                .expect("imported SDK overlay Form");
            let form = output.get_object(form_id).unwrap().as_stream().unwrap();
            assert_eq!(
                form.dict.get(b"Subtype").unwrap().as_name().unwrap(),
                b"Form"
            );
            let key = form.dict.get(b"StructParents").unwrap().as_i64().unwrap();
            structure_keys.push(key);
            form_ids.push(form_id);
            let parent_array = deref(&output, &parent_tree[&key])
                .unwrap()
                .as_array()
                .unwrap();
            assert_eq!(parent_array.len(), 6);
            assert!(parent_array.iter().all(|item| item.as_reference().is_ok()));

            let mut mcrs = Vec::new();
            collect_mcrs(&output, root.get(b"K").unwrap(), &mut mcrs, 0);
            let matching = mcrs
                .iter()
                .filter(|mcr| {
                    mcr.get(b"Stm")
                        .and_then(Object::as_reference)
                        .is_ok_and(|id| id == form_id)
                })
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 6);
            assert!(matching.iter().all(|mcr| {
                mcr.get(b"Pg")
                    .and_then(Object::as_reference)
                    .is_ok_and(|id| id == page_id)
            }));
            let mut ids = matching
                .iter()
                .map(|mcr| mcr.get(b"MCID").unwrap().as_i64().unwrap())
                .collect::<Vec<_>>();
            ids.sort_unstable();
            expected_mcids.push(ids);
        }
        assert_ne!(structure_keys[0], structure_keys[1]);
        assert_ne!(form_ids[0], form_ids[1]);
        assert_eq!(expected_mcids[0], (0..6).collect::<Vec<_>>());
        assert_eq!(expected_mcids[1], (0..6).collect::<Vec<_>>());

        // Overlay-only continuation pages keep page-scoped marked content and
        // its ParentTree binding rather than wrapping the page in a Form.
        let tail = assemble_output_pages(
            &[],
            &[OutputPage::OverlayOnly {
                overlay_bytes: &overlay,
                overlay_page_number: 1,
            }],
        )
        .unwrap();
        let tail = Document::load_mem(&tail).unwrap();
        assert_modern_pdf_serialization(&tail);
        let page_id = tail.get_pages()[&1];
        let page = tail.get_dictionary(page_id).unwrap();
        let key = page.get(b"StructParents").unwrap().as_i64().unwrap();
        let parent_tree = struct_parent_tree(&tail).unwrap().unwrap();
        let parent_array = deref(&tail, &parent_tree[&key])
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(parent_array.len(), 6);
        let mut mcrs = Vec::new();
        let root_id = tail
            .get_dictionary(tail.trailer.get(b"Root").unwrap().as_reference().unwrap())
            .unwrap()
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        collect_mcrs(
            &tail,
            tail.get_dictionary(root_id).unwrap().get(b"K").unwrap(),
            &mut mcrs,
            0,
        );
        assert!(mcrs.is_empty(), "overlay-only K items remain page MCIDs");
        let root = tail.get_dictionary(root_id).unwrap();
        fn collect_mcids(doc: &Document, object: &Object, out: &mut Vec<i64>, depth: usize) {
            assert!(depth < 128);
            match object {
                Object::Reference(id) => {
                    collect_mcids(doc, doc.get_object(*id).unwrap(), out, depth + 1)
                }
                Object::Array(items) => {
                    for item in items {
                        collect_mcids(doc, item, out, depth + 1);
                    }
                }
                Object::Dictionary(dict) => {
                    if let Ok(k) = dict.get(b"K") {
                        collect_mcids(doc, k, out, depth + 1);
                    }
                }
                Object::Integer(mcid) => out.push(*mcid),
                _ => {}
            }
        }
        let mut page_mcids = Vec::new();
        collect_mcids(&tail, root.get(b"K").unwrap(), &mut page_mcids, 0);
        page_mcids.sort_unstable();
        assert_eq!(page_mcids, (0..6).collect::<Vec<_>>());
    }

    #[test]
    fn tagged_overlay_remaps_nested_mcr_and_preserves_out_of_order_parent_slots() {
        let source = source_without_resources();
        let nested = nested_indirect_mcr_overlay(&sdk_retained_text_overlay_pdf());
        let nested_output = assemble_output_pages(
            &[SourcePdf {
                key: "paper",
                bytes: &source,
            }],
            &[OutputPage::Imported {
                source_key: "paper",
                source_page_index: 0,
                overlay_bytes: &nested,
                overlay_page_number: 1,
                display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                clip: [0.0, 0.0, 100.0, 80.0],
            }],
        )
        .unwrap();
        let nested_output = Document::load_mem(&nested_output).unwrap();
        let page_id = nested_output.get_pages()[&1];
        let page = nested_output.get_dictionary(page_id).unwrap();
        let resources = as_dict(&nested_output, page.get(b"Resources").unwrap());
        let xobjects = as_dict(&nested_output, resources.get(b"XObject").unwrap());
        let form_id = xobjects
            .iter()
            .find_map(|(name, value)| {
                if name.starts_with(b"SDOCXOverlay") {
                    value.as_reference().ok()
                } else {
                    None
                }
            })
            .unwrap();
        let mut mcrs = Vec::new();
        let root_id = nested_output
            .get_dictionary(
                nested_output
                    .trailer
                    .get(b"Root")
                    .unwrap()
                    .as_reference()
                    .unwrap(),
            )
            .unwrap()
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        collect_mcrs(
            &nested_output,
            nested_output
                .get_dictionary(root_id)
                .unwrap()
                .get(b"K")
                .unwrap(),
            &mut mcrs,
            0,
        );
        assert_eq!(mcrs.len(), 6);
        assert!(mcrs.iter().all(|mcr| {
            mcr.get(b"Stm")
                .and_then(Object::as_reference)
                .is_ok_and(|id| id == form_id)
                && mcr
                    .get(b"Pg")
                    .and_then(Object::as_reference)
                    .is_ok_and(|id| id == page_id)
        }));

        let mut out_of_order = Document::load_mem(&sdk_retained_text_overlay_pdf()).unwrap();
        let catalog_id = out_of_order
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let catalog = out_of_order.get_dictionary(catalog_id).unwrap().clone();
        let structure_root_id = catalog
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        let structure_root = out_of_order
            .get_dictionary(structure_root_id)
            .unwrap()
            .clone();
        let document_id = structure_root.get(b"K").unwrap().as_array().unwrap()[0]
            .as_reference()
            .unwrap();
        let document_elem = out_of_order.get_dictionary(document_id).unwrap().clone();
        let spans = document_elem.get(b"K").unwrap().as_array().unwrap().clone();
        assert_eq!(spans.len(), 6);
        let first_span = spans[0].as_reference().unwrap();
        let parent_tree = as_dict(&out_of_order, structure_root.get(b"ParentTree").unwrap());
        let nums = parent_tree.get(b"Nums").unwrap().as_array().unwrap();
        let parent_array_id = nums[1].as_reference().unwrap();
        out_of_order
            .get_object_mut(document_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("K", first_span);
        out_of_order
            .get_object_mut(first_span)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "K",
                vec![5.into(), 0.into(), 3.into(), 1.into(), 2.into(), 4.into()],
            );
        out_of_order
            .get_object_mut(parent_array_id)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .for_each(|parent| *parent = first_span.into());
        let mut out_of_order_bytes = Vec::new();
        out_of_order.save_to(&mut out_of_order_bytes).unwrap();

        let output = assemble_output_pages(
            &[SourcePdf {
                key: "paper",
                bytes: &source,
            }],
            &[OutputPage::Imported {
                source_key: "paper",
                source_page_index: 0,
                overlay_bytes: &out_of_order_bytes,
                overlay_page_number: 1,
                display_to_source: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                clip: [0.0, 0.0, 100.0, 80.0],
            }],
        )
        .unwrap();
        let output = Document::load_mem(&output).unwrap();
        let parent_tree = struct_parent_tree(&output).unwrap().unwrap();
        let page = output.get_dictionary(output.get_pages()[&1]).unwrap();
        let resources = as_dict(&output, page.get(b"Resources").unwrap());
        let xobjects = as_dict(&output, resources.get(b"XObject").unwrap());
        let form_id = xobjects
            .iter()
            .find_map(|(name, value)| {
                if name.starts_with(b"SDOCXOverlay") {
                    value.as_reference().ok()
                } else {
                    None
                }
            })
            .unwrap();
        let key = output
            .get_object(form_id)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"StructParents")
            .unwrap()
            .as_i64()
            .unwrap();
        let parents = deref(&output, &parent_tree[&key])
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(parents.len(), 6, "out-of-order MCIDs must not shrink slots");
        let parent_span = parents[0].as_reference().unwrap();
        assert!(
            parents
                .iter()
                .all(|parent| parent.as_reference().unwrap() == parent_span)
        );
        let root_id = output
            .get_dictionary(output.trailer.get(b"Root").unwrap().as_reference().unwrap())
            .unwrap()
            .get(b"StructTreeRoot")
            .unwrap()
            .as_reference()
            .unwrap();
        let mut mcrs = Vec::new();
        collect_mcrs(
            &output,
            output.get_dictionary(root_id).unwrap().get(b"K").unwrap(),
            &mut mcrs,
            0,
        );
        assert_eq!(
            mcrs.iter()
                .map(|mcr| mcr.get(b"MCID").unwrap().as_i64().unwrap())
                .collect::<Vec<_>>(),
            [5, 0, 3, 1, 2, 4]
        );
    }

    #[test]
    fn malformed_resources_fail_but_absent_resources_are_created_for_overlay() {
        let overlay = sdk_overlay_pdf();
        let mut malformed = Document::load_mem(&source_pdf()).unwrap();
        let page_id = malformed.get_pages()[&2];
        malformed
            .get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Resources", 17);
        let mut malformed_bytes = Vec::new();
        malformed.save_to(&mut malformed_bytes).unwrap();
        let error = graft_overlay_page(
            &malformed_bytes,
            2,
            &overlay,
            1,
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 100.0, 80.0],
        )
        .unwrap_err();
        assert!(error.contains("expected dictionary"), "{error}");

        // No Resources anywhere in the tree is valid; the overlay adds its own page resources.
        let without_resources = source_without_resources();
        let output = graft_overlay_page(
            &without_resources,
            1,
            &overlay,
            1,
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 100.0, 80.0],
        )
        .unwrap();
        let output = Document::load_mem(&output).unwrap();
        let page = output.get_dictionary(output.get_pages()[&1]).unwrap();
        let resources = as_dict(&output, page.get(b"Resources").unwrap());
        assert!(resources.get(b"XObject").is_ok());
    }

    #[test]
    fn overlay_clip_discards_unfinished_source_path_in_both_graft_routes() {
        for (source_content, expected) in [
            ("70 70 20 20 re W\n", vec!["W", "n", "Q", "n", "q"]),
            ("q 70 70 20 20 re W*\n", vec!["W*", "n", "Q", "Q", "n", "q"]),
        ] {
            let mut source = Document::load_mem(&source_without_resources()).unwrap();
            let page_id = source.get_pages()[&1];
            let content = source.add_object(Stream::new(
                Dictionary::new(),
                source_content.as_bytes().to_vec(),
            ));
            source
                .get_dictionary_mut(page_id)
                .unwrap()
                .set("Contents", content);
            let mut bytes = Vec::new();
            source.save_to(&mut bytes).unwrap();
            let overlay_bytes = sdk_overlay_pdf();
            let matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
            let clip = [0.0, 0.0, 20.0, 20.0];

            let grafted = graft_overlay_page(&bytes, 1, &overlay_bytes, 1, matrix, clip).unwrap();
            let grafted = Document::load_mem(&grafted).unwrap();
            let grafted_page = grafted.get_dictionary(grafted.get_pages()[&1]).unwrap();

            let overlay = Document::load_mem(&overlay_bytes).unwrap();
            let mut page = source.get_dictionary(page_id).unwrap().clone();
            add_overlay_form(
                &mut source,
                &mut page,
                &overlay,
                overlay.get_pages()[&1],
                matrix,
                clip,
                None,
            )
            .unwrap();

            for (doc, page) in [(&grafted, grafted_page), (&source, &page)] {
                let content = contents_bytes(doc, page).unwrap();
                let operators: Vec<_> = Content::decode(&content)
                    .unwrap()
                    .operations
                    .into_iter()
                    .map(|op| op.operator)
                    .collect();
                let source_path = operators.iter().position(|op| op == "re").unwrap();
                assert_eq!(
                    &operators[source_path + 1..source_path + 1 + expected.len()],
                    expected
                );
            }
        }
    }

    #[test]
    fn invalid_explicit_placement_domains_are_rejected_before_pdf_parsing() {
        let input = b"not a PDF";
        let overlay = b"not a PDF";
        for matrix in [
            [f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, f64::INFINITY, 0.0, 0.0],
        ] {
            assert!(
                graft_overlay_page(input, 1, overlay, 1, matrix, [0.0, 0.0, 10.0, 10.0])
                    .unwrap_err()
                    .contains("matrix contains non-finite")
            );
        }
        for clip in [
            [0.0, 0.0, f64::INFINITY, 10.0],
            [0.0, 0.0, 0.0, 10.0],
            [2.0, 0.0, 1.0, 10.0],
            [-f64::MAX, 0.0, f64::MAX, 10.0],
        ] {
            assert!(
                graft_overlay_page(input, 1, overlay, 1, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], clip,)
                    .unwrap_err()
                    .contains("clip")
            );
        }
    }

    fn reachable_resource_kinds(doc: &Document, root: &Dictionary) -> (usize, usize, usize, usize) {
        let mut pending = vec![Object::Dictionary(root.clone())];
        let mut seen = std::collections::HashSet::<ObjectId>::new();
        let (mut fonts, mut shadings, mut soft_masks, mut images) = (0, 0, 0, 0);
        while let Some(object) = pending.pop() {
            let object = match object {
                Object::Reference(id) => {
                    if !seen.insert(id) {
                        continue;
                    }
                    doc.get_object(id).unwrap().clone()
                }
                other => other,
            };
            match object {
                Object::Dictionary(dict) => {
                    classify_dictionary(
                        &dict,
                        &mut fonts,
                        &mut shadings,
                        &mut soft_masks,
                        &mut images,
                    );
                    pending.extend(dict.into_iter().map(|(_, value)| value));
                }
                Object::Stream(stream) => {
                    classify_dictionary(
                        &stream.dict,
                        &mut fonts,
                        &mut shadings,
                        &mut soft_masks,
                        &mut images,
                    );
                    pending.extend(stream.dict.into_iter().map(|(_, value)| value));
                }
                Object::Array(items) => pending.extend(items),
                _ => {}
            }
        }
        (fonts, shadings, soft_masks, images)
    }

    fn classify_dictionary(
        dict: &Dictionary,
        fonts: &mut usize,
        shadings: &mut usize,
        soft_masks: &mut usize,
        images: &mut usize,
    ) {
        if dict
            .get(b"Type")
            .and_then(Object::as_name)
            .is_ok_and(|n| n == b"Font")
            || dict.has(b"FontFile2")
            || dict.has(b"FontFile3")
        {
            *fonts += 1;
        }
        if dict.has(b"ShadingType") {
            *shadings += 1;
        }
        if dict.has(b"SMask")
            || dict
                .get(b"S")
                .and_then(Object::as_name)
                .is_ok_and(|n| n == b"Luminosity")
        {
            *soft_masks += 1;
        }
        if dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .is_ok_and(|n| n == b"Image")
        {
            *images += 1;
        }
    }
}
