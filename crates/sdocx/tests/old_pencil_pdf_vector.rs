#![cfg(feature = "pdf")]
#[path = "helpers/old_pencil_dot_channels.rs"]
mod dot_channels;
use sdocx::{
    BoundingBox, Color, Document, ObjectType, Page, ParseLimits, PdfOptions, RenderOptions,
    StoredObject, render_document_svg,
};
fn stroke(argb: u32) -> sdocx::Stroke {
    let mut b = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    b[2627..2631].copy_from_slice(&argb.to_le_bytes());
    let o = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (b.len() + 32) as u32,
        payload_offset: 0,
        payload_size: b.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    o.decode_stroke(&b, &ParseLimits::default()).unwrap()
}
fn overlap_2d_second_stroke() -> sdocx::Stroke {
    let mut bytes = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    bytes[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    for i in 0..203 {
        bytes[173 + 4 * i + 1] ^= 0x80;
    }
    StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (bytes.len() + 32) as u32,
        payload_offset: 0,
        payload_size: bytes.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    }
    .decode_stroke(&bytes, &ParseLimits::default())
    .unwrap()
}
fn reflected_stroke() -> sdocx::Stroke {
    let mut bytes = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    bytes[2631..2635].copy_from_slice(&8.0_f32.to_le_bytes());
    bytes[2639..2643].copy_from_slice(&8.0_f32.to_le_bytes());
    for i in 0..203 {
        bytes[173 + 4 * i + 1] ^= 0x80;
    }
    let obj = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (bytes.len() + 32) as u32,
        payload_offset: 0,
        payload_size: bytes.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    obj.decode_stroke(&bytes, &ParseLimits::default()).unwrap()
}
fn size5_stroke(reflected: bool) -> sdocx::Stroke {
    let mut bytes = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for offset in [2631, 2639] {
        bytes[offset..offset + 4].copy_from_slice(&5.0_f32.to_le_bytes());
    }
    if reflected {
        bytes[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
        for i in 0..203 {
            bytes[173 + 4 * i + 1] ^= 0x80;
        }
    }
    let object = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (bytes.len() + 32) as u32,
        payload_offset: 0,
        payload_size: bytes.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    object
        .decode_stroke(&bytes, &ParseLimits::default())
        .unwrap()
}

fn size12_stroke(case: &str) -> sdocx::Stroke {
    let mut bytes = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for offset in [2631, 2639] {
        bytes[offset..offset + 4].copy_from_slice(&12.0_f32.to_le_bytes());
    }
    if case == "reflect" {
        bytes[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
        for i in 0..203 {
            bytes[173 + 4 * i + 1] ^= 0x80;
        }
    } else if case == "rapid" {
        bytes[2627..2631].copy_from_slice(&0xc0ee8811_u32.to_le_bytes());
        for i in 0..203 {
            let off = 173 + 4 * i;
            let value = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
            bytes[off..off + 2]
                .copy_from_slice(&((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes());
            if i >= 100 {
                bytes[off + 3] ^= 0x80;
            }
        }
        bytes[985..989].copy_from_slice(&0.125_f32.to_le_bytes());
        for i in 0..203 {
            let value: u16 = if i % 2 == 0 { 3072 } else { 0x8c00 };
            bytes[989 + 2 * i..991 + 2 * i].copy_from_slice(&value.to_le_bytes());
        }
    }
    let object = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (bytes.len() + 32) as u32,
        payload_offset: 0,
        payload_size: bytes.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    object
        .decode_stroke(&bytes, &ParseLimits::default())
        .unwrap()
}

fn doc(stroke: sdocx::Stroke) -> Document {
    Document {
        pages: vec![Page {
            uuid: "test".into(),
            width: 1920,
            height: 1080,
            objects: vec![stroke.into()],
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            content_bbox: BoundingBox::default(),
            template: None,
            background: Default::default(),
        }],
        metadata: Default::default(),
    }
}
fn svg(doc: &Document, opt: &RenderOptions) -> String {
    render_document_svg(doc, opt).remove(0).svg
}
fn verify_mask_graph(document: &lopdf::Document, root: &lopdf::Stream) -> Vec<lopdf::ObjectId> {
    use sha2::{Digest, Sha256};
    let expected = [
        "3e46977cab92725d8b4b78d8492e23f73f5c9071799320544b9734e5477a6900",
        "d65fdc9aa1535c27dc1619514b92948e133924b720b67fc638c39420ea231f67",
        "3e85f3d635bc6e543e21b8a1d8bd334b774cc1587d97af7c2fc2d8772c7e8b85",
        "0c4a918dd8c665bfa5b335de3a6b63c611ac1e4d02b150c7b2770cd9bd5b5686",
        "0b3c70cda19d2ce5009bbb4ece8ec99c681c9f5b2b0ee0743cf9e0a478c7f892",
        "bf714853cc77b4316bc90d120d3008eee500ee6378963c66baf639ba3e931a02",
        "a32c2810b1cb965065c28876ebf72ba31023e2c011fe6a660ba2e577de0fb18b",
    ];
    let states = root
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"ExtGState")
        .unwrap()
        .as_dict()
        .unwrap();
    expected
        .into_iter()
        .enumerate()
        .map(|(tile, hash)| {
            let state_id = states
                .get(format!("T{tile}").as_bytes())
                .unwrap()
                .as_reference()
                .unwrap();
            let state = document.get_object(state_id).unwrap().as_dict().unwrap();
            let soft_mask = state.get(b"SMask").unwrap().as_dict().unwrap();
            assert_eq!(
                soft_mask.get(b"S").unwrap().as_name().unwrap(),
                b"Luminosity"
            );
            let id = soft_mask.get(b"G").unwrap().as_reference().unwrap();
            let form = document.get_object(id).unwrap().as_stream().unwrap();
            assert_eq!(
                form.dict.get(b"Subtype").unwrap().as_name().unwrap(),
                b"Form"
            );
            let group = form.dict.get(b"Group").unwrap().as_dict().unwrap();
            assert_eq!(group.get(b"S").unwrap().as_name().unwrap(), b"Transparency");
            assert!(group.get(b"I").unwrap().as_bool().unwrap());
            assert_eq!(
                soft_mask.get(b"BC").unwrap().as_array().unwrap()[0]
                    .as_i64()
                    .unwrap(),
                0
            );
            let content = form.decompressed_content().unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&content)), hash);
            assert!(content.starts_with(b"0 g 0 0 48 48 re f\n"));
            assert!(!content.windows(3).any(|w| w == b" gs"));
            id
        })
        .collect()
}

#[test]
fn direct_pdf_opt_in_is_independent_and_retains_stamp_order() {
    let plain = RenderOptions::default();
    let mut pdf = plain.clone();
    pdf.old_pencil_pdf_profile = true;
    let mut svgopt = plain.clone();
    svgopt.old_pencil_svg_profile = true;
    let d = doc(stroke(0x8024a0d8));
    assert_eq!(svg(&d, &plain), svg(&d, &pdf));
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &svgopt, &PdfOptions::default()).unwrap()
    );
    let mut both = svgopt.clone();
    both.old_pencil_pdf_profile = true;
    assert_eq!(
        sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &both, &PdfOptions::default()).unwrap()
    );
    let candidate = sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap();
    let parsed = lopdf::Document::load_mem(&candidate).unwrap();
    let forms: Vec<_> = parsed
        .objects
        .values()
        .filter_map(|v| v.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Form"))
        .collect();
    let tiles = forms
        .iter()
        .filter(|s| {
            s.dict
                .get(b"BBox")
                .ok()
                .and_then(|v| v.as_array().ok())
                .is_some_and(|v| {
                    v.len() == 4 && v[2].as_i64().ok() == Some(48) && v[3].as_i64().ok() == Some(48)
                })
        })
        .count();
    assert_eq!(tiles, 7);
    let tile = forms
        .iter()
        .find(|s| {
            s.dict
                .get(b"BBox")
                .ok()
                .and_then(|v| v.as_array().ok())
                .is_some_and(|v| v.len() == 4 && v[2].as_i64().ok() == Some(48))
        })
        .unwrap();
    let group = tile.dict.get(b"Group").unwrap().as_dict().unwrap();
    assert_eq!(group.get(b"S").unwrap().as_name().unwrap(), b"Transparency");
    assert!(group.get(b"I").unwrap().as_bool().unwrap());
    assert_eq!(group.get(b"CS").unwrap().as_name().unwrap(), b"DeviceGray");

    let roots: Vec<_> = forms
        .iter()
        .filter_map(|s| s.decompressed_content().ok())
        .filter(|v| v.windows(6).any(|w| w == b"/T0 gs"))
        .collect();
    assert_eq!(roots.len(), 1);
    let root = forms
        .iter()
        .find(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|v| v.windows(6).any(|w| w == b"/T0 gs"))
        })
        .unwrap();
    assert_eq!(verify_mask_graph(&parsed, root).len(), 7);
    let root_states = root
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"ExtGState")
        .unwrap()
        .as_dict()
        .unwrap();
    let root_state = parsed
        .get_object(root_states.get(b"T0").unwrap().as_reference().unwrap())
        .unwrap()
        .as_dict()
        .unwrap();
    assert!((root_state.get(b"ca").unwrap().as_f32().unwrap() - (128.0 / 255.0)).abs() < 0.000001);
    let mask = root_state.get(b"SMask").unwrap().as_dict().unwrap();
    assert_eq!(mask.get(b"S").unwrap().as_name().unwrap(), b"Luminosity");
    assert!(mask.get(b"G").unwrap().as_reference().is_ok());

    let content = String::from_utf8(roots[0].clone()).unwrap();
    let actual: Vec<_> = content
        .lines()
        .filter_map(|line| {
            line.split("/T")
                .nth(1)
                .and_then(|x| x.split_whitespace().next())
                .and_then(|x| x.parse::<usize>().ok())
        })
        .collect();
    assert_eq!(actual, (0..31).map(|i| i % 7).collect::<Vec<_>>());
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].objects = (0..3).map(|_| stroke(0x8024a0d8).into()).collect();
    let candidate = sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap();
    let parsed = lopdf::Document::load_mem(&candidate).unwrap();
    let n = parsed
        .objects
        .values()
        .filter_map(|v| v.as_stream().ok())
        .filter_map(|s| s.decompressed_content().ok())
        .filter(|v| v.windows(6).any(|w| w == b"/T0 gs"))
        .count();
    assert_eq!(n, 2);
    let roots: Vec<_> = parsed
        .objects
        .values()
        .filter_map(|v| v.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|v| v.windows(6).any(|w| w == b"/T0 gs"))
        })
        .collect();
    let masks: std::collections::HashSet<_> = roots
        .into_iter()
        .flat_map(|root| verify_mask_graph(&parsed, root))
        .collect();
    assert_eq!(
        masks.len(),
        14,
        "each stroke retains its own seven renumbered tile masks"
    );

    let page_id = *parsed.get_pages().values().next().unwrap();
    let page_content = String::from_utf8(parsed.get_page_content(page_id).unwrap()).unwrap();
    assert!(
        page_content.lines().any(|line| line.trim() == "S"),
        "third stroke must remain visible as fallback"
    );

    let mut degenerate = stroke(0x8024a0d8);
    let point = degenerate.points[0];
    degenerate.points.fill(point);
    let d = doc(degenerate);
    assert!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap()
            != sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap(),
        "a multi-point stroke with no accepted moves uses the dot profile"
    );
    let mut invalid = stroke(0x8024a0d8);
    invalid.points[0].x = f64::NAN;
    let d = doc(invalid);
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap()
    );
    let mut invalid = stroke(0x8024a0d8);
    invalid.points[0].x = 1.0e38;
    let d = doc(invalid);
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap()
    );
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].width = 1919;
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap()
    );
    let mut bad = stroke(0x8024a0d8);
    bad.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1);
    let d = doc(bad);
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &pdf, &PdfOptions::default()).unwrap()
    );
}

#[test]
fn synthetic_size5_reflected_retains_direct_pdf_fallback() {
    let default = RenderOptions::default();
    let mut pdf = default.clone();
    pdf.old_pencil_pdf_profile = true;
    let document = doc(size5_stroke(true));
    assert_eq!(
        sdocx::render_document_pdf(&document, &default, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&document, &pdf, &PdfOptions::default()).unwrap()
    );
}

fn run_pdf_oracle(python: std::ffi::OsString, reader: &str) {
    let path = std::env::temp_dir().join(format!(
        "sdocx-old-pencil-pdf-oracle-{}-{reader}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    for (name, color) in [("blue", 0x8024a0d8), ("alt", 0x40f130a0)] {
        let doc = doc(stroke(color));
        for enabled in [false, true] {
            let mut options = RenderOptions::default();
            options.old_pencil_pdf_profile = enabled;
            let bytes = sdocx::render_document_pdf(&doc, &options, &PdfOptions::default()).unwrap();
            std::fs::write(path.join(format!("{name}-{enabled}.pdf")), bytes).unwrap();
        }
    }
    for case in ["plain", "reflect", "rapid"] {
        let document = doc(size12_stroke(case));
        for enabled in [false, true] {
            let mut options = RenderOptions::default();
            options.old_pencil_pdf_profile = enabled;
            let bytes =
                sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap();
            std::fs::write(path.join(format!("size12_{case}-{enabled}.pdf")), bytes).unwrap();
        }
    }
    let dot = doc(size12_visible_dot());
    let plain = sdocx::render_document_pdf(&dot, &RenderOptions::default(), &PdfOptions::default())
        .unwrap();
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    let profile = sdocx::render_document_pdf(&dot, &options, &PdfOptions::default()).unwrap();
    assert_eq!(plain, profile);
    std::fs::write(path.join("size12_dot-false.pdf"), &plain).unwrap();
    std::fs::write(path.join("size12_dot-true.pdf"), &profile).unwrap();
    let mut overlap_document = doc(stroke(0x8024a0d8));
    overlap_document.pages[0]
        .objects
        .push(stroke(0x8024a0d8).into());
    for enabled in [false, true] {
        let mut options = RenderOptions::default();
        options.old_pencil_pdf_profile = enabled;
        let bytes = sdocx::render_document_pdf(&overlap_document, &options, &PdfOptions::default())
            .unwrap();
        std::fs::write(path.join(format!("overlap-{enabled}.pdf")), bytes).unwrap();
    }
    let mut overlap_2d = doc(stroke(0x8024a0d8));
    overlap_2d.pages[0]
        .objects
        .push(overlap_2d_second_stroke().into());
    for enabled in [false, true] {
        let mut options = RenderOptions::default();
        options.old_pencil_pdf_profile = enabled;
        let bytes =
            sdocx::render_document_pdf(&overlap_2d, &options, &PdfOptions::default()).unwrap();
        std::fs::write(path.join(format!("overlap_2d-{enabled}.pdf")), bytes).unwrap();
    }
    let reflected_document = doc(reflected_stroke());
    for enabled in [false, true] {
        let mut options = RenderOptions::default();
        options.old_pencil_pdf_profile = enabled;
        let bytes =
            sdocx::render_document_pdf(&reflected_document, &options, &PdfOptions::default())
                .unwrap();
        std::fs::write(path.join(format!("reflect-{enabled}.pdf")), bytes).unwrap();
    }
    for (name, reflected) in [("size5_plain", false), ("size5_reflect", true)] {
        let document = doc(size5_stroke(reflected));
        for enabled in [false, true] {
            let mut options = RenderOptions::default();
            options.old_pencil_pdf_profile = enabled;
            let bytes =
                sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap();
            std::fs::write(path.join(format!("{name}-{enabled}.pdf")), bytes).unwrap();
        }
    }
    for (scale, target) in [(1, 50.5_f64), (1, 50.25), (2, 50.5), (2, 50.25)] {
        let origin = target - (24.5 / 48.0) * 7.0;
        let start = origin + 24.0 * 7.0 / 48.0;
        let side = 7.0 / 48.0;
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="{start}" y="{start}" width="{side}" height="{side}" fill="black"/></svg>"#
        );
        let page = sdocx::RenderedPage::new(0, 100, 100, source);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &PdfOptions::default()).unwrap();
        std::fs::write(
            path.join(format!(
                "onehot-{scale}-{}.pdf",
                if target == 50.5 { "center" } else { "quarter" }
            )),
            bytes,
        )
        .unwrap();
    }
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new(python)
        .arg(base.join("tests/old_pencil_pdf_compare.py"))
        .arg(base.join("../../conformance"))
        .arg(&path)
        .arg(reader)
        .output()
        .unwrap();
    std::fs::remove_dir_all(path).unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires PyMuPDF 1.28.2; set SDOCX_PDF_ORACLE_PYTHON"]
fn direct_pdf_matches_retained_native_frames_with_configured_mupdf() {
    let python = std::env::var_os("SDOCX_PDF_ORACLE_PYTHON")
        .expect("set SDOCX_PDF_ORACLE_PYTHON to a Python with PyMuPDF 1.28.2");
    run_pdf_oracle(python, "mupdf");
}

#[test]
#[ignore = "requires PyMuPDF 1.28.2, pypdfium2 5.9.0, PDFium 150.0.7869.0, and Pillow; set SDOCX_PDFIUM_ORACLE_PYTHON"]
fn direct_pdf_matches_retained_native_ink_deltas_with_configured_pdfium() {
    let python = std::env::var_os("SDOCX_PDFIUM_ORACLE_PYTHON")
        .expect("set SDOCX_PDFIUM_ORACLE_PYTHON to a Python with the pinned readers");
    run_pdf_oracle(python, "pdfium");
}

#[cfg(feature = "serde")]
#[test]
fn missing_pdf_profile_deserializes_off() {
    let mut value = serde_json::to_value(RenderOptions::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("old_pencil_pdf_profile");
    let options: RenderOptions = serde_json::from_value(value).unwrap();
    assert!(!options.old_pencil_pdf_profile);
}

#[test]
fn zero_alpha_pdf_profile_uses_transparent_stamp_states() {
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    let bytes =
        sdocx::render_document_pdf(&doc(stroke(0x0024a0d8)), &options, &PdfOptions::default())
            .unwrap();
    let parsed = lopdf::Document::load_mem(&bytes).unwrap();
    let roots: Vec<_> = parsed
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|stream| {
            stream
                .decompressed_content()
                .ok()
                .is_some_and(|data| data.windows(6).any(|w| w == b"/T0 gs"))
        })
        .collect();
    assert_eq!(roots.len(), 1);
    let root = roots[0];
    assert_eq!(verify_mask_graph(&parsed, root).len(), 7);
    let content = String::from_utf8(root.decompressed_content().unwrap()).unwrap();
    let actual: Vec<_> = content
        .lines()
        .filter_map(|line| {
            line.split("/T")
                .nth(1)
                .and_then(|x| x.split_whitespace().next())
                .and_then(|x| x.parse::<usize>().ok())
        })
        .collect();
    assert_eq!(actual, (0..31).map(|i| i % 7).collect::<Vec<_>>());
    let states = root
        .dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"ExtGState")
        .unwrap()
        .as_dict()
        .unwrap();
    for tile in 0..7 {
        let state = parsed
            .get_object(
                states
                    .get(format!("T{tile}").as_bytes())
                    .unwrap()
                    .as_reference()
                    .unwrap(),
            )
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(state.get(b"ca").unwrap().as_i64().unwrap(), 0);
    }
}

#[test]
fn serialized_subthreshold_dot_pdf_uses_one_native_stamp() {
    let mut b = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for offset in [2631, 2639] {
        b[offset..offset + 4].copy_from_slice(&8.0_f32.to_le_bytes());
    }
    b[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    b[173..985].fill(0);
    b[173..175].copy_from_slice(&2u16.to_le_bytes());
    b[985..989].copy_from_slice(&0.5_f32.to_le_bytes());
    b[989..1395].fill(0);
    let obj = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (b.len() + 32) as u32,
        payload_offset: 0,
        payload_size: b.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    let s = obj.decode_stroke(&b, &ParseLimits::default()).unwrap();
    let d = doc(s);
    let mut opt = RenderOptions::default();
    opt.old_pencil_pdf_profile = true;
    let bytes = sdocx::render_document_pdf(&d, &opt, &PdfOptions::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let roots: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|data| data.windows(6).any(|w| w == b"/T0 gs"))
        })
        .collect();
    assert_eq!(roots.len(), 1);
    let root = roots[0];
    assert_eq!(verify_mask_graph(&pdf, root).len(), 7);
    let content = String::from_utf8(root.decompressed_content().unwrap()).unwrap();
    let actual: Vec<_> = content
        .lines()
        .filter_map(|line| {
            line.split("/T")
                .nth(1)
                .and_then(|x| x.split_whitespace().next())
                .and_then(|x| x.parse::<usize>().ok())
        })
        .collect();
    assert_eq!(actual, [0]);
}

#[test]
fn one_point_saved_stroke_keeps_pdf_fallback() {
    let mut s = stroke(0x8024a0d8);
    s.points.truncate(1);
    s.pressures.truncate(1);
    s.timestamps.truncate(1);
    s.tilts.truncate(1);
    s.orientations.truncate(1);
    let d = doc(s);
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_pdf_profile = true;
    assert_eq!(
        sdocx::render_document_pdf(&d, &plain, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&d, &opt, &PdfOptions::default()).unwrap()
    );
}

#[test]
fn size12_profile_is_exact_and_bounded() {
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    let document = doc(size12_stroke("plain"));
    assert_ne!(
        sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&document, &RenderOptions::default(), &PdfOptions::default())
            .unwrap()
    );
    for size in [
        f32::from_bits(12.0_f32.to_bits() - 1),
        f32::from_bits(12.0_f32.to_bits() + 1),
    ] {
        let mut s = size12_stroke("plain");
        s.pen_width = size;
        let style = &mut s.rendering.as_mut().unwrap().style;
        style.pen_size = Some(size);
        style.fixed_width = Some(size);
        let d = doc(s);
        assert_eq!(
            sdocx::render_document_pdf(&d, &options, &PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &RenderOptions::default(), &PdfOptions::default())
                .unwrap()
        );
    }
    let mut many = document.clone();
    many.pages[0].objects = vec![size12_stroke("plain").into(); 3];
    let bytes = sdocx::render_document_pdf(&many, &options, &PdfOptions::default()).unwrap();
    let parsed = lopdf::Document::load_mem(&bytes).unwrap();
    let roots = parsed
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|d| d.windows(6).any(|w| w == b"/T0 gs"))
        })
        .count();
    assert_eq!(roots, 2);
    let page_id = *parsed.get_pages().values().next().unwrap();
    let operations = lopdf::content::Content::decode(&parsed.get_page_content(page_id).unwrap())
        .unwrap()
        .operations;
    let fallback_bytes =
        sdocx::render_document_pdf(&document, &RenderOptions::default(), &PdfOptions::default())
            .unwrap();
    let fallback = lopdf::Document::load_mem(&fallback_bytes).unwrap();
    let page_id = *fallback.get_pages().values().next().unwrap();
    let expected = lopdf::content::Content::decode(&fallback.get_page_content(page_id).unwrap())
        .unwrap()
        .operations;
    let last_path = |ops: &[lopdf::content::Operation]| {
        let end = ops.iter().rposition(|op| op.operator == "S").unwrap();
        let start = ops[..end]
            .iter()
            .rposition(|op| op.operator == "m")
            .unwrap();
        format!("{:?}", &ops[start..=end])
    };
    assert_eq!(
        last_path(&operations),
        last_path(&expected),
        "third stroke uses the same fallback path"
    );
    let mut rejected = size12_stroke("plain");
    let neighbor = f32::from_bits(12.0_f32.to_bits() - 1);
    rejected.pen_width = neighbor;
    let style = &mut rejected.rendering.as_mut().unwrap().style;
    style.pen_size = Some(neighbor);
    style.fixed_width = Some(neighbor);
    let mut preceded = document.clone();
    preceded.pages[0].objects = vec![
        rejected.into(),
        size12_stroke("plain").into(),
        size12_stroke("plain").into(),
    ];
    let parsed = lopdf::Document::load_mem(
        &sdocx::render_document_pdf(&preceded, &options, &PdfOptions::default()).unwrap(),
    )
    .unwrap();
    let roots = parsed
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|d| d.windows(6).any(|w| w == b"/T0 gs"))
        })
        .count();
    assert_eq!(
        roots, 2,
        "rejected size does not consume the profile budget"
    );
}

fn size12_visible_dot() -> sdocx::Stroke {
    let mut b = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for off in [2631, 2639] {
        b[off..off + 4].copy_from_slice(&12.0_f32.to_le_bytes());
    }
    b[173..985].fill(0);
    b[173..175].copy_from_slice(&2_u16.to_le_bytes());
    b[985..989].copy_from_slice(&0.0_f32.to_le_bytes());
    b[989..1395].fill(0);
    let o = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (b.len() + 32) as u32,
        payload_offset: 0,
        payload_size: b.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    o.decode_stroke(&b, &ParseLimits::default()).unwrap()
}

#[test]
fn size12_visible_dot_decoded_channels_match_native_getters() {
    let stroke = size12_visible_dot();
    let mut channels = Vec::new();
    for point in &stroke.points {
        channels.extend_from_slice(&(point.x as f32).to_le_bytes());
        channels.extend_from_slice(&(point.y as f32).to_le_bytes());
    }
    for value in &stroke.pressures {
        channels.extend_from_slice(&(*value as f32).to_le_bytes());
    }
    for value in &stroke.timestamps {
        channels.extend_from_slice(&(*value as i32).to_le_bytes());
    }
    for value in stroke.tilts.iter().chain(&stroke.orientations) {
        channels.extend_from_slice(&(*value as f32).to_le_bytes());
    }
    assert_eq!(channels, dot_channels::channels("dot-12-visible.channels"));
}

#[test]
fn size12_dot_falls_back_without_consuming_stamp_budget() {
    let dot = size12_visible_dot();
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    let document = doc(dot.clone());
    assert_eq!(
        sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&document, &RenderOptions::default(), &PdfOptions::default())
            .unwrap()
    );
    let mut many = document;
    many.pages[0].objects = vec![
        dot.into(),
        size12_stroke("plain").into(),
        size12_stroke("plain").into(),
    ];
    let parsed = lopdf::Document::load_mem(
        &sdocx::render_document_pdf(&many, &options, &PdfOptions::default()).unwrap(),
    )
    .unwrap();
    let roots = parsed
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|d| d.windows(6).any(|w| w == b"/T0 gs"))
        })
        .count();
    assert_eq!(roots, 2);
}

#[test]
fn edge_crossing_fallback_does_not_consume_stamp_budget() {
    let mut edge = size12_stroke("plain");
    let min_x = edge
        .points
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min);
    for point in &mut edge.points {
        point.x -= min_x;
    }
    let edge_document = doc(edge.clone());
    let plain_svg = svg(&edge_document, &RenderOptions::default());
    let mut svg_options = RenderOptions::default();
    svg_options.old_pencil_svg_profile = true;
    let profile_svg = svg(&edge_document, &svg_options);
    assert!(!plain_svg.contains("crispEdges"));
    assert!(
        profile_svg.contains("crispEdges"),
        "edge stroke must pass shared preparation"
    );
    let crossing_stamp = profile_svg
        .split("transform=\"matrix(")
        .skip(1)
        .any(|part| {
            let Some((matrix, rest)) = part.split_once(')') else {
                return false;
            };
            let values: Vec<_> = matrix
                .split(',')
                .filter_map(|value| value.parse::<f64>().ok())
                .collect();
            values.len() == 6
                && values[0] > 0.0
                && values[4] < 0.0
                && rest
                    .split_once("</g>")
                    .is_some_and(|(group, _)| group.contains("<use"))
        });
    assert!(
        crossing_stamp,
        "a sampled stamp must extend past the left page edge"
    );
    let mut document = doc(size12_stroke("plain"));
    document.pages[0].objects = vec![edge.into(); 16];
    document.pages[0]
        .objects
        .push(size12_stroke("plain").into());
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    let parsed = lopdf::Document::load_mem(
        &sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap(),
    )
    .unwrap();
    let roots = parsed
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| {
            s.decompressed_content()
                .ok()
                .is_some_and(|d| d.windows(6).any(|w| w == b"/T0 gs"))
        })
        .count();
    assert_eq!(
        roots, 1,
        "edge fallback must leave stamp budget for the following stroke"
    );
}

#[test]
fn edge_crossing_pdf_rejections_consume_point_work_budget() {
    let mut edge = size12_stroke("plain");
    let min_x = edge
        .points
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min);
    for point in &mut edge.points {
        point.x -= min_x;
    }
    let valid = size12_stroke("plain");
    assert_eq!(edge.points.len(), 204);
    let mut options = RenderOptions::default();
    options.old_pencil_pdf_profile = true;
    for (attempts, expected_roots) in [(39, 1), (40, 0)] {
        let mut document = doc(valid.clone());
        document.pages[0].objects = vec![edge.clone().into(); attempts];
        document.pages[0].objects.push(valid.clone().into());
        let parsed = lopdf::Document::load_mem(
            &sdocx::render_document_pdf(&document, &options, &PdfOptions::default()).unwrap(),
        )
        .unwrap();
        let roots = parsed
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| {
                s.decompressed_content()
                    .ok()
                    .is_some_and(|d| d.windows(6).any(|w| w == b"/T0 gs"))
            })
            .count();
        assert_eq!(
            roots, expected_roots,
            "after {attempts} rejected edge strokes"
        );
    }
}
