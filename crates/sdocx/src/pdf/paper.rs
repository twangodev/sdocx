//! Test-only source-preserving PDF paper graft prototype.
//!
//! This module is intentionally not wired into production. Placement remains an explicit
//! caller input until the full native writer-to-source-page coordinate chain is verified.

use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, content::Content};

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
        *dst = value
            .as_float()
            .or_else(|_| value.as_i64().map(|v| v as f32))
            .map_err(|e| e.to_string())? as f64;
    }
    if !out.iter().all(|value| value.is_finite()) {
        return Err("page box contains non-finite coordinates".into());
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
    append(doc, inherited(doc, page, b"Contents")?, &mut out, 0)?;
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
    flatten(doc, object, 0)
}

/// Graft one overlay PDF page onto a page in the original source PDF.
/// `display_to_source` maps origin-normalized overlay display coordinates
/// (including the overlay page's CropBox/Rotate/UserUnit) into source page user
/// coordinates. `clip` is the corresponding clip rectangle in source user
/// coordinates. Keeping this explicit avoids asserting Samsung's unresolved
/// object-placement rule.
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
        "q {} {} {} {} re W n {} {} {} {} {} {} cm /{} Do Q\n",
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
    let mut suffix = Vec::with_capacity((source_q_depth + 1) * 2);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PdfOptions, RenderedPage};
    use lopdf::{Object, dictionary};
    use std::sync::Arc;

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
